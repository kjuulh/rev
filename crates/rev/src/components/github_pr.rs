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
            .constraints(vec![Constraint::Percentage(100), Constraint::Min(1)].as_ref())
            .split(area);

        if self.state == GitHubPrAction::DoneReview {
            f.render_widget(Paragraph::new(format!("done review")), layout[0]);
            return Ok(());
        }

        if let Some(pr) = &self.pr {
            f.render_widget(
                Paragraph::new(format!("github pr, {}", pr.title)),
                layout[0],
            );
        } else {
            f.render_widget(Paragraph::new("processing"), layout[0]);
        }

        f.render_widget(
            Paragraph::new("some text")
                .fg(Color::Black)
                .bg(Color::White),
            layout[1],
        );

        Ok(())
    }
}
