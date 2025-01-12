use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll as TaskPoll},
    time::Duration,
};

use tokio::sync::mpsc;
use tracing::warn;

use crate::config::KeyEvent;

#[derive(Debug, Clone, Copy)]
pub enum Event<I> {
    Closed,
    Input(I),
    FocusLost,
    FocusGained,
    Resize(u16, u16),
    Tick,
}

#[allow(clippy::module_name_repetitions)]
pub struct EventLoop {
    pub rx: mpsc::UnboundedReceiver<Event<KeyEvent>>,
    pub abort_tx: mpsc::UnboundedSender<()>,
}

struct PollFuture {
    timeout: Duration,
}

impl Future for PollFuture {
    type Output = bool;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> TaskPoll<Self::Output> {
        // Polling crossterm::event::poll, which is a blocking call
        // Spawn it in a separate task, to avoid blocking async runtime
        match crossterm::event::poll(self.timeout) {
            Ok(true) => TaskPoll::Ready(true),
            Ok(false) => {
                // Register the task to be polled again after a delay to avoid busy-looping
                cx.waker().wake_by_ref();
                TaskPoll::Pending
            }
            Err(_) => TaskPoll::Ready(false),
        }
    }
}

async fn poll_event(timeout: Duration) -> bool {
    PollFuture { timeout }.await
}

impl EventLoop {
    pub fn new(tick_rate: f64, init: bool) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let tick_interval = Duration::from_secs_f64(1.0 / tick_rate);

        let (abort, mut abort_recv) = mpsc::unbounded_channel();

        if init {
            //let mut reader = crossterm::event::EventStream::new();
            tokio::spawn(async move {
                loop {
                    //let event = reader.next();
                    let delay = tokio::time::sleep(tick_interval);
                    let event_available = poll_event(tick_interval);

                    tokio::select! {
                        // if we receive a message on the abort channel, stop the event loop
                        _ = abort_recv.recv() => {
                            tx.send(Event::Closed).unwrap_or_else(|_| warn!("Unable to send Closed event"));
                            tx.send(Event::Tick).unwrap_or_else(|_| warn!("Unable to send Tick event"));
                            break;
                        },
                        // if `delay` completes, pass to the next event "frame"
                        () = delay => {
                            tx.send(Event::Tick).unwrap_or_else(|_| warn!("Unable to send Tick event"));
                        },
                        // if the receiver dropped the channel, stop the event loop
                        () = tx.closed() => break,
                        // if an event was received, process it
                        _ = event_available => {
                            let maybe_event = crossterm::event::read();
                            match maybe_event {
                                Ok(crossterm::event::Event::Key(key)) => {
                                    tx.send(Event::Input(key.into())).unwrap_or_else(|_| warn!("Unable to send {:?} event", key));
                                },
                                Ok(crossterm::event::Event::FocusLost) => {
                                    tx.send(Event::FocusLost).unwrap_or_else(|_| warn!("Unable to send FocusLost event"));
                                },
                                Ok(crossterm::event::Event::FocusGained) => {
                                    tx.send(Event::FocusGained).unwrap_or_else(|_| warn!("Unable to send FocusGained event"));
                                },
                                Ok(crossterm::event::Event::Resize(x, y)) => {
                                    tx.send(Event::Resize(x, y)).unwrap_or_else(|_| warn!("Unable to send Resize event"));
                                },
                                _ => {}
                            }
                        }
                    }
                }
            });
        }

        Self {
            rx,
            abort_tx: abort,
        }
    }
}
