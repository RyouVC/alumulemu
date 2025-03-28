//! Downloader API module

use crate::import::downloader::{DOWNLOAD_QUEUE, DownloadQueueItem, DownloadStatus, Progress};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use color_eyre::Result;
use http::StatusCode;
use std::collections::HashMap;
use ulid::Ulid;

/// Get all active downloads and their current status
pub async fn get_downloads() -> Result<HashMap<Ulid, DownloadQueueItem>> {
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

    // Process the vector outside of the MutexGuard's scope
    let downloads = downloads_vec
        .into_iter()
        .map(|(id, item, _)| (id, item))
        .collect::<HashMap<Ulid, DownloadQueueItem>>();

    Ok(downloads)
}

/// Get a specific download item by its ID
pub async fn get_download(id: &Ulid) -> Result<Option<DownloadQueueItem>> {
    let item = {
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
            .map(|(_, item, _)| item.clone())
    };

    Ok(item)
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
