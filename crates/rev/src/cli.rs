use clap::Parser;

use crate::{app::App, Command, Commands};

pub async fn run() -> anyhow::Result<()> {
    let cli = Command::parse();

    match cli.command.unwrap() {
        Commands::Init => {
            tracing::info!("hello rev");
        }
        Commands::Review => {
            tracing::info!("starting tui");
            match App::default().register_components().await {
                Ok(a) => {
                    if let Err(e) = a.run().await {
                        tracing::error!("{}", e);
                        return Err(e);
                    }
                }
                Err(e) => {
                    tracing::error!("{}", e);
                    return Err(e);
                }
            }
            tracing::info!("stopping tui");
        }
    }

    Ok(())
}
