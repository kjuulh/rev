use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct Review {
    pub id: String,
    pub number: usize,
    pub title: String,
    pub repository: String,
    pub description: String,
    pub author: String,
    pub publish_at: Option<DateTime<Utc>>,
    pub labels: Vec<String>,
    pub comments: Comments,
    pub status_checks: Vec<StatusCheck>,
}

#[derive(Debug, Clone)]
pub struct Comments {
    pub has_previous: bool,
    pub comments: Vec<Comment>,
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub author: String,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum StatusCheck {
    StatusContext {
        id: String,
        state: String,
        description: Option<String>,
        context: String,
    },
    CheckRun {
        id: String,
        name: String,
        status: String,
        conclusion: String,
    },
}

#[derive(Debug, Clone)]
pub struct ReviewListItem {
    pub id: String,
    pub name: String,
    pub title: String,
    pub owner: String,
    pub date: chrono::DateTime<chrono::Utc>,
    pub number: usize,
}

#[derive(Debug, Clone)]
pub struct ReviewList {
    pub items: Vec<ReviewListItem>,
    pub last_cursor: Option<String>,
    pub has_more: bool,
}
