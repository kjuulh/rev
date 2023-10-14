use ratatui::{prelude::*, widgets::*};

use rev_widget_list::WidgetListItem;

#[derive(Debug, Clone)]
pub struct CommentItem<'a> {
    paragraph: Paragraph<'a>,
    height: u16,
}

impl CommentItem<'_> {
    pub fn new(author: &str, body: &str, height: u16) -> Self {
        let paragraph = Paragraph::new(body.to_string())
            .wrap(Wrap { trim: false })
            .style(Style::default().bg(Color::Black))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(author.to_string()),
            );

        let body_len = body.split("\n").collect::<Vec<_>>().len() as u16;
        Self {
            paragraph,
            height: body_len + height - 2,
        }
    }

    // Render the item differently depending on the selection state
    fn modify_fn(mut item: WidgetListItem<Self>, selected: Option<bool>) -> WidgetListItem<Self> {
        if let Some(selected) = selected {
            if selected {
                let style = Style::default().bg(Color::White);
                item.content.paragraph = item.content.paragraph.style(style);
            }
        }
        item
    }
}

impl<'a> From<CommentItem<'a>> for WidgetListItem<CommentItem<'a>> {
    fn from(val: CommentItem<'a>) -> Self {
        let height = val.height.to_owned();
        Self::new(val, height).modify_fn(CommentItem::modify_fn)
    }
}

impl<'a> Widget for CommentItem<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.paragraph.render(area, buf);
    }
}
