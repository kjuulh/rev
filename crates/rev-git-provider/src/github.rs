use anyhow::Context;
use async_trait::async_trait;
use graphql_client::{GraphQLQuery, Response};
use reqwest::Client;
use which::which;

use crate::{
    models::{Comment, Comments, Review, ReviewList, ReviewListItem, StatusCheck},
    traits::{GitReview, GitUserReview},
    Provider,
};

use self::graphql::{
    pull_request::{
        self, CheckConclusionState, CheckStatusState,
        PullRequestRepositoryPullRequestCommitsNodesCommitStatusCheckRollupContextsNodes,
    },
    pull_requests, PullRequest, PullRequests,
};

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

    #[derive(GraphQLQuery)]
    #[graphql(
        schema_path = "github/graphql/schema.graphql",
        query_path = "github/graphql/query.graphql",
        response_derives = "Clone,Debug"
    )]
    pub struct PullRequest;
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
                number: pr.number as usize,
            })
            .collect::<Vec<_>>();

        Ok(ReviewList {
            items: repos,
            last_cursor: prs.page_info.end_cursor,
            has_more: prs.page_info.has_next_page,
        })
    }
}

type StatusChecks =
    PullRequestRepositoryPullRequestCommitsNodesCommitStatusCheckRollupContextsNodes;

#[async_trait]
impl GitReview for Github {
    async fn get_review(
        &self,
        owner: String,
        name: String,
        number: usize,
    ) -> anyhow::Result<Option<Review>> {
        let vars = pull_request::Variables {
            owner,
            name,
            number: number as i64,
        };
        let query = PullRequest::build_query(vars);

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

        let resp: Response<pull_request::ResponseData> = res
            .json()
            .await
            .context("failed to get json from response")?;

        if let Some(errors) = resp.errors {
            let error = AggregateGraphQLError { errors };
            anyhow::bail!("get_user_reviews failed with: {}", error);
        }

        let repository = resp.data.context("data to be present")?.repository;
        let repository = match repository {
            Some(pr) => pr,
            None => return Ok(None),
        };

        let pr = match repository.pull_request {
            Some(pr) => pr,
            None => return Ok(None),
        };

        Ok(Some(Review {
            id: pr.id,
            number: pr.number as usize,
            title: pr.title,
            author: pr.author.map(|a| a.login).unwrap_or("ghost".to_string()),
            publish_at: pr.published_at,
            labels: pr
                .labels
                .into_iter()
                .filter_map(|l| l.nodes)
                .flat_map(|n| {
                    n.iter()
                        .flatten()
                        .map(|n| n.name.clone())
                        .collect::<Vec<_>>()
                })
                .collect(),
            comments: Comments {
                has_previous: pr.comments.page_info.has_previous_page,
                comments: pr
                    .comments
                    .nodes
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|n| Comment {
                        author: n.author.map(|a| a.login).unwrap_or("ghost".to_string()),
                        text: n.body_text,
                    })
                    .collect(),
            },
            status_checks: pr
                .commits
                .nodes
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|n| n.commit.status_check_rollup)
                .filter_map(|n| n.contexts.nodes)
                .flatten()
                .flatten()
                .map(|c| match c {
                    StatusChecks::CheckRun(c) => StatusCheck::CheckRun {
                        id: c.id,
                        name: c.name,
                        status: {
                            let status_name = match c.status {
                                CheckStatusState::COMPLETED => "completed",
                                CheckStatusState::IN_PROGRESS => "in progress",
                                CheckStatusState::PENDING => "pending",
                                CheckStatusState::QUEUED => "queued",
                                CheckStatusState::REQUESTED => "requested",
                                CheckStatusState::WAITING => "waiting",
                                CheckStatusState::Other(ref e) => e,
                            };
                            status_name.to_string()
                        },
                        conclusion: c
                            .conclusion
                            .map(|c| {
                                let conclusion = match c {
                                    CheckConclusionState::ACTION_REQUIRED => "action required",
                                    CheckConclusionState::CANCELLED => "cancelled",
                                    CheckConclusionState::FAILURE => "failure",
                                    CheckConclusionState::NEUTRAL => "neutral",
                                    CheckConclusionState::SKIPPED => "skipped",
                                    CheckConclusionState::STALE => "stale",
                                    CheckConclusionState::STARTUP_FAILURE => "startup failure",
                                    CheckConclusionState::SUCCESS => "success",
                                    CheckConclusionState::TIMED_OUT => "timed out",
                                    CheckConclusionState::Other(ref o) => o,
                                };
                                conclusion.to_string()
                            })
                            .unwrap_or("unknown".to_string()),
                    },
                    StatusChecks::StatusContext(sc) => StatusCheck::StatusContext {
                        id: sc.id,
                        state: match sc.state {
                            pull_request::StatusState::ERROR => "error",
                            pull_request::StatusState::EXPECTED => "expected",
                            pull_request::StatusState::FAILURE => "failure",
                            pull_request::StatusState::PENDING => "pending",
                            pull_request::StatusState::SUCCESS => "succeess",
                            pull_request::StatusState::Other(ref o) => o,
                        }
                        .to_string(),
                        description: sc.description,
                        context: sc.context,
                    },
                })
                .collect(),
        }))
    }
}

impl Provider for Github {}
