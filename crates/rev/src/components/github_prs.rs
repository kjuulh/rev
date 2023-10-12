use std::sync::Arc;

use chrono::Utc;
use ratatui::{prelude::*, widgets::*};
use rev_git_provider::models::ReviewListItem;
use timeago::Formatter;
use tokio::sync::{
    mpsc::{Receiver, UnboundedSender},
    Mutex,
};

use crate::{
    action::{Action, GitHubPrAction},
    git_pull_requests::GitPullRequests,
};

use super::Component;

pub struct GithubPrs {
    prs_provider: GitPullRequests,
    action_tx: Option<UnboundedSender<Action>>,
    state: GitHubPrAction,
    prs: Option<Vec<ReviewListItem>>,
    table_state: TableState,
    prs_stream: Arc<Mutex<Option<Receiver<ReviewListItem>>>>,
}

impl GithubPrs {
    pub fn new(prs_provider: GitPullRequests) -> Self {
        Self {
            prs_provider,
            action_tx: None,
            state: GitHubPrAction::Normal,
            prs: None,
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
            let mut prs_res = Vec::new();

            if prs_stream.is_none() {
                *prs_stream = prs.run("kjuulh", None).await.ok();
            }

            if let Some(ref mut pr_stream) = *prs_stream {
                while let Some(pr) = pr_stream.recv().await {
                    prs_res.push(pr);
                    if prs_res.len() > 3 {
                        break;
                    }
                }
            }

            if !prs_res.is_empty() {
                tx.send(Action::GitHubPrs(GitHubPrAction::AddReviews {
                    items: prs_res,
                }))
                .unwrap();
            }
            tx.send(Action::GitHubPrs(GitHubPrAction::ExitProcessing))
                .unwrap();
        });
    }
}

impl Component for GithubPrs {
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
            Action::GotoPage(page) if page == "github_review_list" => {
                tracing::info!("schedule fetch");
                self.schedule_fetch()
            }
            Action::GitHubPrs(action) => {
                tracing::info!("received action: {:?}", action);
                match action {
                    GitHubPrAction::Normal => self.state = action,
                    GitHubPrAction::EnterProcessing => self.state = action,
                    GitHubPrAction::AddReviews { items } => {
                        if let Some(mut prs) = self.prs.take() {
                            prs.extend(items);
                            self.prs = Some(prs);
                        } else {
                            self.prs = Some(items);
                        }

                        if let Some(prs) = self.prs.as_ref() {
                            if prs.len() < 30 {
                                self.schedule_fetch();
                            }
                        }
                    }
                    GitHubPrAction::ExitProcessing => self.state = action,
                    GitHubPrAction::NextReview { .. } => {}
                    GitHubPrAction::DoneReview => {}
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

        if let Some(prs) = self.prs.as_ref() {
            let formatter = Formatter::default();

            let normal_style = Style::default();

            let header_cells = ["Owner", "Repository", "Title", "Date created"]
                .iter()
                .map(|h| Cell::from(*h).style(Style::default().fg(Color::White)));

            let header = Row::new(header_cells)
                .style(normal_style)
                .height(1)
                .bottom_margin(1);

            let rows = prs.iter().map(|item| {
                Row::new([
                    Cell::from(item.owner.clone()),
                    Cell::from(item.name.clone()),
                    Cell::from(item.title.clone()),
                    Cell::from(formatter.convert_chrono(item.date, Utc::now())),
                ])
                .height(1)
                .bottom_margin(1)
            });

            let t = Table::new(rows)
                .header(header)
                .column_spacing(3)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Github pull requests"),
                )
                .widths(&[
                    Constraint::Percentage(10),
                    Constraint::Percentage(15),
                    Constraint::Percentage(55),
                    Constraint::Percentage(20),
                ]);

            f.render_stateful_widget(t, layout[0], &mut self.table_state);
        } else {
            f.render_widget(Paragraph::new("processing"), layout[0])
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
