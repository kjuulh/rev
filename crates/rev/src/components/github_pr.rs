use std::sync::Arc;

use chrono::Utc;
use ratatui::{prelude::*, widgets::*};
use rev_git_provider::models::{Review, ReviewListItem};
use timeago::Formatter;
use tokio::sync::{
    mpsc::{Receiver, UnboundedSender},
    Mutex,
};

use crate::{
    action::{Action, GitHubPrAction},
    git_pull_requests::{GitPullRequest, GitPullRequests},
};

use super::Component;

pub struct GithubPr {
    vertical_scroll_state: ScrollbarState,
    prs_provider: GitPullRequest,
    action_tx: Option<UnboundedSender<Action>>,
    state: GitHubPrAction,
    pr: Option<Review>,
    table_state: TableState,
    prs_stream: Arc<Mutex<Option<Receiver<Review>>>>,
}

impl GithubPr {
    pub fn new(prs_provider: GitPullRequest) -> Self {
        Self {
            prs_provider,
            action_tx: None,
            state: GitHubPrAction::Normal,
            pr: None,
            table_state: TableState::default(),
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

        let rightBody = Layout::new()
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .direction(Direction::Vertical)
            .split(body[1]);

        let description = body[0];
        let comments = rightBody[0];
        let statusChecks = rightBody[1];

        let comments_list_items = pr
            .comments
            .comments
            .iter()
            .map(|c| Paragraph::new(c.text).block(block.title(c.author)))
            .collect::<Vec<_>>();

        let comments_list = List::new(comments_list_items);

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
