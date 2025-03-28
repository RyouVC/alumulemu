use futures_util::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
use tracing::{Level, debug, error, info, instrument, span, trace, warn};
use ulid::Ulid;

use crate::db::DB;

pub static DOWNLOAD_QUEUE: std::sync::LazyLock<std::sync::Mutex<DownloadQueue>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(DownloadQueue::new()));

/// Status of a download
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Download has been queued but not started yet
    Queued,
    /// Download is in progress
    Downloading,
    /// Download has been paused
    Paused,
    /// Download completed successfully
    Completed,
    /// Download was cancelled by user
    Cancelled,
    /// Download failed with an error
    Failed(String),
}

impl Default for DownloadStatus {
    fn default() -> Self {
        Self::Queued
    }
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "Queued"),
            Self::Downloading => write!(f, "Downloading"),
            Self::Paused => write!(f, "Paused"),
            Self::Completed => write!(f, "Completed"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Failed(err) => write!(f, "Failed: {}", err),
        }
    }
}

/// Represents the progress of a download
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    /// Total size of the download in bytes (if known)
    pub total_size: Option<u64>,
    /// Number of bytes downloaded so far
    pub downloaded: u64,
    /// Status of the download
    pub status: DownloadStatus,
    /// Final path of the downloaded file (once known)
    pub file_path: Option<PathBuf>,
}

impl Progress {
    /// Calculate the download progress as a percentage
    ///
    /// Returns None if the total size is unknown
    pub fn percentage(&self) -> Option<f32> {
        self.total_size
            .map(|total| (self.downloaded as f32 / total as f32) * 100.0)
    }

    /// Check if the download is complete (either successfully or with failure)
    pub fn is_complete(&self) -> bool {
        matches!(
            self.status,
            DownloadStatus::Completed | DownloadStatus::Failed(_) | DownloadStatus::Cancelled
        )
    }

    /// Check if the download completed successfully
    pub fn is_successful(&self) -> bool {
        matches!(self.status, DownloadStatus::Completed)
    }

    /// Get the error message if the download failed
    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            DownloadStatus::Failed(err) => Some(err),
            _ => None,
        }
    }
}

// Download handle returned to caller for tracking progress and cancellation
#[derive(Debug)]
pub struct DownloadHandle {
    pub id: Ulid,
    pub progress_rx: watch::Receiver<Progress>,
    cancellation_token: CancellationToken,
}

