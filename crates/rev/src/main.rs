use std::net::SocketAddr;

use app::App;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None, subcommand_required = true)]
struct Command {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Review,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = Command::parse();

    match cli.command.unwrap() {
        Commands::Init => {
            tracing::info!("hello rev");
        }
        Commands::Review => {
            tracing::info!("starting tui");
            App::default().register_components().await?.run().await?;
            tracing::info!("stopping tui");
        }
    }

    Ok(())
}

mod tui {
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
    use ratatui::prelude::{CrosstermBackend, Rect};
    use tokio::{
        sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
        task::JoinHandle,
    };
    use tokio_util::sync::CancellationToken;

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
                                Some(Err(_)) => {
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
}

mod action {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Action {
        Tick,
        Render,
        Resize(u16, u16),
        Suspend,
        Resume,
        Quit,
        Refresh,
        Error(String),
        Help,
    }
}

mod config {
    use std::{
        collections::HashMap,
        ops::{Deref, DerefMut},
    };

    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    use crate::action::Action;

    #[derive(Debug, Clone)]
    pub struct Config {
        pub keybinds: Keybinds,
    }

    impl Default for Config {
        fn default() -> Self {
            Self {
                keybinds: Keybinds::default(),
            }
        }
    }

    pub type InnerKeybinds = HashMap<Vec<KeyEvent>, Action>;

    #[derive(Clone, Debug)]
    pub struct Keybinds(pub InnerKeybinds);

    impl Deref for Keybinds {
        type Target = InnerKeybinds;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for Keybinds {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl Default for Keybinds {
        fn default() -> Self {
            let mut keybinds = HashMap::new();
            keybinds.insert(vec![parse_key_event("q").unwrap()], Action::Quit);

            Self(keybinds)
        }
    }

    fn parse_key_event(raw: &str) -> anyhow::Result<KeyEvent> {
        let raw_lower = raw.to_ascii_lowercase();

        let e = match &raw_lower {
            c if c.len() == 1 => {
                let c = c.chars().next().expect("to get next key code");
                KeyCode::Char(c)
            }
            _ => anyhow::bail!("Unable to parse {raw_lower}"),
        };

        Ok(KeyEvent::new(e, KeyModifiers::empty()))
    }
}

mod app {
    use ratatui::prelude::Rect;
    use tokio::sync::mpsc;

    use crate::{
        action::Action,
        components::{home::Home, Component},
        config::Config,
        tui,
    };

    pub struct App {
        config: Config,
        tick_rate: f64,
        frame_rate: f64,
        should_quit: bool,
        components: Vec<Box<dyn Component>>,
    }

    impl App {
        pub fn new(tick_rate: f64, frame_rate: f64) -> Self {
            Self {
                tick_rate,
                frame_rate,
                config: Config::default(),
                should_quit: false,
                components: Vec::new(),
            }
        }

        pub async fn register_components(&mut self) -> anyhow::Result<&mut Self> {
            self.components.push(Box::new(Home::new()));

            Ok(self)
        }

        pub async fn run(&mut self) -> anyhow::Result<()> {
            let (action_tx, mut action_rx) = mpsc::unbounded_channel();

            let mut tui = tui::Tui::new()?
                .tick_rate(self.tick_rate)
                .frame_rate(self.frame_rate);
            tui.enter()?;

            for component in self.components.iter_mut() {
                component.register_action_handler(action_tx.clone());
                component.register_config_handler(self.config.clone());
            }

            for component in self.components.iter_mut() {
                component.init(tui.size()?)?;
            }

            loop {
                if let Some(e) = tui.next().await {
                    match e {
                        tui::Event::Quit => action_tx.send(Action::Quit)?,
                        tui::Event::Key(key) => {
                            if let Some(action) = self.config.keybinds.get(&vec![key]) {
                                tracing::info!("got action: {action:?}");
                                action_tx.send(action.clone())?;
                            }
                        }
                        tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                        tui::Event::Tick => action_tx.send(Action::Tick)?,
                        tui::Event::Render => action_tx.send(Action::Render)?,
                        _ => {}
                    }
                    for component in self.components.iter_mut() {
                        if let Some(action) = component.handle_events(Some(e.clone()))? {
                            action_tx.send(action)?;
                        }
                    }
                }

                while let Ok(action) = action_rx.try_recv() {
                    if action != Action::Tick && action != Action::Render {
                        tracing::debug!("{action:?}");
                    }

                    match action {
                        Action::Resize(x, y) => {
                            tui.resize(Rect::new(0, 0, x, y))?;
                            tui.draw(|f| {
                                for component in self.components.iter_mut() {
                                    let r = component.draw(f, f.size());
                                    if let Err(e) = r {
                                        action_tx
                                            .send(Action::Error(format!("failed to draw {:?}", e)))
                                            .expect("failed to send error event");
                                    }
                                }
                            })?;
                        }
                        Action::Suspend => todo!(),
                        Action::Resume => todo!(),
                        Action::Quit => self.should_quit = true,
                        Action::Render => {
                            tui.draw(|f| {
                                for component in self.components.iter_mut() {
                                    let r = component.draw(f, f.size());
                                    if let Err(e) = r {
                                        action_tx
                                            .send(Action::Error(format!("failed to draw {:?}", e)))
                                            .expect("failed to send error event");
                                    }
                                }
                            })?;
                        }
                        _ => {}
                    }
                }

                if self.should_quit {
                    tui.stop()?;
                    break;
                }
            }

            tui.exit()?;

            Ok(())
        }
    }

    impl Default for App {
        fn default() -> Self {
            Self::new(1.0, 4.0)
        }
    }
}

mod components {
    use crossterm::event::{KeyEvent, MouseEvent};
    use ratatui::layout::Rect;
    use tokio::sync::mpsc::UnboundedSender;

    use crate::{
        action::Action,
        config::Config,
        tui::{Event, Frame},
    };

    #[allow(unused_variables)]
    pub trait Component {
        fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> anyhow::Result<()> {
            Ok(())
        }

        fn register_config_handler(&mut self, config: Config) -> anyhow::Result<()> {
            Ok(())
        }

        fn init(&mut self, area: Rect) -> anyhow::Result<()> {
            Ok(())
        }

        fn handle_events(&mut self, event: Option<Event>) -> anyhow::Result<Option<Action>> {
            let r = match event {
                Some(Event::Key(key_event)) => self.handle_key_events(key_event)?,
                Some(Event::Mouse(mouse_event)) => self.handle_mouse_events(mouse_event)?,
                _ => None,
            };

            Ok(r)
        }

        fn handle_key_events(&mut self, key: KeyEvent) -> anyhow::Result<Option<Action>> {
            Ok(None)
        }

        fn handle_mouse_events(&mut self, mouse: MouseEvent) -> anyhow::Result<Option<Action>> {
            Ok(None)
        }

        fn update(&mut self, action: Action) -> anyhow::Result<Option<Action>> {
            Ok(None)
        }

        fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> anyhow::Result<()>;
    }

    pub mod home {
        use ratatui::widgets::Paragraph;

        use super::Component;

        pub struct Home {}

        impl Home {
            pub fn new() -> Self {
                Self {}
            }
        }

        impl Component for Home {
            fn draw(
                &mut self,
                f: &mut crate::tui::Frame<'_>,
                area: ratatui::prelude::Rect,
            ) -> anyhow::Result<()> {
                f.render_widget(Paragraph::new("hello world"), area);
                Ok(())
            }
        }
    }
}
