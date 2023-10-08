use std::{
    ops::{Deref, DerefMut},
    time::Duration,
};

use crossterm::{
    cursor,
    event::{DisableMouseCapture, EnableMouseCapture, KeyEvent, KeyEventKind, MouseEvent},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::{FutureExt, StreamExt};
use ratatui::prelude::CrosstermBackend;
use tokio::{
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum Event {
    Init,
    Quit,
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Error,
    FocusGained,
    FocusLost,
    Tick,
    Render,
}

pub type Frame<'a> = ratatui::Frame<'a, CrosstermBackend<std::io::Stdout>>;

pub struct Tui {
    pub terminal: ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
    pub task: JoinHandle<()>,
    pub cancellation_token: CancellationToken,
    pub event_rx: UnboundedReceiver<Event>,
    pub event_tx: UnboundedSender<Event>,
    pub frame_rate: f64,
    pub tick_rate: f64,
    pub mouse: bool,
}

impl Tui {
    pub fn new() -> anyhow::Result<Self> {
        let tick_rate = 4.0;
        let frame_rate = 60.0;
        let terminal = ratatui::Terminal::new(CrosstermBackend::new(std::io::stdout()))?;
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let cancellation_token = CancellationToken::new();
        let task = tokio::spawn(async {});
        let mouse = false;

        Ok(Self {
            terminal,
            task,
            cancellation_token,
            event_rx,
            event_tx,
            frame_rate,
            tick_rate,
            mouse,
        })
    }

    pub fn tick_rate(mut self, tick_rate: f64) -> Self {
        self.tick_rate = tick_rate;
        self
    }
    pub fn frame_rate(mut self, frame_rate: f64) -> Self {
        self.frame_rate = frame_rate;
        self
    }

    #[allow(dead_code)]
    pub fn mouse(mut self, mouse: bool) -> Self {
        self.mouse = mouse;
        self
    }

    pub fn enter(&mut self) -> anyhow::Result<()> {
        crossterm::terminal::enable_raw_mode()?;
        crossterm::execute!(std::io::stdout(), EnterAlternateScreen, cursor::Hide)?;
        if self.mouse {
            crossterm::execute!(std::io::stdout(), EnableMouseCapture)?;
        }

        self.start();

        Ok(())
    }

    pub fn start(&mut self) {
        let tick_delay = std::time::Duration::from_secs_f64(1.0 / self.tick_rate);
        let render_delay = std::time::Duration::from_secs_f64(1.0 / self.frame_rate);
        self.cancel();
        self.cancellation_token = CancellationToken::new();

        let cancellation_token = self.cancellation_token.clone();
        let event_tx = self.event_tx.clone();

        self.task = tokio::spawn(async move {
            let mut reader = crossterm::event::EventStream::new();
            let mut tick_interval = tokio::time::interval(tick_delay);
            let mut render_interval = tokio::time::interval(render_delay);

            event_tx.send(Event::Init).expect("event init to be sent");

            loop {
                let tick_delay = tick_interval.tick();
                let render_delay = render_interval.tick();
                let crossterm_event = reader.next().fuse();

                tokio::select! {
                    _ = cancellation_token.cancelled() => {
                        break;
                    }
                    _ = tick_delay => {
                        event_tx.send(Event::Tick).expect("to send tick event");
                    }
                    _ = render_delay => {
                        event_tx.send(Event::Render).expect("to send render event");
                    }
                    maybe_event = crossterm_event => {
                        match maybe_event {
                            Some(Ok(evt)) => {
                                match evt {
                                    crossterm::event::Event::FocusGained => { event_tx.send(Event::FocusGained).expect("to send event"); },
                                    crossterm::event::Event::FocusLost => { event_tx.send(Event::FocusLost).expect("to send event"); },
                                    crossterm::event::Event::Key(key) => {
                                        if key.kind == KeyEventKind::Press {
                                            event_tx.send(Event::Key(key)).expect("to send event");
                                        }
                                    },
                                    crossterm::event::Event::Mouse(mouse) => { event_tx.send(Event::Mouse(mouse)).expect("to send event"); },
                                    crossterm::event::Event::Paste(_s) => { },
                                    crossterm::event::Event::Resize(x, y) => { event_tx.send(Event::Resize(x, y)).expect("to send event"); },
                                }
                            },
                            Some(Err(_e)) => {
                                event_tx.send(Event::Error).expect("to send error event");
                            }
                            None => {},
                        }
                    }
                }
            }
        });
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.event_rx.recv().await
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.cancel();
        let mut counter = 0;
        while !self.task.is_finished() {
            std::thread::sleep(Duration::from_millis(1));
            counter += 1;
            if counter > 50 {
                self.task.abort();
            }
            if counter > 100 {
                tracing::error!("failed to abort task in 100 milliseconds for unknown reasons");
                break;
            }
        }

        Ok(())
    }

    pub fn exit(&mut self) -> anyhow::Result<()> {
        self.stop()?;
        if crossterm::terminal::is_raw_mode_enabled()? {
            self.flush()?;
            if self.mouse {
                crossterm::execute!(std::io::stdout(), DisableMouseCapture)?;
            }

            crossterm::execute!(std::io::stdout(), LeaveAlternateScreen, cursor::Show)?;
            crossterm::terminal::disable_raw_mode()?;
        }

        Ok(())
    }
}

impl Deref for Tui {
    type Target = ratatui::Terminal<CrosstermBackend<std::io::Stdout>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl DerefMut for Tui {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        self.exit().expect("to exit tui nicely");
    }
}
