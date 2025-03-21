//! Config module for alumulemu
use std::default;

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
pub struct Config {
    #[clap(env = "ALU_HOST", default_value = "0.0.0.0:3000")]
    pub host: String,

    #[clap(flatten)]
    pub db_config: DatabaseConfig,
}

pub fn config() -> Config {
    Config::parse()
}
