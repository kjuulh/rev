use ratatui::prelude::Rect;
use tokio::sync::mpsc;

use crate::{
    action::Action,
    components::{diff::GitDiff, home::Home, Component},
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
        //self.components.push(Box::new(Home::new()));
        self.components.push(Box::new(GitDiff::new()));

        Ok(self)
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        let (action_tx, mut action_rx) = mpsc::unbounded_channel();

        let mut tui = tui::Tui::new()?
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
            component.register_config_handler(self.config.clone())?;
        }

        for component in self.components.iter_mut() {
            component.init()?;
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

                for component in self.components.iter_mut() {
                    if let Some(action) = component.update(action.clone())? {
                        action_tx.send(action)?;
                    }
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
