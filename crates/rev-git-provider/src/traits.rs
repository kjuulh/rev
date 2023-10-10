use async_trait::async_trait;

use crate::models::{Review, ReviewList};

#[async_trait]
pub trait GitUserReview {
    async fn get_user_reviews(&self, user: &str, tags: &[&str]) -> anyhow::Result<ReviewList>;
    async fn get_user_reviews_cursor(
        &self,
        user: &str,
        tags: &[&str],
        cursor: Option<String>,
    ) -> anyhow::Result<ReviewList>;
}

#[async_trait]
pub trait GitReview {
    async fn get_review(&self, lookup: String) -> anyhow::Result<Review>;
}
