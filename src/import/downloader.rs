use futures_util::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderValue};
use reqwest::{Client, Response, Url};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;

use crate::db::DB;

pub static DOWNLOAD_QUEUE: std::sync::LazyLock<std::sync::Mutex<DownloadQueue>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(DownloadQueue::new()));

/// Represents the progress of a download
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    /// Total size of the download in bytes (if known)
    pub total_size: Option<u64>,
    /// Number of bytes downloaded so far
    pub downloaded: u64,
    /// Whether the download is complete
    pub complete: bool,
    /// Error message if download failed
    pub error: Option<String>,
}

impl Progress {
    /// Calculate the download progress as a percentage
    ///
    /// Returns None if the total size is unknown
    pub fn percentage(&self) -> Option<f32> {
        self.total_size
            .map(|total| (self.downloaded as f32 / total as f32) * 100.0)
    }
}

// Download handle returned to caller for tracking progress and cancellation
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

            if progress.complete {
                return match progress.error {
                    Some(error) => Err(error),
                    // Return the output path from the DownloadQueueItem
                    None => {
                        // Get the file path from the queue
                        let item: DownloadQueueItem = DB
                            .select(("download_queue", self.id.to_string()))
                            .await
                            .map_err(|e| format!("Failed to retrieve download info: {}", e))?
                            .ok_or_else(|| "Download info not found in database".to_string())?;

                        Ok(item.output_path)
                    }
                };
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct DownloadQueueItem {
    pub id: Option<Ulid>,
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
        let _: Option<Self> = DB
            .upsert(("download_queue", self.id.as_ref().unwrap().to_string()))
            .content(self.clone())
            .await?;
        Ok(())
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
        let id = Ulid::new();
        // Set the ID in the item
        item.id = Some(id);
        item.created_at = Some(chrono::Utc::now());

        // Create a watch channel for progress updates
        let (progress_tx, progress_rx) = watch::channel(Progress::default());

        let cancellation_token = CancellationToken::new();

        // Clone what we need to move into the task
        let url = item.url.clone();
        let output_path = item.output_path.clone();
        let token_clone = cancellation_token.clone();

        // Save the progress transmitter for later use
        self.progress_watchers.insert(id, progress_tx.clone());

        // Clone for the second task before first move
        let progress_tx_clone = progress_tx.clone();

        // Start the download task
        let handle = tokio::spawn(async move {
            // Create a channel for receiving progress updates
            let (internal_tx, mut internal_rx) = mpsc::channel(10);

            let downloader = Downloader::new();
            let result = downloader
                .download_file_with_progress_cancellable(
                    &url,
                    &output_path,
                    internal_tx,
                    token_clone.clone(),
                )
                .await;

            // Forward final status based on result
            let final_progress = match result {
                Ok(_) => {
                    // Wait for any final progress updates
                    if let Ok(Some(progress)) = tokio::time::timeout(
                        std::time::Duration::from_millis(100),
                        internal_rx.recv(),
                    )
                    .await
                    {
                        progress
                    } else {
                        // Create a completion message if we didn't get one
                        Progress {
                            complete: true,
                            error: None,
                            ..progress_tx.borrow().clone()
                        }
                    }
                }
                Err(e) => Progress {
                    error: Some(e.to_string()),
                    complete: true,
                    ..progress_tx.borrow().clone()
                },
            };

            // Send final update
            let _ = progress_tx.send(final_progress);
        });

        // Start a task to monitor the progress channel and update the item
        let item_clone = item.clone();
        tokio::spawn(async move {
            Self::monitor_progress_static(id, item_clone, progress_tx_clone).await;
        });

        // Store the download information
        self.downloads.insert(id, (item, handle));

        // Return the handle to the caller
        DownloadHandle::new(id, progress_rx, cancellation_token)
    }

    // Monitor progress for a specific download and update database
    // Static version of monitor_progress that doesn't require &self
    async fn monitor_progress_static(
        id: Ulid,
        mut item: DownloadQueueItem,
        progress_tx: watch::Sender<Progress>,
    ) {
        let (internal_tx, mut internal_rx) = mpsc::channel::<Progress>(10);

        // Forward progress updates to the watch channel
        tokio::spawn(async move {
            while let Some(progress) = internal_rx.recv().await {
                // Update the item with latest progress
                item.progress = progress.clone();
                let _ = item.save().await;

                // Update the watch channel
                let _ = progress_tx.send(progress);
            }
        });
    }

    pub fn cancel(&mut self, id: &Ulid) -> bool {
        if let Some((_, handle)) = self.downloads.get(id) {
            handle.abort();
            // Update progress with cancelled status
            if let Some(progress_tx) = self.progress_watchers.get(id) {
                let mut current = progress_tx.borrow().clone();
                current.error = Some("Download cancelled".to_string());
                current.complete = true;
                let _ = progress_tx.send(current);
            }
            self.downloads.remove(id);
            self.progress_watchers.remove(id);
            true
        } else {
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

        // Update progress for all active downloads
        for item in db_items {
            if let Some(id) = item.id {
                if let Some((queue_item, _)) = self.downloads.get_mut(&id) {
                    // Update the queue item with the database version
                    *queue_item = item;
                }
            }
        }

        Ok(())
    }
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
                complete: false,
                error: None,
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
                            complete: false,
                            error: None,
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
                complete: true,
                error: None,
            })
            .await;

        // Return the actual path used for the download
        Ok(final_path)
    }

    pub async fn download_file_with_progress_cancellable<P: AsRef<Path>>(
        &self,
        url: &str,
        output_path: P,
        progress_tx: mpsc::Sender<Progress>,
        cancel_token: CancellationToken,
    ) -> io::Result<PathBuf> {
        let response = self.get_with_redirects(url).await?;

        // Check if already canceled
        if cancel_token.is_cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Download cancelled",
            ));
        }

        // Check if output_path is a directory
        let output_path_ref = output_path.as_ref();
        let final_path = if output_path_ref.is_dir() {
            // Try to extract filename from Content-Disposition header or URL
            // ...existing code for filename extraction...

            // This is simplified for the response - in real code you'd put the existing filename extraction logic here
            let filename = parse_content_disposition(
                response
                    .headers()
                    .get(header::CONTENT_DISPOSITION)
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or(""),
            )
            .unwrap_or_else(|| {
                Url::parse(url)
                    .ok()
                    .and_then(|u| Path::new(u.path()).file_name()?.to_str().map(String::from))
                    .unwrap_or_else(|| {
                        format!(
                            "download_{}.bin",
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        )
                    })
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

        // Create output file
        let mut file = File::create(&final_path).await?;

        // Stream the response to file
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        // Send initial progress update
        let _ = progress_tx
            .send(Progress {
                total_size,
                downloaded: 0,
                complete: false,
                error: None,
            })
            .await;

        while let Some(chunk) = stream.next().await {
            // Check for cancellation
            if cancel_token.is_cancelled() {
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

                    // Send progress update
                    let _ = progress_tx
                        .send(Progress {
                            total_size,
                            downloaded,
                            complete: false,
                            error: None,
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
                complete: true,
                error: None,
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
fn parse_content_disposition(content_disposition: &str) -> Option<String> {
    // Look for filename="..." or filename*=... patterns
    if let Some(pos) = content_disposition.find("filename=\"") {
        let start = pos + "filename=\"".len();
        if let Some(end) = content_disposition[start..].find('"') {
            return Some(content_disposition[start..(start + end)].to_string());
        }
    }

    // Look for filename=... (without quotes)
    if let Some(pos) = content_disposition.find("filename=") {
        let start = pos + "filename=".len();
        let end = content_disposition[start..]
            .find(|c: char| c.is_whitespace() || c == ';')
            .unwrap_or(content_disposition[start..].len());
        if end > 0 {
            return Some(content_disposition[start..(start + end)].to_string());
        }
    }

    None
}
