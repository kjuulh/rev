use std::sync::{Arc, Mutex};

use ratatui::prelude::Rect;
use tokio::sync::mpsc;

use crate::{
    action::Action,
    components::{diff::GitDiff, home::Home},
    config::Config,
    tui,
};

use self::page::{Page, Pages};

mod page {
    use crate::{components::Component, tui::Frame};

    pub enum Pages {
        Home(Page),
        Diff(Page),
    }

    pub struct Page {
        pub name: String,
        pub components: Vec<Box<dyn Component>>,
    }

    impl Pages {
        pub fn apply(
            &mut self,
            apply_fn: impl Fn(&mut Box<dyn Component>) -> anyhow::Result<()>,
        ) -> anyhow::Result<()> {
            let page = match self {
                Pages::Home(page) => Some(page),
                Pages::Diff(page) => Some(page),
            };

            if let Some(page) = page {
                for c in page.components.iter_mut() {
                    apply_fn(c)?;
                }
            }

            Ok(())
        }

        pub fn draw(&mut self, frame: &mut Frame<'_>) -> anyhow::Result<()> {
            let page = match self {
                Pages::Home(p) => Some(p),
                Pages::Diff(p) => Some(p),
            };

            if let Some(page) = page {
                for c in page.components.iter_mut() {
                    c.draw(frame, frame.size())?;
                }
            }

            Ok(())
        }
    }
}

pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    should_quit: bool,
    pages: Vec<Arc<Mutex<Pages>>>,
    current_page: Option<Arc<Mutex<Pages>>>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Self {
        Self {
            tick_rate,
            frame_rate,
            config: Config::default(),
            should_quit: false,
            pages: Vec::new(),
            current_page: None,
        }
    }

    pub async fn register_pages(&mut self) -> anyhow::Result<&mut Self> {
        let home = Arc::new(Mutex::new(Pages::Home(Page {
            name: "home".into(),
            components: vec![Box::new(Home::new())],
        })));

        self.pages.push(home.clone());
        self.pages.push(Arc::new(Mutex::new(Pages::Diff(Page {
            name: "diff".into(),
            components: vec![Box::new(GitDiff::new())],
        }))));
        self.current_page = Some(home.clone());

        Ok(self)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        for page in self.pages.iter_mut() {
            let mut page = page.lock().unwrap();
            page.apply(|c| {
                c.register_action_handler(action_tx.clone())?;
                c.register_config_handler(self.config.clone())
            })?;
        }

        for page in self.pages.iter_mut() {
            let mut page = page.lock().unwrap();
            page.apply(|c| c.init())?;
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
                for page in self.pages.iter_mut() {
                    let mut page = page.lock().unwrap();
                    page.apply(|c| {
                        if let Some(action) = c.handle_events(Some(e.clone()))? {
                            action_tx.send(action)?;
                        }

                        Ok(())
                    })?;
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
                            if let Some(page) = self.current_page.clone() {
                                let mut page = page.lock().unwrap();
                                if let Err(e) = page.draw(f) {
                                    action_tx
                                        .send(Action::Error(format!("failed to draw {:?}", e)))
                                        .expect("to send error message");
                                }
                            }
                        })?;
                    }
                    Action::Suspend => todo!(),
                    Action::Resume => todo!(),
                    Action::Quit => self.should_quit = true,
                    Action::Render => {
                        tui.draw(|f| {
                            if let Some(page) = self.current_page.clone() {
                                let mut page = page.lock().unwrap();
                                if let Err(e) = page.draw(f) {
                                    action_tx
                                        .send(Action::Error(format!("failed to draw {:?}", e)))
                                        .expect("to send error message");
                                }
                            }
                        })?;
                    }
                    _ => {}
                }

                if let Some(page) = self.current_page.clone() {
                    let mut page = page.lock().unwrap();
                    page.apply(|c| {
                        if let Some(action) = c.update(action.clone())? {
                            action_tx.send(action)?;
                        }

                        Ok(())
                    })?;
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
        Self::new(10.0, 64.0)
    }
}
