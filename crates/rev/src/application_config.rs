use std::{ops::Deref, path::PathBuf};

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
pub struct InnerApplicationConfig {
    pub committer: String,
}

#[derive(Clone, Debug)]
pub struct ApplicationConfig {
    config: InnerApplicationConfig,
}

impl ApplicationConfig {
    pub async fn new(
        args: inner_application_config::InnerApplicationConfig,
    ) -> anyhow::Result<Self> {
        let config = InnerApplicationConfig::from(args)?;

        Ok(Self { config })
    }
}

impl Deref for ApplicationConfig {
    type Target = InnerApplicationConfig;

    fn deref(&self) -> &Self::Target {
        &self.config
    }
}

#[cfg(test)]
mod test {
    use kdl::KdlDocument;

    #[tokio::test]
    async fn test_can_parse_kdl() -> anyhow::Result<()> {
        let project = directories::ProjectDirs::from("io", "kjuulh", "rev")
            .expect("to be able to find XDG home variables");

        let path = project.config_dir().to_path_buf().join("rev.kdl");
        let file = tokio::fs::read_to_string(path).await?;

        let doc: KdlDocument = file.parse()?;

        let something = doc
            .get("config")
            .ok_or(anyhow::anyhow!("failed to find file"))?
            .children()
            .unwrap()
            .get("committer")
            .unwrap()
            .entries()
            .first()
            .unwrap()
            .value();
        dbg!(something);

        todo!();

        Ok(())
    }
}
