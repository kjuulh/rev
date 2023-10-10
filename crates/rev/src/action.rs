#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Error(String),
    Help,
    GotoPage(String),
    GitHubPrs(GitHubPrAction),
    BeginReview,
    SkipReview,
}

#[derive(Debug, Clone)]
pub enum GitHubPrAction {
    Normal,
    EnterProcessing,
    AddReviews {
        items: Vec<rev_git_provider::models::ReviewListItem>,
    },
    ExitProcessing,
}

impl PartialEq for GitHubPrAction {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Normal, Self::Normal)
                | (Self::EnterProcessing, Self::EnterProcessing)
                | (Self::ExitProcessing, Self::ExitProcessing)
                | (
                    GitHubPrAction::AddReviews { .. },
                    GitHubPrAction::AddReviews { .. }
                )
        )
    }
}

impl Eq for GitHubPrAction {}
