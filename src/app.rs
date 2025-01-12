use rustc_hash::FxHashSet as Set;
use std::sync::Arc;

use color_eyre::Result;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};

use crate::channel::{ChannelConfigs, Channel};
use crate::config::{Config, KeyBindings};
use crate::television::{OnAir, Television};
use crate::{
    action::Action,
    event::{Event, EventLoop, Key},
    tui::{self, RenderingTask},
};
use crate::television::Mode;
use crate::entry::Entry;

// Tui app
pub struct App {
    keymap: KeyBindings,
    tick_rate: f64,
    frame_rate: f64,
    /// The television instance that handles channels and entries.
    television: Arc<Mutex<Television>>,
    /// A flag that indicates whether the application should quit during the next frame.
    should_quit: bool,
    /// A flag that indicates whether the application should suspend during the next frame.
    should_suspend: bool,
    /// A sender channel for actions.
    action_tx: mpsc::UnboundedSender<Action>,
    /// The receiver channel for actions.
    action_rx: mpsc::UnboundedReceiver<Action>,
    /// The receiver channel for events.
    event_rx: mpsc::UnboundedReceiver<Event<Key>>,
    /// A sender channel to abort the event loop.
    event_abort_tx: mpsc::UnboundedSender<()>,
    /// A sender channel for rendering tasks.
    render_tx: mpsc::UnboundedSender<RenderingTask>,
}

#[derive(Debug)]
pub enum ExitAction {
    Entries(Set<Entry>),
    Input(String),
    Passthrough(Set<Entry>, String),
    Command(Vec<Entry>, String, String),
    None,
}

impl App {
    pub fn new(
        channel: Channel,
        config: Config,
        _passthrough_keybindings: &[String],
        input: Option<String>,
        channels: ChannelConfigs,
    ) -> Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (render_tx, _) = mpsc::unbounded_channel();
        let (_, event_rx) = mpsc::unbounded_channel();
        let (event_abort_tx, _) = mpsc::unbounded_channel();

