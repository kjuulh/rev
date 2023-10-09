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
    #[derive(Debug, Clone)]
    pub struct Review {}

    #[derive(Debug, Clone)]
    pub struct ReviewListItem {
        pub id: String,
        pub title: String,
        pub owner: String,
        pub date: chrono::DateTime<chrono::Utc>,
    }

    #[derive(Debug, Clone)]
    pub struct ReviewList {
        pub items: Vec<ReviewListItem>,
        pub last_cursor: Option<String>,
        pub has_more: bool,
    }
}

pub mod traits {
    use async_trait::async_trait;

    use crate::models::{Review, ReviewList};

    #[async_trait]
    pub trait GitUserReview {
        async fn get_user_reviews(&self, user: &str, tags: &[&str]) -> anyhow::Result<ReviewList>;
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
    use which::which;

    use crate::{
        models::{Review, ReviewList, ReviewListItem},
        traits::{GitReview, GitUserReview},
        Provider,
    };

    use self::graphql::{user_repositories, UserRepositories};

    pub mod graphql {
        use graphql_client::GraphQLQuery;

        pub type DateTime = chrono::DateTime<chrono::Utc>;

        #[derive(GraphQLQuery)]
        #[graphql(
            schema_path = "github/graphql/schema.graphql",
            query_path = "github/graphql/query.graphql",
            response_derives = "Clone,Debug"
        )]
        pub struct UserRepositories;
    }

    pub struct Github {
        client: reqwest::Client,
        uri: String,
    }

    pub struct GithubOptions {
        uri: String,
        use_gh: bool,
    }

    impl Default for GithubOptions {
        fn default() -> Self {
            Self {
                uri: "https://api.github.com/graphql".into(),
                use_gh: true,
            }
        }
    }

    impl Github {
        pub fn new(options: GithubOptions) -> anyhow::Result<Self> {
            let token = if options.use_gh {
                let token = which("gh")
                    .ok()
                    .filter(|p| {
                        if p.exists() {
                            tracing::debug!("gh is on path");
                            true
                        } else {
                            tracing::debug!("gh is not on path");
                            false
                        }
                    })
                    .and_then(|p| {
                        let output = std::process::Command::new(p)
                            .arg("auth")
                            .arg("token")
                            .output()
                            .ok()
                            .filter(|o| o.status.success())
                            .and_then(|o| {
                                let token =
                                    std::str::from_utf8(&o.stdout).ok().map(|s| s.to_string());
                                if token.is_some() {
                                    tracing::trace!("found github token using gh");
                                }
                                token
                            })
                            .map(|s| s.trim().to_string());

                        output
                    });
                token
            } else {
                None
            };

            let client = Client::builder()
                .user_agent("graphql-rust/0.10.0")
                .default_headers(
                    std::iter::once((
                        reqwest::header::AUTHORIZATION,
                        reqwest::header::HeaderValue::from_str(&format!(
                            "Bearer {}",
                            token.unwrap_or_else(|| {
                                tracing::debug!("falling back on GITHUB_API_TOKEN");

                                std::env::var("GITHUB_API_TOKEN")
                                    .context("GITHUB_API_TOKEN was not found")
                                    .unwrap()
                            })
                        ))?,
                    ))
                    .collect(),
                )
                .build()?;

            Ok(Self {
                client,
                uri: options.uri,
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
        async fn get_user_reviews(&self, user: &str, tags: &[&str]) -> anyhow::Result<ReviewList> {
            let vars = user_repositories::Variables {
                owner: user.into(),
                labels: if tags.is_empty() {
                    None
                } else {
                    Some(tags.iter().map(|t| t.to_string()).collect())
                },
            };
            let query = UserRepositories::build_query(vars);

            let res = self
                .client
                .post(&self.uri)
                .json(&query)
                .send()
                .await
                .context("github call graphql query failed")?;

            if !res.status().is_success() {
                let error_body = res.text().await?;
                tracing::error!("GraphQL Error: {}", error_body);
                anyhow::bail!("failed to query graphql endpoint");
            }

            let resp: Response<user_repositories::ResponseData> = res
                .json()
                .await
                .context("failed to get json from response")?;

            if let Some(errors) = resp.errors {
                let error = AggregateGraphQLError { errors };
                anyhow::bail!("get_user_reviews failed with: {}", error);
            }

            let prs = resp
                .data
                .context("data to be present")?
                .user
                .context("user to be present")?
                .pull_requests;

            let repos = prs
                .nodes
                .context("nodes to be present")?
                .into_iter()
                .flatten()
                .map(|pr| ReviewListItem {
                    id: pr.id,
                    title: pr.title,
                    owner: pr.repository.owner.login,
                    date: pr.created_at,
                })
                .collect::<Vec<_>>();

            Ok(ReviewList {
                items: repos,
                last_cursor: prs.page_info.end_cursor,
                has_more: prs.page_info.has_next_page,
            })
        }
    }

    #[async_trait]
    impl GitReview for Github {
        async fn get_review(&self, _lookup: String) -> anyhow::Result<Review> {
            todo!()
        }
    }

    impl Provider for Github {}
}

#[cfg(test)]
mod test {
    use tracing_test::traced_test;

    use crate::GitProvider;

    #[tokio::test]
    #[traced_test]
    async fn test_can_call_github() -> anyhow::Result<()> {
        let g = GitProvider::github()?;

        //let titles = g.get_user_reviews("kjuulh", &["dependencies"]).await?;
        let titles = g.get_user_reviews("kjuulh", &[]).await?;
        println!("title: {:#?}", titles);

        assert_ne!(0, titles.items.len());

        Ok(())
    }
}
