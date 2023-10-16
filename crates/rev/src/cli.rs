use clap::{Parser, Subcommand};
use tokio::io::AsyncWriteExt;

use crate::{
    app::App,
    application_config::{inner_application_config::InnerApplicationConfig, ApplicationConfig},
    logging,
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
    Init {
        #[arg(long = "force", default_value = "false")]
        force: bool,
    },
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
        Commands::Init { force } => {
            let config = ApplicationConfig::new(cli.global_args).await?;

            let config_home_path = config.get_config_file_path();
            let config_file_path = config_home_path.join("rev.kdl");

            if config_file_path.exists() && !force {
                println!(
                    "config file already exists at: {}\nUse --force to override, be careful you may want to back up your config first",
                    config_file_path.display()
                );
                return Ok(());
            }

            tokio::fs::create_dir_all(config_home_path).await?;
            let mut file = tokio::fs::File::create(&config_file_path).await?;

            file.write_all(
                format!(
                    r#"config {{
    committer "{}"
}}"#,
                    config.committer
                )
                .as_bytes(),
            )
            .await?;

            println!("wrote config to: {}", config_file_path.display());
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

                let config = ApplicationConfig::new(cli.global_args).await?;

                dbg!(&config);
                dbg!(&config.get_config_file_path());
            }
        },
    }

    Ok(())
}
