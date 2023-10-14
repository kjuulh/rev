mod action;
mod app;
mod cli;
mod components;
mod config;
mod git_pull_requests;
mod logging;
mod page;
mod tui;

mod application_config {
    use std::path::PathBuf;

    use rev_config::AppConfig;
    use rev_config_derive::AppConfig;

    #[derive(Clone, Debug)]
    pub struct ApplicationSettings {
        pub config_home: PathBuf,
        pub config_file_ext: String,
    }

    impl ApplicationSettings {
        pub fn new(config_home: PathBuf, config_file_ext: impl Into<String>) -> Self {
            Self {
                config_home,
                config_file_ext: config_file_ext.into(),
            }
        }
    }

    impl Default for ApplicationSettings {
        fn default() -> Self {
            let project_home = std::env::var("REV_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    let project = directories::ProjectDirs::from("io", "kjuulh", "rev")
                        .expect("to be able to find XDG home variables");

                    project.config_dir().to_path_buf()
                });

            Self::new(project_home, "kdl")
        }
    }

    #[derive(AppConfig, Clone, Debug)]
    pub struct InnerApplicationConfig {}

    #[derive(Clone, Debug)]
    pub struct ApplicationConfig {
        settings: ApplicationSettings,
        config: InnerApplicationConfig,
    }

    impl ApplicationConfig {
        pub async fn new(settings: ApplicationSettings) -> anyhow::Result<Self> {
            let config = Self::read_config().await?;

            Ok(Self { settings, config })
        }

        async fn read_config() -> anyhow::Result<InnerApplicationConfig> {
            todo!()
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    logging::initialize_logging()?;

    cli::run().await?;

    Ok(())
}
