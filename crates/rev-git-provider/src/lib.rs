use std::{ops::Deref, sync::Arc};

use github::{Github, GithubOptions};
use traits::{GitReview, GitUserReview};

pub trait Provider: GitUserReview + GitReview {}

#[derive(Clone)]
pub struct GitProvider {
    provider: Arc<dyn Provider + Send + Sync + 'static>,
}

impl GitProvider {
    pub fn github() -> anyhow::Result<Self> {
        let github = Arc::new(Github::new(GithubOptions::default())?);

        Ok(Self { provider: github })
    }
}

impl Deref for GitProvider {
    type Target = Arc<dyn Provider + Send + Sync + 'static>;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}

pub mod github;
pub mod models;
pub mod traits;

#[cfg(test)]
mod test {
    use tracing_test::traced_test;

    use crate::GitProvider;

    #[tokio::test]
    #[traced_test]
    async fn test_can_call_github() -> anyhow::Result<()> {
        let g = GitProvider::github()?;

        //let titles = g.get_user_reviews("kjuulh", &["dependencies"]).await?;
        let titles = g.get_user_reviews(None, None, None).await?;
        println!("title: {:#?}", titles);

        assert_ne!(0, titles.items.len());

        Ok(())
    }
}
