use axum::{
    Router,
    http::StatusCode,
    response::{Html, IntoResponse},
};
use tower_http::services::ServeDir;

use crate::backend::api::api_router;
use crate::backend::user::{basic_auth, basic_auth_if_public};

/// Create the main backend router
pub fn create_router() -> Router {
    Router::new()
        // API routes
        .nest("/api", api_router())
        // Admin routes
        .nest("/admin", admin_router())
        // Favicon route - placed before other routes for priority
        .route(
            "/favicon.ico",
            axum::routing::get(|| async {
                match std::fs::read("alu-panel/dist/favicon.ico") {
                    Ok(content) => axum::response::Response::builder()
                        .header("Content-Type", "image/x-icon")
                        .body(axum::body::Body::from(content))
                        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
                    Err(_) => StatusCode::NOT_FOUND.into_response(),
                }
            }),
        )
        // Static files from alu-panel/dist
        .merge(static_router())
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

pub fn static_router() -> Router {
    Router::new()
        // Static files from alu-panel/dist
        .nest_service("/static", ServeDir::new("alu-panel/dist/static"))
        // Fallback for web UI
        .fallback(|| async {
            match std::fs::read_to_string("alu-panel/dist/index.html") {
                Ok(contents) => Html(contents).into_response(),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        })
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
