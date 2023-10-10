use std::collections::VecDeque;

use rev_git_provider::{models::ReviewListItem, GitProvider};
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct GitPullRequests {
    provider: GitProvider,
}

impl GitPullRequests {
    pub fn new(provider: GitProvider) -> Self {
        Self { provider }
    }

    async fn run_inner(
        &self,
        tx: mpsc::Sender<ReviewListItem>,
        owner: &str,
        tags: Option<Vec<String>>,
    ) -> anyhow::Result<()> {
        let mut buffer = VecDeque::new();
        let mut cursor = None;
        let mut has_more = true;
        let mut seen = 0;

        loop {
            if buffer.len() <= 3 && has_more {
                tracing::debug!("fetching more: len {}", buffer.len());
                let review_list = self
                    .provider
                    .get_user_reviews_cursor(
                        Some("lunarway/squad-aura"),
                        None,
                        tags.clone(),
                        cursor,
                    )
                    .await?;

                has_more = review_list.has_more;
                cursor = review_list.last_cursor;
                seen += review_list.items.len();
                tracing::debug!("get user reviews got items: {}", review_list.items.len());
                buffer.extend(review_list.items);

                if !has_more {
                    break;
                }
            }

            if seen > 100 {
                break;
            }

            if let Some(item) = buffer.pop_front() {
                if tx.send(item).await.is_err() {
                    break;
                }
            }
        }

        for item in buffer {
            if tx.send(item).await.is_err() {
                break;
            }
        }

        drop(tx);

        Ok(())
    }

    pub async fn run(
        &self,
        owner: &str,
        tags: Option<Vec<String>>,
    ) -> anyhow::Result<mpsc::Receiver<ReviewListItem>> {
        let s = self.clone();
        let (tx, rx) = tokio::sync::mpsc::channel::<ReviewListItem>(3);

        let owner = owner.to_string();

        tokio::spawn(async move {
            if let Err(e) = s.run_inner(tx, &owner, tags).await {
                tracing::error!("faced error: {e}");
            }
        });

        Ok(rx)
    }
}

// #[cfg(test)]
// mod test {
//     use rev_git_provider::{models::ReviewListItem, GitProvider};
//     use tracing_test::traced_test;

//     use crate::git_pull_requests::GitPullRequests;

//     #[tokio::test]
//     #[traced_test]
//     async fn test_can_fetch_many_prs() -> anyhow::Result<()> {
//         let mut prs = GitPullRequests::new(GitProvider::github()?);

//         let join = tokio::spawn(async move { while let Some(_item) = rx.recv().await {} });

//         prs.run(tx).await?;

//         join.await?;

//         Ok(())
//     }
// }
