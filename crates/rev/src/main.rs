mod action;
mod app;
mod application_config;
mod cli;
mod components;
mod config;
mod git_pull_requests;
mod logging;
mod page;
mod tui;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    logging::initialize_logging()?;

    tracing::debug!("starting app");

    cli::run().await?;

    Ok(())
}