        Ok(Self {
            keymap: config.keybindings.clone(),
            //     passthrough_keybindings
            //         .flat_map(|s| match parse_key(s) {
            //             Ok(key) => Ok((key, Action::SelectPassthrough(s.clone()))),
            //             Err(e) => Err(e),
            //         })
            tick_rate: config.ui.tick_rate,
            frame_rate: config.ui.frame_rate,
            television: Arc::new(Mutex::new(Television::new(channel, config, input, channels))),
            should_quit: false,
            should_suspend: false,
            action_tx,
            action_rx,
            event_rx,
            event_abort_tx,
            render_tx,
        })
    }

    /// Application main loop
    ///
    /// This function will start the event loop and the rendering loop and handle
    /// all actions that are sent to the application.
    /// The function will return the selected entry if the application is exited.
    ///
    pub async fn run(&mut self, is_output_tty: bool) -> Result<ExitAction> {
        debug!("Starting backend event loop");
        let event_loop = EventLoop::new(self.tick_rate, true);
        self.event_rx = event_loop.rx;
        self.event_abort_tx = event_loop.abort_tx;

        // Rendering loop
        debug!("Starting rendering loop");
        let (render_tx, render_rx) = mpsc::unbounded_channel();
        self.render_tx = render_tx.clone();
        let action_tx_r = self.action_tx.clone();
        let television_r = self.television.clone();
        let frame_rate = self.frame_rate;
        let rendering_task = tokio::spawn(async move {
            tui::render(
                render_rx,
                action_tx_r,
                television_r,
                frame_rate,
                is_output_tty,
            )
            .await
        });

        debug!("Starting event handling loop");
        let action_tx = self.action_tx.clone();

        // main loop
        loop {
            // handle event and convert to action
            if let Some(event) = self.event_rx.recv().await {
                let action = self.convert_event_to_action(event).await;
                action_tx.send(action)?;
            }

            let exit_action = self.handle_actions().await?;

            if self.should_quit {
                // send a termination signal to the event loop
                self.event_abort_tx.send(())?;

                // wait for the rendering task to finish
                rendering_task.await??;

                return Ok(exit_action);
            }
        }
    }

    /// Convert an event to an action.
    ///
    /// This function will convert an event to an action based on the current
    /// mode the television is in.
    ///
    async fn convert_event_to_action(&self, event: Event<Key>) -> Action {
        match event {
            Event::Input(keycode) => {
                info!("{:?} {:?}", keycode, self.television.lock().await.mode);
                // text input events
                match keycode {
                    Key::Backspace => return Action::DeletePrevChar,
                    Key::Ctrl('w') => return Action::DeletePrevWord,
                    Key::Delete => return Action::DeleteNextChar,
                    Key::Left => return Action::GoToPrevChar,
                    Key::Right => return Action::GoToNextChar,
                    Key::Home | Key::Ctrl('a') => {
                        return Action::GoToInputStart
                    }
                    Key::End | Key::Ctrl('e') => return Action::GoToInputEnd,
                    Key::Char(c) => return Action::AddInputChar(c),
                    _ => {}
                }

                // get action based on keybindings
                self.keymap.check_key_for_action(&keycode)
                    .unwrap_or(if let Key::Char(c) = keycode {
                        Action::AddInputChar(c)
                    } else {
                        Action::NoOp
                    })
            }
            // terminal events
            Event::Tick => Action::Tick,
            Event::Resize(x, y) => Action::Resize(x, y),
            Event::FocusGained => Action::Resume,
            Event::FocusLost => Action::Suspend,
            Event::Closed => Action::NoOp,
        }
    }

    /// Handle actions.
    ///
    /// This function will handle all actions that are sent to the application.
    /// The function will return the selected entry if the application is exited.
    ///
    async fn handle_actions(&mut self) -> Result<ExitAction> {
        while let Ok(action) = self.action_rx.try_recv() {
            if action != Action::Tick && action != Action::Render {
                debug!("{action:?}");
            }
            match action {
                Action::Quit => {
                    self.should_quit = true;
                    self.render_tx.send(RenderingTask::Quit)?;
                }
                Action::Suspend => {
                    self.should_suspend = true;
                    self.render_tx.send(RenderingTask::Suspend)?;
                }
                Action::Resume => {
                    self.should_suspend = false;
                    self.render_tx.send(RenderingTask::Resume)?;
                }
                Action::SelectAndExit => {
                    self.should_quit = true;
                    self.render_tx.send(RenderingTask::Quit)?;

                    info!("select and exit");
                    // Acquire lock
                    let mut television = self.television.lock().await;

                    let command = television.channel.run_command.clone();

                    if let Some(command) = command {

                        let entries: Vec<Entry> = if television.channel
                            .selected_entries()
                            .is_empty()
                        {
                            let entry = television
                                .results_picker
                                .selected()
                                .map(|i| {
                                    television.channel
                                        .get_result(i.try_into().unwrap())
                                        .unwrap()
                                })
                                .unwrap();
                            vec![entry]
                        } else {
                            television.channel
                                .selected_entries()
                                .iter()
                                .cloned()
                                .collect()
                        };

                        let delimiter = television.channel
                            .preview_command
                            .delimiter.clone();
                        info!("run cmd {command}");

                        return Ok(ExitAction::Command(entries, command, delimiter));
                        // run_command(entries, command, delimiter).await;
                    };


                    if let Some(entries) = television
                        .get_selected_entries(Some(Mode::Channel))
                    {
                        return Ok(ExitAction::Entries(entries));
                    }

                    return Ok(ExitAction::Input(
                        television.current_pattern.clone(),
                    ));
                }
                Action::SelectPassthrough(passthrough) => {
                    self.should_quit = true;
                    self.render_tx.send(RenderingTask::Quit)?;
                    if let Some(entries) = self
                        .television
                        .lock()
                        .await
                        .get_selected_entries(Some(Mode::Channel))
                    {
                        return Ok(ExitAction::Passthrough(
                            entries,
                            passthrough,
                        ));
                    }
                    return Ok(ExitAction::None);
                }
                Action::ClearScreen => {
                    self.render_tx.send(RenderingTask::ClearScreen)?;
                }
                Action::Resize(w, h) => {
                    self.render_tx.send(RenderingTask::Resize(w, h))?;
                }
                Action::Render => {
                    self.render_tx.send(RenderingTask::Render)?;
                }
                _ => {}
            }
            // forward action to the television handler
            if let Some(action) =
                self.television.lock().await.update(&action)?
            {
                self.action_tx.send(action)?;
            };
        }

        Ok(ExitAction::None)
    }

}
