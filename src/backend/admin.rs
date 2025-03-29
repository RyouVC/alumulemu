// filepath: /home/cappy/Projects/alumulemu/src/backend/admin.rs
use axum::{
    Json,
    extract::{Path, Query},
    response::IntoResponse,
};
use http::StatusCode;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::{
    games_dir,
    import::import_utils::import_by_id,
    index::TinfoilResponse,
    router::{AlumRes, RescanOptions, update_metadata_from_filesystem},
};

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

// json body
pub async fn generic_import_by_json(
    Json(params): Json<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let (importer_name, id) = params;

    tracing::info!(
        "Starting generic import with importer '{}' for ID: {}",
        importer_name,
        id
    );

    todo!("Implement JSON import logic");

    // Use our new generic import_by_id utility
    match import_by_id(Some(importer_name), id).await {
        Ok(response) => Ok(response),
        Err(e) => {
            tracing::error!("Import error: {}", e);
            Ok((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": e.to_string()
                })),
            )
                .into_response())
        }
    }
}

// Generic importer that allows specifying which importer to use
pub async fn generic_import_by_id(
    Path(params): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    let (importer_name, id) = params;

    tracing::info!(
        "Starting generic import with importer '{}' for ID: {}",
        importer_name,
        id
    );

    // Use our new generic import_by_id utility
    match import_by_id(Some(importer_name), id).await {
        Ok(response) => Ok(response),
        Err(e) => {
            tracing::error!("Import error: {}", e);
            Ok((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": e.to_string()
                })),
            )
                .into_response())
        }
    }
}

// Auto-select importer based on ID format
pub async fn auto_import_by_id(Path(id): Path<String>) -> Result<impl IntoResponse, StatusCode> {
    tracing::info!("Auto-selecting importer for ID: {}", id);

    // Use our new generic import_by_id utility with no specific importer
    match import_by_id(None, id).await {
        Ok(response) => Ok(response),
        Err(e) => {
            tracing::error!("Import error: {}", e);
            Ok((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "status": "error",
                    "message": e.to_string()
                })),
            )
                .into_response())
        }
    }
}