impl DownloadHandle {
    fn new(id: Ulid, progress_rx: watch::Receiver<Progress>, token: CancellationToken) -> Self {
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
    pub async fn wait_until_done(&mut self) -> Result<PathBuf, String> {
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

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DownloadQueueItem {
    pub id: Option<surrealdb::sql::Thing>,
    pub url: String,
    pub output_path: PathBuf,
    pub progress: Progress,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl DownloadQueueItem {
    /// Creates a new `DownloadQueueItem` with the specified URL and output path
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download from
    /// * `output_path` - The path to save the downloaded file to (can be a directory)
    ///
    /// # Returns
    ///
    /// A new `DownloadQueueItem` with default values for other fields
    pub fn new<P: AsRef<Path>>(url: impl Into<String>, output_path: P) -> Self {
        Self {
            id: None,
            url: url.into(),
            output_path: output_path.as_ref().to_path_buf(),
            progress: Progress::default(),
            created_at: None,
        }
    }

    pub async fn save(&self) -> color_eyre::Result<()> {
        if let Some(id) = &self.id {
            // Extract just the ID part without the table prefix
            // Instead of using id.to_string() which gives "download_queue:ULID",
            // we'll use id.id.to_string() to get just the ULID part
            let id_raw = id.id.to_string();
            trace!("Saving download progress with id: {}", id_raw);

            let _: Option<Self> = DB
                .upsert(("download_queue", id_raw))
                .content(self.clone())
                .await?;
            Ok(())
        } else {
            Err(color_eyre::eyre::eyre!("Cannot save item without ID"))
        }
    }
}

#[derive(Debug, Default)]
pub struct DownloadQueue {
    downloads: HashMap<Ulid, (DownloadQueueItem, tokio::task::JoinHandle<()>)>,
    progress_watchers: HashMap<Ulid, watch::Sender<Progress>>,
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
    pub async fn cleanup(&mut self) {
        let completed_ids: Vec<Ulid> = self
            .downloads
            .iter()
            .filter(|(_, (_, handle))| handle.is_finished())
            .map(|(id, _)| *id)
            .collect();

        for id in completed_ids {
            self.downloads.remove(&id);
            self.progress_watchers.remove(&id);
        }
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
    pub fn start_download_in_background(source: ImportSource) -> Ulid {
        // Create the download item
        let item = DownloadQueueItem::new(source.url, source.output_dir);

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

/// Represents an import source with the necessary information to download a file
#[derive(Debug, Clone)]
pub struct ImportSource {
    pub url: String,
    pub output_dir: PathBuf,
}

pub struct Downloader {
    client: Client,
    max_redirects: usize,
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

impl Downloader {
    pub fn new() -> Self {
        // Setup headers exactly like curl
        let mut headers = HeaderMap::new();
        // headers.insert(header::USER_AGENT, HeaderValue::from_static("curl/8.9.1"));
        headers.insert(header::ACCEPT, HeaderValue::from_static("*/*"));

        // Create a client that doesn't follow redirects automatically
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .build()
            .unwrap();

        Self {
            client,
            max_redirects: 10,
        }
    }

    pub fn with_max_redirects(mut self, max_redirects: usize) -> Self {
        self.max_redirects = max_redirects;
        self
    }

    pub async fn download_file<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
    ) -> io::Result<PathBuf> {
        // Create a null channel that drops all progress updates
        let (tx, _) = mpsc::channel(10);
        self.download_file_with_progress(url, output_path, tx).await
    }

    pub async fn download_file_with_progress<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        progress_tx: mpsc::Sender<Progress>,
    ) -> io::Result<PathBuf> {
        let response = self.get_with_redirects(url).await?;

        // Check if output_path is a directory
        let output_path_ref = output_path.as_ref();
        let final_path = if output_path_ref.is_dir() {
            // Try to extract filename from Content-Disposition header
            let filename = if let Some(content_disposition) =
                response.headers().get(header::CONTENT_DISPOSITION)
            {
                println!("Content-Disposition: {:?}", content_disposition);

                let content_disposition_str = content_disposition
                    .to_str()
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                // Parse Content-Disposition for filename
                // Example: attachment; filename="filename.zip"
                parse_content_disposition(content_disposition_str)
            } else {
                None
            };

            // If we couldn't get filename from Content-Disposition, try to get it from the URL
            let filename = filename
                .or_else(|| {
                    let binding = Url::parse(url).ok()?;
                    let url_path = binding.path();
                    let path = Path::new(url_path);
                    path.file_name()?.to_str().map(|s| s.to_string())
                })
                .unwrap_or_else(|| {
                    // If all else fails, use a generic filename with timestamp
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    format!("download_{}.bin", now)
                });

            output_path_ref.join(filename)
        } else {
            output_path_ref.to_path_buf()
        };

        // Get content length if available
        let total_size = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|cl| cl.to_str().ok())
            .and_then(|cl| cl.parse::<u64>().ok());

        // Create output file using tokio's async file operations
        let mut file = File::create(&final_path).await?;

        // Stream the response to file
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        // Send initial progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded: 0,
                status: DownloadStatus::Downloading,
                file_path: Some(final_path.clone()),
            })
            .await;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(chunk) => {
                    let chunk_size = chunk.len() as u64;
                    file.write_all(&chunk).await?;

                    // Update download progress
                    downloaded += chunk_size;

                    // Send progress update
                    let _ = progress_tx
                        .send(Progress {
                            total_size,
                            downloaded,
                            status: DownloadStatus::Downloading,
                            file_path: Some(final_path.clone()),
                        })
                        .await;
                }
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
            }
        }

        // Send final progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded,
                status: DownloadStatus::Completed,
                file_path: Some(final_path.clone()),
            })
            .await;

