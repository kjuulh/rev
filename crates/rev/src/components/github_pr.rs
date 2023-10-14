use std::sync::Arc;

use ratatui::{prelude::*, widgets::*};
use rev_git_provider::models::Review;
use rev_widget_list::{SelectableWidgetList, WidgetListItem};

use tokio::sync::{
    mpsc::{Receiver, UnboundedSender},
    Mutex,
};

use crate::{
    action::{Action, GitHubPrAction},
    git_pull_requests::GitPullRequest,
};

use super::Component;

pub struct GithubPr {
    vertical_scroll_state: ScrollbarState,
    prs_provider: GitPullRequest,
    action_tx: Option<UnboundedSender<Action>>,
    state: GitHubPrAction,
    pr: Option<Review>,
    prs_stream: Arc<Mutex<Option<Receiver<Review>>>>,
}

impl GithubPr {
    pub fn new(prs_provider: GitPullRequest) -> Self {
        Self {
            prs_provider,
            action_tx: None,
            state: GitHubPrAction::Normal,
            pr: None,
            prs_stream: Arc::default(),
            vertical_scroll_state: ScrollbarState::default(),
        }
    }

    fn schedule_fetch(&self) {
        let tx = self.action_tx.clone().unwrap();
        let prs = self.prs_provider.clone();
        let prs_stream = self.prs_stream.clone();
        tokio::spawn(async move {
            let mut prs_stream = prs_stream.lock().await;
            tx.send(Action::GitHubPrs(GitHubPrAction::EnterProcessing))
                .unwrap();
            if prs_stream.is_none() {
                *prs_stream = prs.run("kjuulh", None).await.ok();
            }

            if let Some(ref mut pr_stream) = *prs_stream {
                if let Some(pr) = pr_stream.recv().await {
                    tx.send(Action::GitHubPrs(GitHubPrAction::NextReview { pr }))
                        .unwrap();
                } else {
                    tx.send(Action::GitHubPrs(GitHubPrAction::DoneReview))
                        .unwrap();
                }
            }

            tx.send(Action::GitHubPrs(GitHubPrAction::ExitProcessing))
                .unwrap();
        });
    }
}

impl Component for GithubPr {
    fn register_action_handler(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<crate::action::Action>,
    ) -> anyhow::Result<()> {
        self.action_tx = Some(tx);

        Ok(())
    }

    fn update(
        &mut self,
        action: crate::action::Action,
    ) -> anyhow::Result<Option<crate::action::Action>> {
        match action {
            Action::GotoPage(page) if page == "github_review" => {
                tracing::info!("schedule fetch");
                self.schedule_fetch()
            }
            Action::SkipReview => self.schedule_fetch(),
            Action::GitHubPrs(action) => {
                tracing::info!("received action: {:?}", action);
                match action {
                    GitHubPrAction::Normal => self.state = action,
                    GitHubPrAction::EnterProcessing => self.state = action,
                    GitHubPrAction::AddReviews { .. } => {}
                    GitHubPrAction::ExitProcessing => self.state = action,
                    GitHubPrAction::NextReview { pr } => self.pr = Some(pr),
                    GitHubPrAction::DoneReview => {
                        self.prs_stream = Arc::default();
                        self.state = GitHubPrAction::Normal;
                        self.pr = None;

                        return Ok(Some(Action::GotoPage("github_review_list".to_string())));
                    }
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
        let layout = Layout::new()
            .constraints(vec![Constraint::Percentage(100), Constraint::Min(1)])
            .split(area);
        let block = Block::default().borders(Borders::ALL);

        if self.pr.is_none() {
            f.render_widget(Paragraph::new("processing"), layout[0]);
            return Ok(());
        }
        let pr = self.pr.as_ref().unwrap();
        let main = Layout::new()
            .constraints(vec![Constraint::Min(3), Constraint::Percentage(100)])
            .split(layout[0]);
        f.render_widget(
            Paragraph::new(format!("{} - #{}", &pr.repository, &pr.number)),
            main[0],
        );

        let body = Layout::new()
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .direction(Direction::Horizontal)
            .split(main[1]);

        let mut right_body_contraints = 0;
        let comment_list = {
            if pr.comments.comments.is_empty() {
                None
            } else {
                let comments_list_items = pr
                    .comments
                    .comments
                    .iter()
                    .map(|c| CommentItem::new(&c.author, &c.text, 4))
                    .collect::<Vec<_>>();

                let comments_list = SelectableWidgetList::new(comments_list_items)
                    .block(block.clone().title("comments"))
                    .truncate(true);

                right_body_contraints += 1;
                Some(comments_list)
            }
        };

        let status_checks_list = {
            if pr.status_checks.is_empty() {
                None
            } else {
                let checks_items = pr
                    .status_checks
                    .iter()
                    .map(|c| match c {
                        rev_git_provider::models::StatusCheck::StatusContext {
                            id: _,
                            state: _,
                            description,
                            context,
                        } => CommentItem::new(
                            context,
                            &description.clone().unwrap_or("".to_string()),
                            2,
                        ),
                        rev_git_provider::models::StatusCheck::CheckRun {
                            id: _,
                            name,
                            status: _,
                            conclusion,
                        } => CommentItem::new(name, conclusion, 2),
                    })
                    .collect::<Vec<_>>();

                let status_checks_list = SelectableWidgetList::new(checks_items)
                    .block(block.clone().title("status checks"))
                    .truncate(true);

                right_body_contraints += 1;
                Some(status_checks_list)
            }
        };

        let right_body = Layout::new()
            .constraints(
                (0..=right_body_contraints)
                    .map(|_| Constraint::Ratio(1, right_body_contraints))
                    .collect::<Vec<_>>(),
            )
            .direction(Direction::Vertical)
            .split(body[1]);

        tracing::info!(
            "len of right body: {}, status_checks {}, comments {}",
            right_body.len(),
            status_checks_list.is_some(),
            comment_list.is_some()
        );

        let description = body[0];
        //let statusChecks = rightBody[1];

        let mut next = 0;
        if let Some(mut comments_list) = comment_list {
            let comments = right_body[next];
            f.render_widget(&mut comments_list, comments);
            next += 1;
        }

        if let Some(mut status_checks_list) = status_checks_list {
            let status_checks = right_body[next];
            f.render_widget(&mut status_checks_list, status_checks);
            next += 1;
        }

        self.vertical_scroll_state = self
            .vertical_scroll_state
            .content_length(pr.description.len() as u16);
        f.render_widget(
            Paragraph::new(pr.description.as_str())
                .wrap(Wrap { trim: true })
                .block(block.title(pr.title.as_str())),
            description,
        );
        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            description,
            &mut self.vertical_scroll_state,
        );

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CommentItem<'a> {
    paragraph: Paragraph<'a>,
    height: u16,
}

impl CommentItem<'_> {
    pub fn new(author: &str, body: &str, height: u16) -> Self {
        let paragraph = Paragraph::new(body.to_string())
            .wrap(Wrap { trim: true })
            .style(Style::default().bg(Color::Black))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(author.to_string()),
            );

        Self { paragraph, height }
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
