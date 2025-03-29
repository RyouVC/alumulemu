use axum::{
    Router,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use tower_http::services::ServeDir;

use crate::backend::api::api_router;

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
                    Ok(content) => {
                        match axum::response::Response::builder()
                            .header("Content-Type", "image/x-icon")
                            .body(axum::body::Body::from(content))
                        {
                            Ok(response) => response,
                            Err(e) => {
                                tracing::error!("Failed to build favicon response: {}", e);
                                StatusCode::INTERNAL_SERVER_ERROR.into_response()
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read favicon.ico: {}", e);
                        StatusCode::NOT_FOUND.into_response()
                    }
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
                Err(e) => {
                    tracing::error!("Failed to read index.html: {}", e);
                    StatusCode::NOT_FOUND.into_response()
                }
            }
        })
        // Use our new HRBAC middleware instead of the old basic_auth_if_public
        .layer(axum::middleware::from_fn(super::user::auth_optional_viewer))
}

pub fn static_router() -> Router {
    Router::new()
        // Static files from alu-panel/dist
        .nest_service("/static", ServeDir::new("alu-panel/dist/static"))
        // Fallback for web UI
        .fallback(|| async {
            match std::fs::read_to_string("alu-panel/dist/index.html") {
                Ok(contents) => Html(contents).into_response(),
                Err(e) => {
                    tracing::error!("Failed to read index.html: {}", e);
                    StatusCode::NOT_FOUND.into_response()
                }
            }
        })
}

/// Utility function to safely read the HTML file with error handling
async fn read_html_fallback() -> Response {
    match std::fs::read_to_string("alu-panel/dist/index.html") {
        Ok(contents) => Html(contents).into_response(),
        Err(e) => {
            tracing::error!("Failed to read index.html: {}", e);
            StatusCode::NOT_FOUND.into_response()
        }
    }
}

/// Create the admin router with authentication
pub fn admin_router() -> Router {
    // Create a router for import functionality (Editor access)
    let import_router = Router::new()
        // New JSON-based importers
        .route(
            "/{importer_id}",
            axum::routing::post(super::admin::process_import),
        )
        .route(
            "/list",
            axum::routing::get(super::admin::list_importers),
        );

    // Main admin router with rescan (Admin access)
    Router::new()
        .route("/rescan", axum::routing::post(super::admin::rescan_games))
        .nest("/import", import_router)
        .fallback(read_html_fallback)
        // Add authentication layer - Editor required for editing data
        .layer(axum::middleware::from_fn(super::user::auth_require_editor))
}