        // Return the actual path used for the download
        Ok(final_path)
    }

    #[instrument(name = "download_file", level = "debug", skip(self, progress_tx, cancel_token, output_path), fields(url = %url))]
    pub async fn download_file_with_progress_cancellable<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        progress_tx: mpsc::Sender<Progress>,
        cancel_token: CancellationToken,
    ) -> io::Result<PathBuf> {
        trace!("Starting download with progress tracking");
        let response = self.get_with_redirects(url).await?;
        trace!(status = %response.status(), "Got response");

        // Check if already canceled
        if cancel_token.is_cancelled() {
            info!("Download cancelled before starting");
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Download cancelled",
            ));
        }

        // Check if output_path is a directory
        let output_path_ref = output_path.as_ref();
        let final_path = if output_path_ref.is_dir() {
            // Try to extract filename from Content-Disposition header
            let filename = if let Some(content_disposition) =
                response.headers().get(header::CONTENT_DISPOSITION)
            {
                trace!(content_disposition = ?content_disposition, "Content-Disposition header found");

                let content_disposition_str = content_disposition.to_str().map_err(|e| {
                    warn!(error = %e, "Failed to convert Content-Disposition to string");
                    io::Error::new(io::ErrorKind::InvalidData, e)
                })?;

                // Parse Content-Disposition for filename
                let parsed_filename = parse_content_disposition(content_disposition_str);
                if let Some(ref name) = parsed_filename {
                    debug!(filename = %name, "Extracted filename from Content-Disposition");
                }
                parsed_filename
            } else {
                trace!("No Content-Disposition header found");
                None
            };

            // If we couldn't get filename from Content-Disposition, try to get it from the URL
            let filename = filename
                .or_else(|| {
                    trace!("Attempting to extract filename from URL");
                    let binding = Url::parse(url).ok()?;
                    let url_path = binding.path();
                    let path = Path::new(url_path);
                    let filename = path.file_name()?.to_str().map(|s| s.to_string());

                    if let Some(ref name) = filename {
                        debug!(filename = %name, "Extracted filename from URL path");
                    }

                    filename
                })
                .unwrap_or_else(|| {
                    // If all else fails, use a generic filename with timestamp
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let generic_name = format!("download_{}.bin", now);
                    debug!(filename = %generic_name, "Using generic filename");
                    generic_name
                });

            let final_path = output_path_ref.join(&filename);
            debug!(path = ?final_path, "Final download path");
            final_path
        } else {
            debug!(path = ?output_path_ref, "Using specified file path");
            output_path_ref.to_path_buf()
        };

        // Get content length if available
        let total_size = response
            .headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|cl| cl.to_str().ok())
            .and_then(|cl| cl.parse::<u64>().ok());

        if let Some(size) = total_size {
            info!(bytes = size, path = ?final_path, "Starting download");
        } else {
            info!(path = ?final_path, "Starting download of unknown size");
        }

        // Create output file
        let mut file = File::create(&final_path).await?;

        // Stream the response to file
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        // Send initial progress update with the file path
        trace!("Sending initial progress update");
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded: 0,
                status: DownloadStatus::Downloading,
                file_path: Some(final_path.clone()),
            })
            .await;

        // Create a span for chunk processing
        let chunks_span = span!(Level::TRACE, "download_chunks", path = ?final_path);
        let _guard = chunks_span.enter();

        while let Some(chunk) = stream.next().await {
            // Check for cancellation
            if cancel_token.is_cancelled() {
                info!(
                    downloaded = downloaded,
                    "Download cancelled during progress"
                );
                // Close and delete the partial file
                let _ = file.shutdown().await;
                let _ = tokio::fs::remove_file(&final_path).await;
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Download cancelled",
                ));
            }

            match chunk {
                Ok(chunk) => {
                    let chunk_size = chunk.len() as u64;
                    file.write_all(&chunk).await?;

                    // Update download progress
                    downloaded += chunk_size;

                    // Log chunk at trace level
                    trace!(
                        bytes = downloaded,
                        chunk_size = chunk_size,
                        "Received chunk"
                    );

                    // Send progress update
                    let _ = progress_tx
                        .send(Progress {
                            total_size,
                            downloaded,
                            status: DownloadStatus::Downloading,
                            file_path: Some(final_path.clone()),
                        })
                        .await;
                }
                Err(e) => {
                    error!(error = %e, "Error downloading chunk");
                    return Err(io::Error::new(io::ErrorKind::Other, e));
                }
            }
        }

        // Make sure the file is completely written
        file.flush().await?;
        file.shutdown().await?;

        info!(bytes = downloaded, "Download completed");

        // Send final progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded,
                status: DownloadStatus::Completed,
                file_path: Some(final_path.clone()),
            })
            .await;

        Ok(final_path)
    }

    pub async fn get_with_redirects(&self, url: &str) -> io::Result<Response> {
        let mut current_url = url.to_string();
        let mut redirect_count = 0;

        loop {
            // Send request
            let response = match self.client.get(&current_url).send().await {
                Ok(resp) => resp,
                Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
            };

            // If not a redirect or we've hit the max, return this response
            if !response.status().is_redirection() || redirect_count >= self.max_redirects {
                return Ok(response);
            }

            // Extract location header for the redirect
            let location = match response.headers().get(header::LOCATION) {
                Some(loc) => {
                    let loc_str = loc
                        .to_str()
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

                    // Handle relative URLs
                    let base_url = Url::parse(&current_url)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;

                    base_url
                        .join(loc_str)
                        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?
                        .to_string()
                }
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Redirect without Location header",
                    ));
                }
            };

            // Update for next iteration
            current_url = location;
            redirect_count += 1;
        }
    }
}

