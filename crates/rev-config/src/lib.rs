use std::path::Path;

pub trait AppConfig {}

#[derive(thiserror::Error, Debug)]
pub enum EnvError {
    #[error("any error: {0}")]
    EnvError(#[source] anyhow::Error),
}

pub trait Env {
    fn set_from_env(&mut self) -> Result<(), EnvError>;
}

#[derive(thiserror::Error, Debug)]
pub enum ConfigFileError {
    #[error("any error: {0}")]
    ConfigFileError(#[source] anyhow::Error),
}

pub trait ConfigFile {
    fn set_from_config_file(&mut self, config_file: &Path) -> Result<(), ConfigFileError>;
}
