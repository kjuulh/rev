use async_trait::async_trait;

use crate::models::{Review, ReviewList};

#[async_trait]
pub trait GitUserReview {
    async fn get_user_reviews(
        &self,
        requested: Option<&str>,
        org: Option<&str>,
        tags: Option<Vec<String>>,
    ) -> anyhow::Result<ReviewList>;
    async fn get_user_reviews_cursor(
        &self,
        requested: Option<&str>,
        org: Option<&str>,
        tags: Option<Vec<String>>,
        cursor: Option<String>,
    ) -> anyhow::Result<ReviewList>;
}

#[async_trait]
pub trait GitReview {
    async fn get_review(&self, lookup: String) -> anyhow::Result<Review>;
}
