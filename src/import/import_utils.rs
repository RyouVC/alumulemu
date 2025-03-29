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


/// Helper function to import with JSON
pub async fn import_with_json(importer_id: &str, json: &str) -> ImportResult {
    info!(importer = importer_id, "Starting import request with JSON");

    // Use the registry to find the import source - this validates the request
    // but doesn't start the download yet
    match registry::import_with_json(importer_id, json).await {
        Ok(import_source) => {
            // Store the importer_id for the response
            let response_importer_id = importer_id.to_string();

            // Start a background task to process the import
            let importer_id = importer_id.to_string();
            tokio::spawn(async move {
                info!(
                    importer = importer_id,
                    "Starting import process in background"
                );

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

            // Return success immediately - the source was found and download queued
            Ok(Json(ApiResponse {
                status: "success".to_string(),
                message: Some("Import started".to_string()),
                data: Some(ImportStartResponse {
                    importer: response_importer_id,
                }),
            })
            .into_response())
        }
        Err(e) => {
            // Return error immediately - the source wasn't found, no download started
            error!(importer = importer_id, error = %e, "Import error (pre-download)");
            Err(ImportError::ImportFailed(e))
        }
    }
}
