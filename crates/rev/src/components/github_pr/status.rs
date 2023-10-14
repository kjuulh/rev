use ratatui::{prelude::*, widgets::*};

use rev_git_provider::models::CurrentState;
use rev_widget_list::WidgetListItem;

#[derive(Clone, Debug)]
pub enum StatusCheckInput {
    Github(rev_git_provider::models::StatusCheck),
}

#[derive(Debug, Clone)]
pub struct StatusCheckItem<'a> {
    list: List<'a>,
    height: u16,
}

impl StatusCheckItem<'_> {
    pub fn new(input: StatusCheckInput, height: u16) -> Self {
        let block = Block::default().borders(Borders::ALL);

        fn get_state<'a>(current: CurrentState, state: String) -> Line<'a> {
            let style = match current {
                CurrentState::Success => Style::default().fg(Color::Green),
                CurrentState::Pending => Style::default().fg(Color::Yellow),
                CurrentState::Failure => Style::default().fg(Color::Red),
                CurrentState::Expired => Style::default().fg(Color::Blue),
            };
            Line::styled(state, style)
        }

        let list = match input {
            StatusCheckInput::Github(github) => match github {
                rev_git_provider::models::StatusCheck::StatusContext {
                    state,
                    description,
                    context,
                    current,
                    ..
                } => {
                    if let Some(desc) = description {
                        List::new(vec![
                            ListItem::new(Line::from(vec![desc.into()])),
                            ListItem::new(get_state(current, state.clone())),
                        ])
                        .block(block.title(context))
                    } else {
                        List::new(vec![
                            ListItem::new(Line::from(vec!["no description".into()])),
                            ListItem::new(get_state(current, state.clone())),
                        ])
                        .block(block.title(context))
                    }
                }
                rev_git_provider::models::StatusCheck::CheckRun {
                    name,
                    status,
                    conclusion,
                    current,
                    ..
                } => List::new(vec![
                    ListItem::new(vec![Line::from(vec![status.into()])]),
                    ListItem::new(get_state(current, conclusion.clone())),
                ])
                .block(block.title(name)),
            },
        };

        Self { list, height }
    }

    // Render the item differently depending on the selection state
    fn modify_fn(item: WidgetListItem<Self>, _selected: Option<bool>) -> WidgetListItem<Self> {
        item
    }
}

impl<'a> From<StatusCheckItem<'a>> for WidgetListItem<StatusCheckItem<'a>> {
    fn from(val: StatusCheckItem<'a>) -> Self {
        let height = val.height.to_owned();
        Self::new(val, height).modify_fn(StatusCheckItem::modify_fn)
    }
}

impl<'a> Widget for StatusCheckItem<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        ratatui::widgets::Widget::render(self.list, area, buf);
    }
}
