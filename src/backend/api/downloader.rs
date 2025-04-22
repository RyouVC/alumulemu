//! Downloader API module

use crate::import::downloader::{DOWNLOAD_QUEUE, DownloadStatus, Progress}; // Remove DownloadQueueItem
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc}; // Add imports for chrono types
use color_eyre::Result;
use http::StatusCode;
use serde_json::json;
use std::collections::BTreeMap;
use std::path::PathBuf; // Add import for PathBuf
use ulid::Ulid;

/// Combined DownloadQueueItem with its current Progress, excluding sensitive headers
#[derive(Debug, serde::Serialize)]
pub struct DownloadItemWithProgress {
    // Explicitly list fields from DownloadQueueItem (excluding headers and id) and Progress
    pub url: String,
    pub output_path: PathBuf,
    pub created_at: Option<DateTime<Utc>>,
    // Keep Progress nested
    pub progress: Progress,
}

/// Get all active downloads and their current status
pub async fn get_downloads() -> Result<BTreeMap<Ulid, DownloadItemWithProgress>> {
    // Create a scope to ensure the lock is dropped after getting the data
    let downloads_vec = {
        // Lock is acquired here, and any potential PoisonError is immediately converted
        // to a color_eyre::Report error instead of being propagated with the MutexGuard
        let queue = match DOWNLOAD_QUEUE.lock() {
            Ok(guard) => guard,
            Err(poison_err) => {
                return Err(color_eyre::eyre::eyre!(
                    "Failed to lock download queue: {}",
                    poison_err
                ));
            }
        };

        // Call list_downloads and collect the data into a new vector that doesn't depend on queue
        queue
            .list_downloads()
            .into_iter()
            .map(|(id, item, progress)| (id, item.clone(), progress))
            .collect::<Vec<_>>()

        // Lock is automatically dropped here when queue goes out of scope
    };

    // Process the vector outside of the MutexGuard's scope and use BTreeMap instead of HashMap
    let downloads = downloads_vec
        .into_iter()
        .map(|(id, item, progress)| {
            (
                id, // Keep Ulid as the key
                DownloadItemWithProgress {
                    // Construct the new struct without headers
                    url: item.url,
                    output_path: item.output_path,
                    created_at: item.created_at,
                    progress,
                },
            )
        })
        .collect::<BTreeMap<Ulid, DownloadItemWithProgress>>();

    Ok(downloads)
}

/// Get a specific download item by its ID
pub async fn get_download(id: &Ulid) -> Result<Option<DownloadItemWithProgress>> {
    let item_with_progress = {
        let queue = match DOWNLOAD_QUEUE.lock() {
            Ok(guard) => guard,
            Err(poison_err) => {
                return Err(color_eyre::eyre::eyre!(
                    "Failed to lock download queue: {}",
                    poison_err
                ));
            }
        };

        // Find the specific download item by ID
        queue
            .list_downloads()
            .into_iter()
            .find(|(item_id, _, _)| item_id == id)
            .map(|(_, item, progress)| DownloadItemWithProgress {
                // Construct the new struct without headers
                url: item.url.clone(),
                output_path: item.output_path.clone(),
                created_at: item.created_at,
                progress,
            })
    };

    Ok(item_with_progress)
}

/// Get a summary of download status statistics
pub async fn get_download_stats() -> Result<DownloadStats> {
    let downloads_vec = {
        let queue = match DOWNLOAD_QUEUE.lock() {
            Ok(guard) => guard,
            Err(poison_err) => {
                return Err(color_eyre::eyre::eyre!(
                    "Failed to lock download queue: {}",
                    poison_err
                ));
            }
        };
        queue
            .list_downloads()
            .into_iter()
            .map(|(_, _, progress)| progress)
            .collect::<Vec<Progress>>()
    };

    let mut stats = DownloadStats::default();

    for progress in downloads_vec {
        match progress.status {
            DownloadStatus::Queued => stats.queued += 1,
            DownloadStatus::Downloading => stats.downloading += 1,
            DownloadStatus::Paused => stats.paused += 1,
            DownloadStatus::Completed => stats.completed += 1,
            DownloadStatus::Cancelled => stats.cancelled += 1,
            DownloadStatus::Failed(_) => stats.failed += 1,
        }
    }

    stats.total = stats.queued
        + stats.downloading
        + stats.paused
        + stats.completed
        + stats.cancelled
        + stats.failed;

    Ok(stats)
}

