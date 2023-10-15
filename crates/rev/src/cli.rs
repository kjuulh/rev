use clap::{Parser, Subcommand};

use crate::{
    app::App, application_config::inner_application_config::InnerApplicationConfig, logging,
};

#[derive(Parser)]
#[command(author, version, about, long_about = None, subcommand_required = true)]
struct Command {
    #[command(subcommand)]
    command: Option<Commands>,

    #[clap(flatten)]
    global_args: InnerApplicationConfig,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Review,
    Config {
        #[command(subcommand)]
        subcommand: Option<ConfigCommand>,
    },
}

#[derive(Subcommand)]
enum ConfigCommand {
    Get,
    Validate,
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Command::parse();

    match cli.command.unwrap() {
        Commands::Init => {
            tracing::info!("hello rev");
        }
        Commands::Review => {
            logging::initialize_panic_handler()?;

            tracing::info!("starting tui");
            match App::default().register_pages().await {
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
        Commands::Config { subcommand } => match subcommand {
            Some(subcommand) => match subcommand {
                ConfigCommand::Get => todo!(),
                ConfigCommand::Validate => todo!(),
            },
            None => {
                tracing::debug!("getting config");
            }
        },
    }

    Ok(())
}
