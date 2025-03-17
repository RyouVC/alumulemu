//! Database instance module
use serde::{Deserialize, Serialize};

use std::sync::LazyLock;

use surrealdb::{
    Surreal,
    engine::any::{self, Any},
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NspMetadata {
    pub path: String,
    pub title_id: String,
    pub version: String,
}

impl NspMetadata {
    pub async fn get_all() -> surrealdb::Result<Vec<Self>> {
        DB_2.select("nsp_metadata").await
    }
    pub async fn get_by_path(path: &str) -> surrealdb::Result<Option<Self>> {
        DB_2.select(("nsp_metadata", path)).await
    }

    pub async fn save(&self) -> surrealdb::Result<Option<NspMetadata>> {
        let created: Option<NspMetadata> = DB_2
            .create(("nsp_metadata", &self.path))
            .content(self.clone())
            .await?;
        Ok(created)
    }
}
pub static DB: LazyLock<Surreal<Any>> = LazyLock::new(Surreal::init);
pub static DB_2: LazyLock<Surreal<Any>> = LazyLock::new(Surreal::init);
pub async fn init_database() -> surrealdb::Result<()> {
    DB.connect("rocksdb://./database").await?;
    DB_2.connect("rocksdb://./stored_database").await?;
    //DB.connect("ws://localhost:8000").await?;
    // DB.signin(surrealdb::opt::auth::Root {
    //     username: "root",
    //     password: "root",
    // })
    // .await?;
    DB.use_ns("tinfoil").use_db("games").await?;
    DB_2.use_ns("tinfoil").use_db("stored").await?;
    Ok(())
}
