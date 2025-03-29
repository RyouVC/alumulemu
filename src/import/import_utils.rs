use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::{error, info};

use crate::backend::admin::{ApiResponse, trigger_rescan};
use crate::import::registry;
use crate::router::RescanOptions;

/// Errors that can occur during the import process
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Importer not found: {0}")]
    ImporterNotFound(String),

    #[error("Invalid import request: {0}")]
    InvalidRequest(String),

    #[error("Import failed: {0}")]
    ImportFailed(#[from] crate::import::ImportError),

    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

impl IntoResponse for ImportError {
    fn into_response(self) -> Response {
        let error_msg = self.to_string();
        error!("Import error: {}", error_msg);
        (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse::<()> {
                status: "error".to_string(),
                message: Some(error_msg),
                data: None,
            }),
        )
            .into_response()
    }
}

/// Unified importer interface result type
pub type ImportResult = std::result::Result<Response, ImportError>;

/// Helper function to import by title ID using the UltraNX importer
pub async fn import_by_title_id(
    title_id: impl Into<String>,
    download_type: Option<impl Into<String>>,
) -> ImportResult {
    let title_id = title_id.into();
    let download_type = download_type
        .map(|dt| dt.into())
        .unwrap_or_else(|| "fullpkg".to_string());

    // Create JSON for UltraNX importer
    let json = serde_json::json!({
        "title_id": title_id,
        "download_type": download_type,
    })
    .to_string();

    import_with_json("ultranx", &json).await
}

/// Helper function to import by URL
pub async fn import_by_url(url: impl Into<String>) -> ImportResult {
    let url = url.into();

    // Create JSON for URL importer
    let json = serde_json::json!({
        "url": url,
    })
    .to_string();

    import_with_json("url", &json).await
}

/// Helper function to import with JSON
async fn import_with_json(importer_id: &str, json: &str) -> ImportResult {
    info!(importer = importer_id, "Starting import request with JSON");

    // Use the registry to find and use the importer
    match registry::import_with_json(importer_id, json).await {
        Ok(import_source) => {
            // Store the importer_id for the response
            let response_importer_id = importer_id.to_string();

            // Start a background task to process the import
            let importer_id = importer_id.to_string();
            tokio::spawn(async move {
                info!(importer = importer_id, "Starting import process");

                match import_source.import().await {
                    Ok(_) => {
                        info!(importer = importer_id, "Import completed successfully");

                        // Trigger a rescan after successful import
                        info!("Triggering rescan after import");
                        let _ = trigger_rescan(RescanOptions::default()).await;
                    }
                    Err(e) => {
                        error!(importer = importer_id, error = %e, "Import failed");
                    }
                }
            });

            // Define a response type for import start
            #[derive(serde::Serialize)]
            struct ImportStartResponse {
                importer: String,
            }

            Ok(Json(ApiResponse {
                status: "success".to_string(),
                message: Some("Import started".to_string()),
                data: Some(ImportStartResponse {
                    importer: response_importer_id,
                }),
            })
            .into_response())
        }
        Err(e) => Err(ImportError::ImportFailed(e)),
    }
}
