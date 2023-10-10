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

use self::graphql::{pull_requests, user_repositories, PullRequests, UserRepositories};

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

    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "github/graphql/schema.graphql",
        query_path = "github/graphql/query.graphql",
        response_derives = "Clone,Debug"
    )]
    pub struct PullRequests;
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
                            let token = std::str::from_utf8(&o.stdout).ok().map(|s| s.to_string());
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
    async fn get_user_reviews(
        &self,
        requested: Option<&str>,
        org: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> anyhow::Result<ReviewList> {
        self.get_user_reviews_cursor(requested, org, tags, None)
            .await
    }

    async fn get_user_reviews_cursor(
        &self,
        requested: Option<&str>,
        org: Option<&str>,
        tags: Option<Vec<String>>,
        cursor: Option<String>,
    ) -> anyhow::Result<ReviewList> {
        let review_requested = match requested {
            Some(review) => match review.split_once('/') {
                Some((org, squad)) => format!("team-review-requested:{}/{}", org, squad),
                None => format!("review-requested:{}", review),
            },
            None => "review-requested:@me".into(),
        };

        let query = format!(
            "is:pr {} state:open {} {}",
            review_requested,
            org.map(|o| format!("org:{}", o)).unwrap_or("".into()),
            tags.map(|tags| format!("label:{}", tags.join(",")))
                .unwrap_or("".into())
        );

        let vars = pull_requests::Variables { cursor, query };
        let query = PullRequests::build_query(vars);

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

        let resp: Response<pull_requests::ResponseData> = res
            .json()
            .await
            .context("failed to get json from response")?;

        if let Some(errors) = resp.errors {
            let error = AggregateGraphQLError { errors };
            anyhow::bail!("get_user_reviews failed with: {}", error);
        }

        let prs = resp.data.context("data to be present")?.search;

        let repos = prs
            .nodes
            .context("nodes to be present")?
            .into_iter()
            .flatten()
            .filter_map(|pr| match pr {
                pull_requests::PullRequestsSearchNodes::PullRequest(pr) => Some(pr),
                _ => None,
            })
            .map(|pr| ReviewListItem {
                id: pr.id,
                name: pr.repository.name,
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
