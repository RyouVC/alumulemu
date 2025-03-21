//! Config module for alumulemu

use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Debug, Clone, Default)]
#[clap(rename_all = "lowercase")]
pub enum DatabaseAuthMethod {
    #[default]
    None,
    Root,
}

#[derive(Parser, Debug, Clone)]
pub struct DatabaseConfig {
    #[clap(env = "ALU_DATABASE_URL", default_value = "rocksdb://./database")]
    pub database_url: String,

    #[clap(env = "ALU_DATABASE_AUTH_METHOD", value_enum, default_value = "none")]
    pub db_auth_method: DatabaseAuthMethod,

    #[clap(env = "ALU_SURREAL_ROOT_USERNAME", default_value = "root")]
    #[clap(required_if_eq("db_auth_method", "root"))]
    pub root_username: Option<String>,

    #[clap(env = "ALU_SURREAL_ROOT_PASSWORD", default_value = "root")]
    #[clap(required_if_eq("db_auth_method", "root"))]
    pub root_password: Option<String>,

    #[clap(env = "ALU_SURREAL_NAMESPACE", default_value = "alemulemu")]
    pub db_namespace: String,

    #[clap(env = "ALU_SURREAL_DATABASE", default_value = "alemulemu")]
    pub db_database: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BackendConfig {
    /// Primary region for metadata to be pulled from
    #[clap(env = "ALU_PRIMARY_REGION", default_value = "US")]
    pub primary_region: String,

    /// Primary language for metadata to be pulled from
    #[clap(env = "ALU_SECONDARY_LANGUAGE", default_value = "en")]
    pub primary_lang: String,

    /// Directory to store games
    #[clap(env = "ALU_ROM_DIR", default_value = "games/")]
    pub rom_dir: String,
}
#[derive(Parser, Debug, Clone)]
pub struct Config {
    #[clap(env = "ALU_HOST", default_value = "0.0.0.0:3000")]
    pub host: String,

    #[clap(flatten)]
    pub db_config: DatabaseConfig,

    #[clap(flatten)]
    pub backend_config: BackendConfig,
}

pub fn config() -> Config {
    Config::parse()
}
