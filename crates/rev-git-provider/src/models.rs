#[derive(Debug, Clone)]
pub struct Review {}

#[derive(Debug, Clone)]
pub struct ReviewListItem {
    pub id: String,
    pub name: String,
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
