use axum::{
    Json, Router,
    extract::Path,
    routing::{get, post},
};
use serde_json::Value;

use crate::{backend::kv_config::KVConfig, router::AlumRes};

pub async fn get_key(Path(key): Path<String>) -> AlumRes<Json<Option<KVConfig>>> {
    tracing::trace!("Getting key: {}", key);
    let config = KVConfig::get(&key).await?;
    Ok(Json(config))
}

pub async fn set_key(Path(key): Path<String>, Json(config): Json<Value>) -> AlumRes<Json<Value>> {
    tracing::trace!("Setting key: {}", key);
    // Pass the key and a mutable reference to the config value
    let mut kv = KVConfig::new(key.clone(), None);
    kv.set(config.clone()).await?;
    Ok(Json(config))
}

pub fn config_router() -> Router {
    Router::new()
        .layer(axum::middleware::from_fn(
            crate::backend::user::auth_require_admin,
        ))
        .route("/get/{key}", get(get_key))
        .route("/set/{key}", post(set_key))
}
