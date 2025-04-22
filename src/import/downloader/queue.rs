//! Download queue management
//!
//! This module handles the management of download queues, including tracking
//! multiple concurrent downloads and their progress.

use std::{
    collections::BTreeMap,
    sync::{LazyLock, Mutex},
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use tracing::{Level, debug, error, info, span, trace, warn};
use ulid::Ulid;

use super::http::Downloader;
use super::models::{DownloadQueueItem, DownloadStatus, Progress};
use crate::db::DB;

/// Global download queue instance
pub static DOWNLOAD_QUEUE: LazyLock<Mutex<DownloadQueue>> =
    LazyLock::new(|| Mutex::new(DownloadQueue::new()));

// Download handle returned to caller for tracking progress and cancellation
#[derive(Debug)]
pub struct DownloadHandle {
    pub id: Ulid,
    pub progress_rx: watch::Receiver<Progress>,
    cancellation_token: CancellationToken,
}

impl DownloadHandle {
    pub(crate) fn new(
        id: Ulid,
        progress_rx: watch::Receiver<Progress>,
        token: CancellationToken,
    ) -> Self {
        Self {
            id,
            progress_rx,
            cancellation_token: token,
        }
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancellation_token.is_cancelled()
    }

    // Get latest progress snapshot
    pub fn progress(&self) -> Progress {
        self.progress_rx.borrow().clone()
    }

    // Wait for progress updates
    pub async fn wait_for_progress_change(&mut self) -> Result<(), watch::error::RecvError> {
        self.progress_rx.changed().await
    }

    /// Wait for the download to complete and return the file path
    ///
    /// Returns Ok(PathBuf) with the downloaded file path if successful,
    /// or Err with the error message if it failed.
    pub async fn wait_until_done(&mut self) -> Result<std::path::PathBuf, String> {
        loop {
            // Wait for next progress update
            if let Err(e) = self.wait_for_progress_change().await {
                return Err(format!("Failed to monitor download progress: {}", e));
            }

            let progress = self.progress();

            if progress.is_complete() {
                return match progress.status {
                    DownloadStatus::Completed => {
                        // Return the file path from the progress if available
                        if let Some(path) = progress.file_path {
                            Ok(path)
                        } else {
                            // Fallback to getting the path from database
                            let item: DownloadQueueItem = DB
                                .select(("download_queue", self.id.to_string()))
                                .await
                                .map_err(|e| format!("Failed to retrieve download info: {}", e))?
                                .ok_or_else(|| "Download info not found in database".to_string())?;

                            Ok(item.output_path)
                        }
                    }
                    DownloadStatus::Failed(err) => Err(err),
                    DownloadStatus::Cancelled => Err("Download was cancelled".to_string()),
                    _ => Err("Download entered unexpected state".to_string()),
                };
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct DownloadQueue {
    downloads: BTreeMap<Ulid, (DownloadQueueItem, JoinHandle<()>)>,
    progress_watchers: BTreeMap<Ulid, watch::Sender<Progress>>,
}

impl DownloadQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, mut item: DownloadQueueItem) -> DownloadHandle {
        // Create a new ulid first
        let id_ulid = Ulid::new();

        // Log the ID we're creating
        info!("Creating new download with ULID: {}", id_ulid);

        // Then create a SurrealDB thing using this ulid
        let id = surrealdb::sql::Thing::from((
            "download_queue",
            surrealdb::sql::Id::from(id_ulid.to_string()),
        ));

        item.id = Some(id);
        item.created_at = Some(chrono::Utc::now());

        info!(id = %id_ulid, url = %item.url, "Adding download to queue");
        debug!(id = %id_ulid, path = ?item.output_path, "Download destination");

        // Create a watch channel for progress updates
        let (progress_tx, progress_rx) = watch::channel(Progress::default());

        let cancellation_token = CancellationToken::new();

        // Clone what we need to move into the task
        let url = item.url.clone();
        let output_path = item.output_path.clone();
        let headers = item.headers.clone();
        let token_clone = cancellation_token.clone();
        // Save the progress transmitter for later use
        self.progress_watchers.insert(id_ulid, progress_tx.clone());

        // Create a channel for the download task to send progress updates
        let (internal_tx, mut internal_rx) = mpsc::channel(10);

        // Clone for database updates
        let item_clone = item.clone();
        let progress_tx_clone = progress_tx.clone();
        let id_for_task = id_ulid; // Clone the ID for use in the download task

        // Start the download task
        let handle = tokio::spawn(async move {
            let download_span = span!(Level::DEBUG, "download_task", id = %id_for_task, url = %url);
            let _guard = download_span.enter();

            info!("Starting download task");
            let downloader = Downloader::new();
            let result = downloader
                .download_file_with_progress_cancellable(
                    &url,
                    &output_path,
                    internal_tx,
                    token_clone.clone(),
                    headers.as_ref(),
                )
                .await;

            // Update progress with final status
            let final_progress = match &result {
                Ok(path) => {
                    info!(path = ?path, "Download completed successfully");
                    Progress {
                        status: DownloadStatus::Completed,
                        file_path: Some(path.clone()),
                        ..progress_tx.borrow().clone()
                    }
                }
                Err(e) => {
                    error!(error = %e, "Download failed");
                    Progress {
                        status: DownloadStatus::Failed(e.to_string()),
                        ..progress_tx.borrow().clone()
                    }
                }
            };

            // Send final update
            let _ = progress_tx.send(final_progress);
        });

        // Start a task to forward progress updates from the internal channel to both
        // the watch channel (for the handle) and the database
        let id_clone = id_ulid;
        tokio::spawn(async move {
            let progress_span = span!(Level::TRACE, "download_progress", id = %id_clone);
            let _guard = progress_span.enter();

            let mut db_item = item_clone;

            // Forward progress updates from the downloader to the watch channel and database
            while let Some(progress) = internal_rx.recv().await {
                // Only log detailed progress at trace level
                if let Some(total) = progress.total_size {
                    let percentage = (progress.downloaded as f32 / total as f32) * 100.0;
                    trace!(
                        bytes = progress.downloaded,
                        total = total,
                        percentage = percentage,
                        "Download progress"
                    );

                    // Add more detailed logs at significant progress points, but keep at info level
                    if percentage < 0.1
                        || (percentage > 24.9 && percentage < 25.1)
                        || (percentage > 49.9 && percentage < 50.1)
                        || (percentage > 74.9 && percentage < 75.1)
                        || percentage > 99.9
                    {
                        info!(
                            percentage = format_args!("{:.1}%", percentage),
                            bytes = progress.downloaded,
                            total = total,
                            "Download milestone"
                        );
                    }
                } else {
                    trace!(
                        bytes = progress.downloaded,
                        "Download progress (size unknown)"
                    );

                    // Log every megabyte for downloads with unknown size
                    if progress.downloaded % (1024 * 1024) < 8192 {
                        let mb = progress.downloaded as f32 / (1024.0 * 1024.0);
                        trace!(
                            mb = format_args!("{:.2}", mb),
                            bytes = progress.downloaded,
                            "Download progress (size unknown)"
                        );
                    }
                }

                // Update the database item if the file path is available
                if let Some(ref path) = progress.file_path {
                    if db_item.output_path != *path {
                        info!(
                            id = %id_clone,
                            old_path = ?db_item.output_path,
                            new_path = ?path,
                            "Updating download path in database"
                        );
                        db_item.output_path = path.clone();
                    }
                }

                // Update the database with progress information
                db_item.progress = progress.clone();
                if let Err(e) = db_item.save().await {
                    warn!(error = %e, "Failed to save download progress to database");
                }

                // Update the watch channel for clients
                let _ = progress_tx_clone.send(progress.clone());
            }

            trace!("Progress channel closed");
        });

        // Store the download information
        self.downloads.insert(id_ulid, (item, handle));

        info!(id = %id_ulid, "Download added to queue");

        // Return the handle to the caller
        DownloadHandle::new(id_ulid, progress_rx, cancellation_token)
    }

    pub fn cancel(&mut self, id: &Ulid) -> bool {
        if let Some((_, handle)) = self.downloads.get(id) {
            info!("Cancelling download: id={}", id);
            handle.abort();

            // Update progress with cancelled status
            if let Some(progress_tx) = self.progress_watchers.get(id) {
                let mut current = progress_tx.borrow().clone();
                current.status = DownloadStatus::Cancelled;
                let _ = progress_tx.send(current);
            }

            self.downloads.remove(id);
            self.progress_watchers.remove(id);
            info!("Download cancelled and removed from queue: id={}", id);
            true
        } else {
            warn!("Attempted to cancel non-existent download: id={}", id);
            false
        }
    }

    pub fn get_item(&self, id: &Ulid) -> Option<&DownloadQueueItem> {
        self.downloads.get(id).map(|(item, _)| item)
    }

    pub fn get_progress(&self, id: &Ulid) -> Option<Progress> {
        self.progress_watchers.get(id).map(|tx| tx.borrow().clone())
    }

    pub fn list_downloads(&self) -> Vec<(Ulid, &DownloadQueueItem, Progress)> {
        self.downloads
            .iter()
            .filter_map(|(id, (item, _))| {
                self.progress_watchers
                    .get(id)
                    .map(|tx| (*id, item, tx.borrow().clone()))
            })
            .collect()
    }

    // Cleans up completed downloads
    pub fn cleanup(&mut self) -> usize {
        // First, clean up finished tasks with the previous behavior
        let completed_ids: Vec<Ulid> = self
            .downloads
            .iter()
            .filter(|(_, (_, handle))| handle.is_finished())
            .map(|(id, _)| *id)
            .collect();

        // Now also include downloads that are completed, failed, or cancelled
        // based on their status, not just the finished state of the handle
        let status_complete_ids: Vec<Ulid> = self
            .downloads
            .iter()
            .filter_map(|(id, (_, _))| {
                if let Some(progress_tx) = self.progress_watchers.get(id) {
                    let progress = progress_tx.borrow();
                    if matches!(
                        progress.status,
                        DownloadStatus::Completed
                            | DownloadStatus::Failed(_)
                            | DownloadStatus::Cancelled
                    ) {
                        // Don't add duplicates from the previous filter
                        if !completed_ids.contains(id) {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();

        // Combine both lists
        let all_ids_to_remove: Vec<Ulid> = completed_ids
            .into_iter()
            .chain(status_complete_ids.into_iter())
            .collect();

        let count = all_ids_to_remove.len();

        for id in all_ids_to_remove {
            info!("Removing download from queue: id={}", id);
            self.downloads.remove(&id);
            self.progress_watchers.remove(&id);
        }

        count
    }

    // Sync all active downloads with the database
    pub async fn sync_with_db(&mut self) -> color_eyre::Result<()> {
        // Get all items from the database
        let db_items: Vec<DownloadQueueItem> = DB.select("download_queue").await?;
        info!("Syncing {} download items from database", db_items.len());

        // Update progress for all active downloads
        for item in db_items {
            if let Some(id) = item.id.as_ref() {
                // Extract just the ID part for parsing
                let id_str = id.id.to_string();
                trace!("Processing DB item with id: {}", id_str);

                if let Ok(id_ulid) = id_str.parse::<Ulid>() {
                    if let Some((queue_item, _)) = self.downloads.get_mut(&id_ulid) {
                        // Update the queue item with the database version
                        debug!("Updating queue item from DB: {}", id_ulid);
                        *queue_item = item;
                    } else {
                        trace!(
                            "Download item {} exists in DB but not in memory queue",
                            id_ulid
                        );
                    }
                } else {
                    warn!("Failed to parse DB item ID as ULID: {}", id_str);
                }
            }
        }

        Ok(())
    }

    /// Start a download in the background and return immediately
    ///
    /// This function is designed to be called from API handlers where you can't
    /// hold a mutex lock across await points.
    pub fn start_download_in_background(source: super::models::ImportSource) -> Ulid {
        // Create the download item
        let item = DownloadQueueItem::new(source.url, source.output_dir, source.headers);

        // Get the handle - this only locks the mutex briefly
        let (id, mut handle) = {
            let mut queue = DOWNLOAD_QUEUE.lock().unwrap();
            let handle = queue.add(item);
            (handle.id, handle)
        }; // Lock is released here

        // Spawn a background task to monitor the download (if needed)
        // This doesn't hold any mutex locks
        tokio::spawn(async move {
            let result = handle.wait_until_done().await;
            match result {
                Ok(path) => {
                    info!("Background download completed: id={}, path={:?}", id, path);
                    // Additional processing could happen here
                }
                Err(e) => {
                    error!("Background download failed: id={}, error={}", id, e);
                }
            }
        });

        // Return the ID so the caller can check status later if needed
        id
    }
}
