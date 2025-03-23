//! Database instance module
use serde::{Deserialize, Serialize};

use std::sync::LazyLock;

use surrealdb::{Surreal, engine::any::Any};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NspMetadata {
    pub path: String,
    pub title_id: String,
    pub version: String,
}

impl NspMetadata {
    pub async fn get_all() -> surrealdb::Result<Vec<Self>> {
        DB.select("nsp_metadata").await
    }
    #[tracing::instrument(level = "debug")]
    pub async fn get_by_path(path: &str) -> surrealdb::Result<Option<Self>> {
        DB.select(("nsp_metadata", path)).await
    }

    #[tracing::instrument(level = "debug")]
    pub async fn save(&self) -> surrealdb::Result<Option<NspMetadata>> {
        let created: Option<NspMetadata> = DB
            .upsert(("nsp_metadata", &self.path))
            .content(self.clone())
            .await?;
        Ok(created)
    }

    #[tracing::instrument(level = "debug")]
    pub async fn delete_cache() -> surrealdb::Result<()> {
        let _: Vec<NspMetadata> = DB.delete("nsp_metadata").await?;
        Ok(())
    }
}

pub static DB: LazyLock<Surreal<Any>> = LazyLock::new(Surreal::init);
#[tracing::instrument]
pub async fn init_database() -> surrealdb::Result<()> {
    let config = crate::config::config();

    tracing::info!(
        "Connecting to database at {}",
        config.db_config.database_url
    );
    // DB.connect("rocksdb://./database").await?;
    DB.connect(config.db_config.database_url).await?;

    match config.db_config.db_auth_method {
        crate::config::DatabaseAuthMethod::Root => {
            tracing::info!("Signing in as root user");
            DB.signin(surrealdb::opt::auth::Root {
                username: &config.db_config.root_username.unwrap(),
                password: &config.db_config.root_password.unwrap(),
            })
            .await?;
        }
        crate::config::DatabaseAuthMethod::None => {
            tracing::info!("No authentication configured");
        }
    }

    tracing::info!(
        "Using namespace '{}' and database '{}'",
        config.db_config.db_namespace,
        config.db_config.db_database
    );
    DB.use_ns(config.db_config.db_namespace)
        .use_db(config.db_config.db_database)
        .await?;

    tracing::info!("Database initialization complete");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_delete_cache() {
        init_database().await.unwrap();
        NspMetadata::delete_cache().await.unwrap();
    }
}
