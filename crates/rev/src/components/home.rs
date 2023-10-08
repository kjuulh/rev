use super::Component;

use ratatui::{prelude::*, widgets::*};

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
            .constraints(
                [
                    Constraint::Percentage(100),
                    Constraint::Min(3),
                    Constraint::Min(1),
                ]
                .as_ref(),
            )
            .split(area);

        let main = Block::new()
            .style(Style::default().bg(Color::Red))
            .borders(Borders::ALL);
        let input = Block::new()
            .style(Style::default().bg(Color::Green))
            .borders(Borders::ALL);
        let help = Block::new().style(Style::default().bg(Color::Blue));

        f.render_widget(Paragraph::new("hello world one").block(main), rects[0]);
        f.render_widget(Paragraph::new("hello world two").block(input), rects[1]);
        f.render_widget(Paragraph::new("hello world three").block(help), rects[2]);
        Ok(())
    }
}
