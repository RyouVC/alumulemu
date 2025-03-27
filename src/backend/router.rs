use axum::{
    Router,
    http::StatusCode,
    response::{Html, IntoResponse},
};

use crate::backend::api::api_router;
use crate::backend::user::{basic_auth, basic_auth_if_public, user_router};

/// Create the main backend router
pub fn create_router() -> Router {
    Router::new()
        // API routes
        .nest("/api", api_router())
        // Admin routes
        .nest("/admin", admin_router())
        // Fallback for web UI
        // todo: bundle into executable
        .fallback(|| async {
            match std::fs::read_to_string("alu-panel/dist/index.html") {
                Ok(contents) => Html(contents).into_response(),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        })
        .layer(axum::middleware::from_fn(basic_auth_if_public))
}

/// Create the admin router with authentication
pub fn admin_router() -> Router {
    Router::new()
        .route("/rescan", axum::routing::post(super::admin::rescan_games))
        .fallback(|| async {
            match std::fs::read_to_string("alu-panel/dist/index.html") {
                Ok(contents) => Html(contents).into_response(),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        })
        // Generic importer endpoints
        .route(
            "/import/{importer}/{id}",
            axum::routing::get(super::admin::generic_import_by_id),
        )
        .route(
            "/import/auto/{id}",
            axum::routing::get(super::admin::auto_import_by_id),
        )
        // Add authentication layer
        .layer(axum::middleware::from_fn(basic_auth))
}