/// Parse filename from Content-Disposition header
/// Returns Some(filename) if successful, None otherwise
#[instrument(level = "trace", ret)]
fn parse_content_disposition(content_disposition: &str) -> Option<String> {
    trace!(content_disposition = %content_disposition, "Parsing Content-Disposition");

    // Look for filename="..." pattern (RFC 6266)
    if let Some(pos) = content_disposition.find("filename=\"") {
        let start = pos + "filename=\"".len();
        if let Some(end) = content_disposition[start..].find('"') {
            let filename = content_disposition[start..(start + end)].to_string();
            trace!(filename = %filename, "Found quoted filename");
            return Some(filename);
        }
    }

    // Look for filename*=UTF-8''... pattern (RFC 5987)
    if let Some(pos) = content_disposition.find("filename*=UTF-8''") {
        let start = pos + "filename*=UTF-8''".len();
        let end = content_disposition[start..]
            .find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(content_disposition[start..].len());

        if end > 0 {
            // URL decode the filename
            let encoded_filename = &content_disposition[start..(start + end)];
            if let Ok(decoded) = urlencoding::decode(encoded_filename) {
                let filename = decoded.to_string();
                trace!(filename = %filename, "Found encoded filename");
                return Some(filename);
            }
        }
    }

    // Look for filename=... (without quotes)
    if let Some(pos) = content_disposition.find("filename=") {
        let start = pos + "filename=".len();
        let end = content_disposition[start..]
            .find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(content_disposition[start..].len());

        if end > 0 {
            let filename = content_disposition[start..(start + end)].to_string();
            trace!(filename = %filename, "Found unquoted filename");
            return Some(filename);
        }
    }

    trace!("No filename found");
    None
}
