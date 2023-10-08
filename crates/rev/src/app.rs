use ratatui::prelude::Rect;
use tokio::sync::mpsc;

use crate::{
    action::Action,
    components::{diff::GitDiff, home::Home},
    config::Config,
    page::Page,
    tui,
};

pub struct App {
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    should_quit: bool,
    pages: Vec<Page>,
    current_page: Option<String>,
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

    fn get_current_page(&mut self) -> Option<&mut Page> {
        if let Some(page) = self.current_page.as_ref() {
            return self.pages.iter_mut().find(|p| p.name() == page);
        }

        None
    }

    pub async fn register_pages(&mut self) -> anyhow::Result<&mut Self> {
        self.pages
            .push(Page::new("home", vec![Box::new(Home::new())]));
        self.pages
            .push(Page::new("diff", vec![Box::new(GitDiff::new())]));

        //self.current_page = Some(home.clone());
        self.current_page = Some("home".into());

        Ok(self)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        for page in self.pages.iter_mut() {
            page.apply(|c| {
                c.register_action_handler(action_tx.clone())?;
                c.register_config_handler(self.config.clone())
            })?;
        }

        for page in self.pages.iter_mut() {
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
                            if let Some(page) = self.get_current_page() {
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
                            if let Some(page) = self.get_current_page() {
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

                if let Some(page) = self.get_current_page() {
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
