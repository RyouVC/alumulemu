// filepath: /home/cappy/Projects/alumulemu/src/backend/admin.rs
use axum::{
    Json,
    extract::{Path, Query},
};
use http::StatusCode;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    games_dir,
    import::registry,
    index::TinfoilResponse,
    router::{AlumRes, RescanOptions, update_metadata_from_filesystem},
};

// Define response types for API endpoints
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ApiResponse<T> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ImporterInfo {
    pub id: String,
    pub display_name: String,
    pub description: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ImportersResponse {
    pub importers: Vec<ImporterInfo>,
}

// Global flag to track if a rescan job is already running
static RESCAN_IN_PROGRESS: Lazy<Arc<AtomicBool>> = Lazy::new(|| Arc::new(AtomicBool::new(false)));

pub async fn trigger_rescan(options: RescanOptions) -> color_eyre::Result<()> {
    // Try to set the flag - returns false if already set
    if RESCAN_IN_PROGRESS
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        tracing::info!("Rescan already in progress, ignoring new request");
        return Ok(());
    }

    tracing::info!("Starting games directory rescan as async background job");

    // Clone the flag for use in the background task
    let rescan_flag = RESCAN_IN_PROGRESS.clone();
    tracing::debug!(?options, "Rescan flag cloned for background task");

    // Spawn a background task to handle the rescan
    tokio::spawn(async move {
        let result = update_metadata_from_filesystem(&games_dir(), options).await;

        match result {
            Ok(_) => {
                tracing::info!("Background rescan job completed successfully");
                // Uncomment if you want to add metaview creation back
                // tracing::info!("(re)Creating precomputed metaview");
                // if let Err(e) = create_precomputed_metaview().await {
                //     tracing::warn!("Failed to create precomputed metaview: {}", e);
                // }
            }
            Err(e) => {
                tracing::error!("Background rescan job failed: {}", e);
            }
        }

        // Reset the flag when the job completes
        rescan_flag.store(false, Ordering::SeqCst);
    });

    Ok(())
}

#[tracing::instrument]
pub async fn rescan_games(options: Query<RescanOptions>) -> AlumRes<Json<TinfoilResponse>> {
    // Trigger the rescan job
    tracing::info!("Received request to rescan games directory");
    tracing::info!("Rescan options: {:?}", options);
    let _ = trigger_rescan(options.0).await;

    // Return immediately with a message that the job has started
    Ok(Json(TinfoilResponse::MiscSuccess(
        "Games rescan started in background".to_string(),
    )))
}

/// Process an import using the new JSON-based importers
#[axum::debug_handler]
pub async fn process_import(
    Path(importer_id): Path<String>,
    Json(json_body): Json<serde_json::Value>,
) -> (StatusCode, Json<ApiResponse<()>>) {
    tracing::info!(
        importer = importer_id,
        "Processing import request with JSON body"
    );

    // Convert the JSON value to a string
    let json_str = json_body.to_string();

    // Process the import - this already handles locks properly
    match registry::import_with_json(&importer_id, &json_str).await {
        Ok(import_source) => {
            // Process the import source
            match import_source.import().await {
                Ok(_) => {
                    tracing::info!("Import completed successfully");
                    (
                        StatusCode::OK,
                        Json(ApiResponse {
                            status: "success".to_string(),
                            message: Some("Import completed successfully".to_string()),
                            data: None,
                        }),
                    )
                }
                Err(e) => {
                    tracing::error!("Error processing import: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ApiResponse {
                            status: "error".to_string(),
                            message: Some(format!("Error processing import: {}", e)),
                            data: None,
                        }),
                    )
                }
            }
        }
        Err(e) => {
            tracing::error!("Import error: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse {
                    status: "error".to_string(),
                    message: Some(e.to_string()),
                    data: None,
                }),
            )
        }
    }
}

/// Get a list of all available importers
pub async fn list_importers() -> (StatusCode, Json<ApiResponse<ImportersResponse>>) {
    // Create a scope to ensure the lock is dropped before returning
    let importers = {
        // This ensures the RwLockReadGuard is dropped before the function returns
        registry::get_all_importers()
            .into_iter()
            .map(|importer| ImporterInfo {
                id: importer.name().to_string(),
                display_name: importer.display_name().to_string(),
                description: importer.description().to_string(),
            })
            .collect::<Vec<_>>()
    };

    (
        StatusCode::OK,
        Json(ApiResponse {
            status: "success".to_string(),
            message: None,
            data: Some(ImportersResponse { importers }),
        }),
    )
}
