//! Config module for alumulemu

use std::path::PathBuf;

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

    #[clap(env = "ALU_SURREAL_NAMESPACE", default_value = "alumulemu")]
    pub db_namespace: String,

    #[clap(env = "ALU_SURREAL_DATABASE", default_value = "alumulemu")]
    pub db_database: String,
}

#[derive(Parser, Debug, Clone)]
pub struct BackendConfig {
    /// Primary region for metadata to be pulled from
    #[clap(env = "ALU_PRIMARY_REGION", default_value = "US")]
    pub primary_region: String,

    /// Primary language for metadata to be pulled from
    #[clap(env = "ALU_PRIMARY_LANGUAGE", default_value = "en")]
    pub primary_lang: String,

    /// Directory to store games
    #[clap(env = "ALU_ROM_DIR", default_value = "games/")]
    pub rom_dir: String,

    /// Secondary locales for metadata fallback
    #[clap(
        env = "ALU_SECONDARY_LOCALES",
        value_delimiter = ',',
        default_value = ""
    )]
    pub secondary_locales: Vec<String>,

    #[clap(long, env = "ALU_PROD_KEYS", default_value_t = String::from(dirs::home_dir().unwrap_or_default().join(".switch/prod.keys").to_str().unwrap_or("~/.switch/prod.keys")))]
    pub prod_keys: String,

    #[clap(long, env = "ALU_TITLE_KEYS", default_value_t = String::from(dirs::home_dir().unwrap_or_default().join(".switch/title.keys").to_str().unwrap_or("~/.switch/title.keys")))]
    pub title_keys: String,

    #[clap(long, env = "ALU_TITLE_DB_CACHE_DIR", default_value_t = String::from(std::env::temp_dir().join("titledb").to_str().unwrap_or("/tmp/titledb")))]
    pub title_db_cache_dir: String,

    #[clap(long, env = "ALU_PUBLIC", default_value = "false")]
    pub public: bool,
}

impl BackendConfig {
    pub fn get_locale_string(&self) -> String {
        format!("{}_{}", self.primary_region, self.primary_lang)
    }

    /// Get valid secondary locales (filters out empty strings)
    pub fn get_valid_secondary_locales(&self) -> Vec<String> {
        self.secondary_locales
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect()
    }

    pub fn temp_dir(&self) -> PathBuf {
        self.title_db_cache_dir.clone().into()
    }
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
