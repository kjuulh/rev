use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None, subcommand_required = true)]
struct Command {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Review,
}

mod action;
mod app;
mod cli;
mod components;
mod config;
mod logging;
mod page;
mod tui;

mod git_pull_requests {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    logging::initialize_logging()?;
    logging::initialize_panic_handler()?;

    cli::run().await?;

    Ok(())
}
