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

    fn init(&mut self) -> anyhow::Result<()> {
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
    use ratatui::{
        prelude::{Constraint, Layout},
        widgets::Paragraph,
    };

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
            let rects = Layout::default()
                .constraints([Constraint::Percentage(100), Constraint::Min(3)].as_ref())
                .split(area);

            f.render_widget(Paragraph::new("hello world one"), rects[0]);
            f.render_widget(Paragraph::new("hello world two"), rects[1]);
            Ok(())
        }
    }
}

pub mod diff {
    use std::sync::{Arc, RwLock};

    use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
    use ratatui::{
        style::{Modifier, Style},
        text::Line,
        widgets::{Block, Borders},
    };
    use tui_term::widget::PseudoTerminal;

    use super::Component;

    pub struct GitDiff {
        cmd: CommandBuilder,
        pty_system: NativePtySystem,
        parser: Option<Arc<RwLock<vt100::Parser>>>,
        scrollback: u64,
    }

    impl GitDiff {
        pub fn new() -> Self {
            let pty_system = NativePtySystem::default();
            let cwd = std::env::current_dir().unwrap();
            let mut cmd = CommandBuilder::new("bash");
            cmd.arg("-c");
            cmd.arg("git --no-pager diff | delta --paging=never");
            cmd.cwd(cwd);

            Self {
                cmd,
                pty_system,
                parser: None,
                scrollback: 0,
            }
        }
    }

    impl Component for GitDiff {
        fn update(
            &mut self,
            action: crate::action::Action,
        ) -> anyhow::Result<Option<crate::action::Action>> {
            match action {
                crate::action::Action::Tick => {
                    tracing::info!("tickle me");

                    if let Some(parser) = self.parser.clone() {
                        let mut parser = parser.write().unwrap();
                        self.scrollback += 1;
                        //self.scrollback = self.scrollback % 999;
                        parser.set_scrollback(self.scrollback as usize);
                    }
                }
                _ => {}
            }

            Ok(None)
        }

        fn draw(
            &mut self,
            f: &mut crate::tui::Frame<'_>,
            area: ratatui::prelude::Rect,
        ) -> anyhow::Result<()> {
            match self.parser.as_ref() {
                Some(parser) => {
                    let screen = parser.read().unwrap();
                    let screen = screen.screen();

                    let block = Block::default()
                        .borders(Borders::ALL)
                        .title(Line::from("[ Running: git diff ]"))
                        .style(Style::default().add_modifier(Modifier::BOLD));
                    let pseudo_term = PseudoTerminal::new(screen).block(block.clone());
                    f.render_widget(pseudo_term, area);
                    f.render_widget(block, f.size())
                }
                None => {
                    let pair = self.pty_system.openpty(PtySize {
                        rows: area.height,
                        cols: area.width,
                        pixel_width: 0,
                        pixel_height: 0,
                    })?;

                    let mut child = pair.slave.spawn_command(self.cmd.clone())?;
                    drop(pair.slave);

                    let mut reader = pair.master.try_clone_reader()?;
                    let parser = Arc::new(RwLock::new(vt100::Parser::new(
                        area.height - 1,
                        area.width - 1,
                        1000,
                    )));

                    {
                        let parser = parser.clone();
                        std::thread::spawn(move || {
                            let mut s = String::new();
                            reader.read_to_string(&mut s).unwrap();
                            if !s.is_empty() {
                                let mut parser = parser.write().unwrap();
                                parser.process(s.as_bytes());
                            }
                        });
                    }

                    {
                        let _writer = pair.master.take_writer()?;
                    }

                    let _child_exit_status = child.wait()?;

                    drop(pair.master);

                    self.parser = Some(parser);

                    return self.draw(f, area);
                }
            }
            Ok(())
        }
    }
}