/// Cancel a download by its ID
pub async fn cancel_download(id: &Ulid) -> Result<bool> {
    let result = {
        let mut queue = match DOWNLOAD_QUEUE.lock() {
            Ok(guard) => guard,
            Err(poison_err) => {
                return Err(color_eyre::eyre::eyre!(
                    "Failed to lock download queue: {}",
                    poison_err
                ));
            }
        };
        queue.cancel(id)
    };

    Ok(result)
}

/// Clean up completed and aborted downloads from the queue
pub async fn cleanup_downloads() -> Result<usize> {
    // Acquire the lock, perform cleanup, get the count, then drop the lock
    let result = {
        let mut queue = match DOWNLOAD_QUEUE.lock() {
            Ok(guard) => guard,
            Err(poison_err) => {
                return Err(color_eyre::eyre::eyre!(
                    "Failed to lock download queue: {}",
                    poison_err
                ));
            }
        };
        queue.cleanup()
    };

    // Return the count directly
    Ok(result)
}

/// Handler for cleaning up finished downloads
#[axum::debug_handler]
pub async fn cleanup_downloads_handler() -> Result<impl IntoResponse, StatusCode> {
    match cleanup_downloads().await {
        Ok(count) => {
            tracing::info!("Cleaned up download queue, removed {} items", count);
            Ok(Json(json!({ "count": count })).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to clean up downloads: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Statistics about download queue
#[derive(Debug, Default, serde::Serialize)]
pub struct DownloadStats {
    pub total: usize,
    pub queued: usize,
    pub downloading: usize,
    pub paused: usize,
    pub completed: usize,
    pub cancelled: usize,
    pub failed: usize,
}

pub async fn get_downloads_handler() -> Result<impl IntoResponse, StatusCode> {
    match get_downloads().await {
        Ok(downloads) => Ok(Json(downloads).into_response()),
        Err(e) => {
            tracing::error!("Failed to get downloads: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for getting a specific download by ID
pub async fn get_download_handler(
    axum::extract::Path(id): axum::extract::Path<Ulid>,
) -> Result<impl IntoResponse, StatusCode> {
    match get_download(&id).await {
        Ok(Some(item)) => Ok(Json(item).into_response()),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get download {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for getting download stats
pub async fn get_download_stats_handler() -> Result<impl IntoResponse, StatusCode> {
    match get_download_stats().await {
        Ok(stats) => Ok(Json(stats).into_response()),
        Err(e) => {
            tracing::error!("Failed to get download stats: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Handler for cancelling a download
pub async fn cancel_download_handler(
    axum::extract::Path(id): axum::extract::Path<Ulid>,
) -> Result<impl IntoResponse, StatusCode> {
    match cancel_download(&id).await {
        Ok(success) => {
            if success {
                Ok(StatusCode::OK.into_response())
            } else {
                Ok(StatusCode::NOT_FOUND.into_response())
            }
        }
        Err(e) => {
            tracing::error!("Failed to cancel download {}: {}", id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub fn dl_write_router() -> Router {
    Router::new()
        .route("/{id}/cancel", get(cancel_download_handler))
        .route("/cleanup", get(cleanup_downloads_handler))
        .layer(axum::middleware::from_fn(
            crate::backend::user::auth_require_editor,
        ))
}

pub fn downloader_api() -> Router {
    Router::new()
        .route("/", get(get_downloads_handler))
        .route("/stats", get(get_download_stats_handler))
        .route("/{id}", get(get_download_handler))
        .merge(dl_write_router())
    // .nest("/{id}/cancel", get(cancel_download_router))
}
