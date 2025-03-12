//! Database instance module

use std::sync::LazyLock;

use surrealdb::{engine::{any::{self, Any}}, Surreal};

pub static DB: LazyLock<Surreal<Any>> = LazyLock::new(Surreal::init);

pub async fn init_database() -> surrealdb::Result<()> {

    
    DB.connect("rocksdb://./database").await?;
    DB.use_ns("tinfoil").use_db("games").await?;
    
    Ok(())
}