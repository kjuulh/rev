use std::{ops::Deref, sync::Arc};

use github::{Github, GithubOptions};
use traits::{GitReview, GitUserReview};

pub trait Provider: GitUserReview + GitReview {}

pub struct GitProvider {
    provider: Arc<dyn Provider>,
}

impl GitProvider {
    pub fn github() -> anyhow::Result<Self> {
        let github = Arc::new(Github::new(GithubOptions::default())?);

        Ok(Self { provider: github })
    }
}

impl Deref for GitProvider {
    type Target = Arc<dyn Provider>;

    fn deref(&self) -> &Self::Target {
        &self.provider
    }
}

pub mod models {
    pub struct Review {}
}

pub mod traits {
    use async_trait::async_trait;

    use crate::models::Review;

    #[async_trait]
    pub trait GitUserReview {
        async fn get_user_reviews(&self, user: &str, tags: &[&str]) -> anyhow::Result<Vec<String>>;
    }

    #[async_trait]
    pub trait GitReview {
        async fn get_review(&self, lookup: String) -> anyhow::Result<Review>;
    }
}

pub mod github {
    use anyhow::Context;
    use async_trait::async_trait;
    use graphql_client::{GraphQLQuery, Response};
    use reqwest::Client;

    use crate::{
        models::Review,
        traits::{GitReview, GitUserReview},
        Provider,
    };

    use self::graphql::{user_repositories, UserRepositories};

    pub mod graphql {
        use graphql_client::GraphQLQuery;

        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "github/graphql/schema.graphql",
            query_path = "github/graphql/query.graphql"
        )]
        pub struct UserRepositories;
    }

    pub struct Github {
        github: octocrab::Octocrab,
        client: reqwest::Client,
    }

    pub struct GithubOptions {
        uri: String,
    }

    impl Default for GithubOptions {
        fn default() -> Self {
            Self {
                uri: "https://github.com".into(),
            }
        }
    }

    impl Github {
        pub fn new(options: GithubOptions) -> anyhow::Result<Self> {
            let octo = octocrab::Octocrab::builder()
                .base_uri(options.uri)?
                .add_header(http::header::ACCEPT, "application/json".into())
                .build()?;

            let client = Client::builder()
                .user_agent("graphql-rust/0.10.0")
                .default_headers(
                    std::iter::once((
                        reqwest::header::AUTHORIZATION,
                        reqwest::header::HeaderValue::from_str(&format!(
                            "Bearer {}",
                            std::env::var("GITHUB_API_TOKEN")
                                .context("GITHUB_API_TOKEN was not found")?
                        ))?,
                    ))
                    .collect(),
                )
                .build()?;

            Ok(Self {
                github: octo,
                client,
            })
        }
    }

    struct AggregateGraphQLError {
        errors: Vec<graphql_client::Error>,
    }

    impl std::fmt::Display for AggregateGraphQLError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "GitHub error: {:?}", self.errors)
        }
    }

    impl std::fmt::Debug for AggregateGraphQLError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "GitHub error: {:?}", self.errors)
        }
    }

    impl std::error::Error for AggregateGraphQLError {}

    #[async_trait]
    impl GitUserReview for Github {
        async fn get_user_reviews(&self, user: &str, tags: &[&str]) -> anyhow::Result<Vec<String>> {
            let vars = user_repositories::Variables { owner: user.into() };
            let query = UserRepositories::build_query(vars);

            let res = self
                .client
                .post("https://api.github.com/graphql")
                .json(&query)
                .send()
                .await?;
            let resp: Response<user_repositories::ResponseData> = res.json().await?;

            if let Some(errors) = resp.errors {
                let error = AggregateGraphQLError { errors };
                anyhow::bail!("get_user_reviews failed with: {}", error);
            }

            let repos = resp
                .data
                .iter()
                .flat_map(|d| d.user.as_ref())
                .flat_map(|u| u.pull_requests.nodes.as_ref())
                .flat_map(|n| n)
                .flat_map(|n| n)
                .map(|u| (u, &u.repository.owner.login))
                .collect::<Vec<_>>();

            Ok(repos.iter().map(|(r, _)| r.title.clone()).collect())
        }
    }

    #[async_trait]
    impl GitReview for Github {
        async fn get_review(&self, lookup: String) -> anyhow::Result<Review> {
            todo!()
        }
    }

    impl Provider for Github {}
}

#[cfg(test)]
mod test {
    use crate::GitProvider;

    #[tokio::test]
    async fn test_can_call_github() -> anyhow::Result<()> {
        let g = GitProvider::github()?;

        let titles = g.get_user_reviews("kjuulh", &[]).await?;
        for title in &titles {
            println!("title: {}", title);
        }

        assert_ne!(0, titles.len());
        todo!();

        Ok(())
    }
}
