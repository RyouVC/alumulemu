//! Data structures for the download system
//!
//! This file contains the core data types used throughout the download system,
//! including progress tracking, status enums, and queue items.

use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
};

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

impl fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
    #[allow(dead_code)]
    pub fn is_successful(&self) -> bool {
        matches!(self.status, DownloadStatus::Completed)
    }

    /// Get the error message if the download failed
    #[allow(dead_code)]
    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            DownloadStatus::Failed(err) => Some(err),
            _ => None,
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
    #[serde(skip_serializing_if = "Option::is_none")] // Don't save headers to DB
    pub headers: Option<HashMap<String, String>>,
}

impl DownloadQueueItem {
    /// Creates a new `DownloadQueueItem` with the specified URL, output path, and optional headers
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download from
    /// * `output_path` - The path to save the downloaded file to (can be a directory)
    /// * `headers` - Optional custom headers for the download request
    ///
    /// # Returns
    ///
    /// A new `DownloadQueueItem` with default values for other fields
    pub fn new<P: AsRef<Path>>(
        url: impl Into<String>,
        output_path: P,
        headers: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            id: None,
            url: url.into(),
            output_path: output_path.as_ref().to_path_buf(),
            progress: Progress::default(),
            created_at: None,
            headers, // Add headers here
        }
    }

    pub async fn save(&self) -> color_eyre::Result<()> {
        if let Some(id) = &self.id {
            // Extract just the ID part without the table prefix
            // Instead of using id.to_string() which gives "download_queue:ULID",
            // we'll use id.id.to_string() to get just the ULID part
            let id_raw = id.id.to_string();
            tracing::trace!("Saving download progress with id: {}", id_raw);

            let _: Option<Self> = crate::db::DB
                .upsert(("download_queue", id_raw))
                .content(self.clone())
                .await?;
            Ok(())
        } else {
            Err(color_eyre::eyre::eyre!("Cannot save item without ID"))
        }
    }
}

/// Represents an import source with the necessary information to download a file
#[derive(Debug, Clone)]
pub struct ImportSource {
    pub url: String,
    pub output_dir: PathBuf,
    pub headers: Option<HashMap<String, String>>,
}

/// Parse filename from Content-Disposition header
/// Returns Some(filename) if successful, None otherwise
#[tracing::instrument(level = "trace", ret)]
pub fn parse_content_disposition(content_disposition: &str) -> Option<String> {
    tracing::trace!(content_disposition = %content_disposition, "Parsing Content-Disposition");

    // Look for filename="..." pattern (RFC 6266)
    if let Some(pos) = content_disposition.find("filename=\"") {
        let start = pos + "filename=\"".len();
        if let Some(end) = content_disposition[start..].find('"') {
            let filename = content_disposition[start..(start + end)].to_string();
            tracing::trace!(filename = %filename, "Found quoted filename");
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
                tracing::trace!(filename = %filename, "Found encoded filename");
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
            tracing::trace!(filename = %filename, "Found unquoted filename");
            return Some(filename);
        }
    }

    tracing::trace!("No filename found");
    None
}

// Custom error type that includes download progress information
#[derive(Debug)]
pub struct PartialDownloadError {
    pub bytes_downloaded: u64,
    pub source: std::io::Error,
}

impl fmt::Display for PartialDownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Download failed after {} bytes: {}",
            self.bytes_downloaded, self.source
        )
    }
}

impl std::error::Error for PartialDownloadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
