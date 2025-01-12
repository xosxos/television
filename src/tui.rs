use std::{
    io::{stderr, stdout, LineWriter, Write},
    ops::{Deref, DerefMut},
    sync::Arc,
};

use color_eyre::Result;
use crossterm::{
    cursor, execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, is_raw_mode_enabled, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use ratatui::{layout::Rect, backend::CrosstermBackend, layout::Size};
use tokio::task::JoinHandle;
use tracing::{warn, debug};
use tokio::{
    select,
    sync::{mpsc, Mutex},
};

use crate::television::Television;
use crate::action::Action;

pub struct Tui<W>
where
    W: Write,
{
    pub task: JoinHandle<()>,
    pub frame_rate: f64,
    pub terminal: ratatui::Terminal<CrosstermBackend<W>>,
}

impl<W> Tui<W>
where
    W: Write,
{
    pub fn new(writer: W) -> Result<Self> {
        Ok(Self {
            task: tokio::spawn(async {}),
            frame_rate: 60.0,
            terminal: ratatui::Terminal::new(CrosstermBackend::new(writer))?,
        })
    }

    pub fn frame_rate(mut self, frame_rate: f64) -> Self {
        self.frame_rate = frame_rate;
        self
    }

    pub fn size(&self) -> Result<Size> {
        Ok(self.terminal.size()?)
    }

    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut buffered_stderr = LineWriter::new(stderr());
        execute!(buffered_stderr, EnterAlternateScreen)?;
        self.terminal.clear()?;
        execute!(buffered_stderr, cursor::Hide)?;
        Ok(())
    }

    pub fn exit(&mut self) -> Result<()> {
        if is_raw_mode_enabled()? {
            debug!("Exiting terminal");

            disable_raw_mode()?;
            let mut buffered_stderr = LineWriter::new(stderr());
            execute!(buffered_stderr, cursor::Show)?;
            execute!(buffered_stderr, LeaveAlternateScreen)?;
        }

        Ok(())
    }

    pub fn suspend(&mut self) -> Result<()> {
        self.exit()?;
        #[cfg(not(windows))]
        signal_hook::low_level::raise(signal_hook::consts::signal::SIGTSTP)?;
        Ok(())
    }

    pub fn resume(&mut self) -> Result<()> {
        self.enter()?;
        Ok(())
    }
}

impl<W> Deref for Tui<W>
where
    W: Write,
{
    type Target = ratatui::Terminal<CrosstermBackend<W>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl<W> DerefMut for Tui<W>
where
    W: Write,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl<W> Drop for Tui<W>
where
    W: Write,
{
    fn drop(&mut self) {
        match self.exit() {
            Ok(()) => debug!("Successfully exited terminal"),
            Err(e) => debug!("Failed to exit terminal: {:?}", e),
        }
    }
}


#[derive(Debug)]
pub enum RenderingTask {
    ClearScreen,
    Render,
    Resize(u16, u16),
    Resume,
    Suspend,
    Quit,
}

#[derive(Debug, Clone)]
enum IoStream {
    Stdout,
    BufferedStderr,
}

impl IoStream {
    fn to_stream(&self) -> Box<dyn std::io::Write + Send> {
        match self {
            IoStream::Stdout => Box::new(stdout()),
            IoStream::BufferedStderr => Box::new(LineWriter::new(stderr())),
        }
    }
}

pub async fn render(
    mut render_rx: mpsc::UnboundedReceiver<RenderingTask>,
    action_tx: mpsc::UnboundedSender<Action>,
    television: Arc<Mutex<Television>>,
    frame_rate: f64,
    is_output_tty: bool,
) -> Result<()> {
    let stream = if is_output_tty {
        debug!("Rendering to stdout");
        IoStream::Stdout.to_stream()
    } else {
        debug!("Rendering to stderr");
        IoStream::BufferedStderr.to_stream()
    };
    let mut tui = Tui::new(stream)?.frame_rate(frame_rate);

    debug!("Entering tui");
    tui.enter()?;

    debug!("Registering action handler");
    television
        .lock()
        .await.action_tx = Some(action_tx.clone());

    // Rendering loop
    loop {
        select! {
            () = tokio::time::sleep(tokio::time::Duration::from_secs_f64(1.0 / frame_rate)) => {
                action_tx.send(Action::Render)?;
            }
            maybe_task = render_rx.recv() => {
                if let Some(task) = maybe_task {
                    match task {
                        RenderingTask::ClearScreen => {
                            tui.terminal.clear()?;
                        }
                        RenderingTask::Render => {
                            let mut television = television.lock().await;
                            if let Ok(size) = tui.size() {
                                // Ratatui uses `u16`s to encode terminal dimensions and its
                                // content for each terminal cell is stored linearly in a
                                // buffer with a `u16` index which means we can't support
                                // terminal areas larger than `u16::MAX`.
                                if size.width.checked_mul(size.height).is_some() {
                                    tui.terminal.draw(|frame| {
                                        if let Err(err) = television.draw(frame, frame.area()) {
                                            warn!("Failed to draw: {:?}", err);
                                            let _ = action_tx
                                                .send(Action::Error(format!("Failed to draw: {err:?}")));
                                        }
                                    })?;

                                } else {
                                    warn!("Terminal area too large");
                                }
                            }
                        }
                        RenderingTask::Resize(w, h) => {
                            tui.resize(Rect::new(0, 0, w, h))?;
                            action_tx.send(Action::Render)?;
                        }
                        RenderingTask::Suspend => {
                            tui.suspend()?;
                            action_tx.send(Action::Resume)?;
                            action_tx.send(Action::ClearScreen)?;
                            tui.enter()?;
                        }
                        RenderingTask::Resume => {
                            tui.enter()?;
                        }
                        RenderingTask::Quit => {
                            debug!("Exiting rendering loop");
                            tui.exit()?;
                            break Ok(());
                        }
                    }
                }
            }
        }
    }
}
