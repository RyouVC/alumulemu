//! Module for downloading files from HTTP sources
//!
//! This module provides functionality for downloading files from HTTP sources,
//! managing download queues, and tracking download progress.

mod http;
mod models;
mod queue;

// Re-export the public API
pub use http::Downloader;
pub use models::{DownloadQueueItem, DownloadStatus, ImportSource, Progress};
pub use queue::{DOWNLOAD_QUEUE, DownloadHandle, DownloadQueue};

// Re-export utility functions
pub use models::parse_content_disposition;
