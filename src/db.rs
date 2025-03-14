//! Database instance module

use std::sync::LazyLock;

use surrealdb::{
    Surreal,
    engine::any::{self, Any},
};

pub static DB: LazyLock<Surreal<Any>> = LazyLock::new(Surreal::init);

pub async fn init_database() -> surrealdb::Result<()> {
    // DB.connect("rocksdb://./database").await?;
    DB.connect("ws://localhost:8000").await?;
    DB.signin(surrealdb::opt::auth::Root {
        username: "root",
        password: "root",
    })
    .await?;
    DB.use_ns("tinfoil").use_db("games").await?;

    Ok(())
}
