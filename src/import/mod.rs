//! Primary module for importers.
//!
//! Importers in this case are responsible for importing packages from various sources
//! into alumulemu. This can be done in a variety of ways, such as:
//!
//! - Manually uploading a package file
//! - Downloading a package from a remote source
//! - Merging with an existing repository of packages
//!
//! # Adding a New Importer
//!
//! This section provides a step-by-step guide on how to implement a new importer.
//!
//! ## 1. Create a New Module
//!
//! Create a new file for your importer (e.g., `my_importer.rs`) in the `import` directory
//! and add it to the module declarations at the top of `mod.rs`.
//!
//! ```rust
//! // In mod.rs
//! pub mod my_importer;
//! ```
//!
//! ## 2. Define Your Importer Struct and Request Type
//!
//! ```rust
//! use crate::import::{Importer, Result, ImportSource, ImportError};
//! use serde::Deserialize;
//!
//! #[derive(Clone, Debug)]
//! pub struct MyImporter {
//!     // Add any fields your importer needs
//!     client: reqwest::Client,
//! }
//!
//! // Import request type for your importer - must implement Deserialize
//! #[derive(Debug, Deserialize)]
//! pub struct MyImportRequest {
//!     pub id: String,
//!     pub some_option: Option<bool>,
//!     // Other request fields
//! }
//!
//! impl MyImporter {
//!     pub fn new() -> Self {
//!         Self {
//!             client: reqwest::Client::new(),
//!         }
//!     }
//!     
//!     // Add helper methods for your importer
//! }
//! ```
//!
//! ## 3. Implement the Importer Trait
//!
//! ```rust
//! impl Importer for MyImporter {
//!     type ImportRequest = MyImportRequest;
//!
//!     async fn import(&self, request: Self::ImportRequest) -> Result<ImportSource> {
//!         // Implement your import logic here
//!         // Example:
//!         let url = format!("https://example.com/api/game/{}", request.id);
//!         let response = self.client.get(&url).send().await?;
//!         
//!         if response.status() == 404 {
//!             return Err(ImportError::GameNotFound);
//!         }
//!         
//!         // Process the response and return an ImportSource
//!         let download_url = "https://example.com/download/game.nsp";
//!         Ok(ImportSource::RemoteHttpAuto(download_url.to_string()))
//!     }
//!
//!     fn name(&self) -> &'static str {
//!         "my_importer"
//!     }
//!
//!     fn display_name(&self) -> &'static str {
//!         "My Custom Importer"
//!     }
//!
//!     fn description(&self) -> &'static str {
//!         "Imports games from my custom source"
//!     }
//! }
//! ```
//!
//! ## 4. Register Your Importer (if necessary)
//!
//! If your importer needs to be available through a registry or API endpoint,
//! you'll need to register it. This depends on how your application is structured,
//! but typically involves adding your importer to a collection or registry.
//!
//! ## 5. Testing Your Importer
//!
//! Add tests for your importer to ensure it works correctly:
//!
//! ```rust
//! #[cfg(test)]
//! mod tests {
//!     use super::*;
//!     
//!     #[tokio::test]
//!     async fn test_my_importer() {
//!         let importer = MyImporter::new();
//!         
//!         // Create a test request
//!         let request = MyImportRequest {
//!             id: "test-id".to_string(),
//!             some_option: Some(true),
//!         };
//!         
//!         // Test import functionality
//!         let result = importer.import(request).await;
//!         assert!(result.is_ok());
//!     }
//! }
//! ```
//!
//! That's it! Your importer is now ready to use and can be integrated with the rest of the system.
//! The `ImportSource` enum provides flexible ways to handle different import sources, and the
//! importer framework handles the actual file processing and importing.
//!

use async_zip::tokio::read::seek::ZipFileReader;
use downloader::{DOWNLOAD_QUEUE, DownloadQueueItem};
use futures::future::join_all;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::{fs::File, io::BufReader};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, info};
pub mod dbi;
pub mod downloader;
pub mod import_utils;
pub mod not_ultranx;
pub mod registry;
pub mod tests;
pub mod url;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NxDevice {
    pub serial: String,
    pub device_id: String,
}
impl NxDevice {
    const DBI_VERSION: &'static str = "781-ru";
    const SWITCH_FIRMWARE: &'static str = "19.0.1E";
    pub fn new(serial: String, device_id: String) -> Self {
        Self { serial, device_id }
    }

    pub fn dbi_user_agent(&self) -> String {
        format!(
            "DBI/{dbi_ver} (FW: {fw_ver}; SN: {serial}; DeviceId: {device_id})",
            dbi_ver = Self::DBI_VERSION,
            fw_ver = Self::SWITCH_FIRMWARE,
            serial = self.serial,
            device_id = self.device_id
        )
    }
}

#[derive(Error, Debug)]
pub enum ImportError {
    // IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    // Serde errors
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("Game Not Found in importer source")]
    GameNotFound,

    #[error("Zip error: {0}")]
    ZipError(#[from] async_zip::error::ZipError),

    // Mutex errors
    #[error("Mutex lock error: {0}")]
    MutexError(String),

    // Other errors
    #[error("{0:?}")]
    Other(#[from] color_eyre::eyre::Report),
}

// Add From implementation for PoisonError
impl<T> From<std::sync::PoisonError<std::sync::MutexGuard<'_, T>>> for ImportError {
    fn from(err: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>) -> Self {
        ImportError::MutexError(err.to_string())
    }
}
/// Recursively move a path to another location, handling cross-filesystem moves.
///
/// This is a more robust version of `tokio::fs::rename` that can handle cross-filesystem moves
/// by manually copying files and directories.
pub async fn recursive_move(src: &Path, dest: &Path) -> Result<()> {
    debug!(from = ?src, to = ?dest, "Moving path");

    if src.is_dir() {
        // Try atomic rename (fast path)
        if tokio::fs::rename(src, dest).await.is_ok() {
            return Ok(());
        }

        // Manual copy for cross-filesystem moves
        tokio::fs::create_dir_all(dest).await?;

        let walker = jwalk::WalkDir::new(src);
        for entry in walker.into_iter().filter_map(|entry| entry.ok()) {
            let path = entry.path();
            let relative = path.strip_prefix(src).unwrap();
            let dest_path = dest.join(relative);

            if path.is_dir() {
                tokio::fs::create_dir_all(&dest_path).await?;
            } else if tokio::fs::rename(&path, &dest_path).await.is_err() {
                tokio::fs::copy(&path, &dest_path).await?;
                tokio::fs::remove_file(&path).await?;
            }
        }

        tokio::fs::remove_dir_all(src).await?;
    } else if tokio::fs::rename(src, dest).await.is_err() {
        // Make sure parent directory exists
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::copy(src, dest).await?;
        tokio::fs::remove_file(src).await?;
    }

    debug!(from = ?src, to = ?dest, "Path moved successfully");
    Ok(())
}

pub fn download_path() -> PathBuf {
    let config = crate::config::config();
    let path = PathBuf::from(config.backend_config.cache_dir.clone()).join("downloads");
    // Ensure the download directory exists
    std::fs::create_dir_all(&path).unwrap_or_else(|e| {
        debug!("Failed to create download directory: {}", e);
    });
    path
}

pub enum ImportSource {
    /// A single local file to import
    Local(PathBuf),
    /// A local archive file that will be extracted
    LocalArchive(PathBuf),
    /// A local directory containing files to import
    LocalDir(PathBuf),
    /// (Not implemented) Generic remote import
    Remote,
    /// A remote file accessed via HTTP
    RemoteHttp {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    /// A remote archive file accessed via HTTP that will be extracted
    RemoteHttpArchive {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    /// A remote HTTP source that will automatically determine if it's an archive based on the downloaded file extension
    RemoteHttpAuto {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
    /// (Not implemented) Import from a repository
    Repository,

    RemoteHttpAutoList {
        urls: Vec<String>,
        headers: Option<HashMap<String, String>>,
    },
}

impl ImportSource {
    /// Creates a new RemoteHttpAuto import source which automatically determines type from file extension
    pub fn new_remote_http_auto(
        url: impl Into<String>,
        headers: Option<HashMap<String, String>>,
    ) -> Self {
        Self::RemoteHttpAuto {
            url: url.into(),
            headers,
        }
    }

    pub async fn process(&self) -> Result<(Vec<PathBuf>, Option<tempfile::TempDir>)> {
        match self {
            ImportSource::Local(path) => Ok((vec![path.to_path_buf()], None)),
            ImportSource::LocalArchive(path) => {
                let result = self.extract_archive(path).await?;
                // Delete the archive after successful extraction
                if tokio::fs::remove_file(path).await.is_err() {
                    debug!(archive = ?path, "Failed to remove archive file after extraction");
                } else {
                    info!(archive = ?path, "Removed archive file after successful extraction");
                }
                Ok(result)
            }
            ImportSource::LocalDir(path) => {
                let walker = jwalk::WalkDir::new(path);
                let files: Vec<PathBuf> = walker
                    .into_iter()
                    .filter_map(|entry| entry.ok())
                    .filter(|entry| entry.file_type().is_file())
                    .map(|entry| entry.path().to_path_buf())
                    .collect();
                Ok((files, None))
            }
            ImportSource::RemoteHttp { url, headers } => {
                let path = Self::download_http(url, headers.clone()).await?;
                Ok((vec![path], None))
            }
            ImportSource::RemoteHttpArchive { url, headers } => {
                let path = Self::download_http(url, headers.clone()).await?;
                let result = self.extract_archive(&path).await?;
                // Delete the downloaded archive after successful extraction
                if tokio::fs::remove_file(&path).await.is_err() {
                    debug!(archive = ?path, "Failed to remove downloaded archive file after extraction");
                } else {
                    info!(archive = ?path, "Removed downloaded archive file after successful extraction");
                }
                Ok(result)
            }
            ImportSource::RemoteHttpAuto { url, headers } => {
                let path = Self::download_http(url, headers.clone()).await?;

                // Check if the downloaded file appears to be an archive based on extension
                let is_archive = self.is_archive_file(&path);

                if is_archive {
                    info!(path = ?path, "Auto-detected archive file, extracting");
                    let result = self.extract_archive(&path).await?;
                    // Delete the downloaded archive after successful extraction
                    if tokio::fs::remove_file(&path).await.is_err() {
                        debug!(archive = ?path, "Failed to remove downloaded archive file after successful extraction");
                    } else {
                        info!(archive = ?path, "Removed downloaded archive file after successful extraction");
                    }
                    Ok(result)
                } else {
                    info!(path = ?path, "Auto-detected regular file (non-archive)");
                    Ok((vec![path], None))
                }
            }

            ImportSource::RemoteHttpAutoList { urls, headers } => {
                let download_futures = urls
                    .iter()
                    .map(|url| Self::download_http(url, headers.clone()));

                let download_results = join_all(download_futures).await;

                let mut downloaded_paths = Vec::new();
                for result in download_results {
                    match result {
                        Ok(path) => downloaded_paths.push(path),
                        Err(e) => {
                            // Decide how to handle individual download errors.
                            // Option 1: Return the first error encountered.
                            // return Err(e);
                            // Option 2: Log the error and continue with successful downloads.
                            tracing::error!("Failed to download one of the URLs: {}", e);
                            continue;
                            // Option 3: Collect all errors.
                            // errors.push(e); // Requires defining `errors` Vec earlier.
                        }
                    }
                }
                // If collecting errors (Option 3), check if errors occurred and return appropriately.
                // if !errors.is_empty() { ... }

                let mut output_files = Vec::new();
                let mut archive_paths = Vec::new();
                let mut temp_dir = None; // We'll create this only if needed

                // Separate archives from regular files
                for path in downloaded_paths {
                    if self.is_archive_file(&path) {
                        archive_paths.push(path);
                    } else {
                        output_files.push(path); // Add non-archives directly
                    }
                }

                // Process archives concurrently if any exist
                if !archive_paths.is_empty() {
                    let main_temp_dir = crate::util::tempdir()?; // Create one temp dir for all extractions
                    let temp_path = main_temp_dir.path().to_path_buf(); // Get path for the async block

                    let extraction_futures = archive_paths.into_iter().map(|path| {
                        // Clone path and temp_path for the async block
                        let path_clone = path.clone();
                        let temp_path_clone = temp_path.clone();
                        async move {
                            // Use the new helper function to extract to the shared temp dir
                            extract_archive_to(&path_clone, &temp_path_clone).await
                        }
                    });

                    let extraction_results = join_all(extraction_futures).await;

                    for result in extraction_results {
                        match result {
                            Ok(files) => output_files.extend(files),
                            Err(e) => {
                                // Handle extraction errors similarly to download errors
                                // Close the temp dir before returning the error
                                let _ = main_temp_dir.close();
                                return Err(e);
                                // Or log and continue, or collect errors
                                // tracing::error!("Failed to extract an archive: {}", e);
                            }
                        }
                    }
                    temp_dir = Some(main_temp_dir); // Assign the temp dir if extractions happened
                }

                Ok((output_files, temp_dir))
            }
            ImportSource::Remote => unimplemented!(
                "Generic Remote import source not implemented, this should be a generic remote import, but the details are not yet defined"
            ),
            ImportSource::Repository => unimplemented!(
                "Repository import source not implemented, This should take in a Tinfoil index"
            ),
        }
    }

    /// Determine if a file is likely an archive based on its extension
    fn is_archive_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
            matches!(
                extension.to_lowercase().as_str(),
                "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz"
            )
        } else {
            false
        }
    }

    pub fn new_local(path: impl Into<PathBuf>) -> Self {
        Self::Local(path.into())
    }

    /// Creates a new LocalArchive import source
    pub fn new_local_archive(path: impl Into<PathBuf>) -> Self {
        Self::LocalArchive(path.into())
    }

    /// Creates a new LocalDir import source
    pub fn new_local_dir(path: impl Into<PathBuf>) -> Self {
        Self::LocalDir(path.into())
    }

    /// Creates a new RemoteHttp import source
    pub fn new_remote_http(
        url: impl Into<String>,
        headers: Option<HashMap<String, String>>,
    ) -> Self {
        Self::RemoteHttp {
            url: url.into(),
            headers,
        }
    }

    /// Creates a new RemoteHttpArchive import source
    pub fn new_remote_http_archive(
        url: impl Into<String>,
        headers: Option<HashMap<String, String>>,
    ) -> Self {
        Self::RemoteHttpArchive {
            url: url.into(),
            headers,
        }
    }

    pub async fn download_http(
        url: &str,
        headers: Option<HashMap<String, String>>,
    ) -> Result<PathBuf> {
        let download_path = download_path();
        let queue_item = DownloadQueueItem::new(url, download_path, headers);

        // Create a scope to ensure the lock is dropped after getting the handle
        let mut handle = {
            let mut queue = DOWNLOAD_QUEUE.lock()?;
            queue.add(queue_item)
        };

        tracing::info!("Download added to queue with handle: {:?}", handle);

        if let Ok(path) = handle.wait_until_done().await {
            Ok(path)
        } else {
            Err(ImportError::Other(color_eyre::eyre::eyre!(
                "Download failed"
            )))
        }
    }

    // Directly import to the roms directory
    pub async fn import(&self) -> Result<()> {
        let config = crate::config::config();
        let rom_dir = config.backend_config.rom_dir.clone();
        let rom_dir = Path::new(&rom_dir);

        let (output_files, temp_dir) = self.process().await?;

        for file in output_files {
            // Determine if this is from a temp dir extraction
            if let Some(temp_dir) = &temp_dir {
                if let Ok(relative) = file.strip_prefix(temp_dir.path()) {
                    // Preserve directory structure by using the relative path
                    let dest = rom_dir.join(relative);

                    // Ensure parent directory exists
                    if let Some(parent) = dest.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }

                    recursive_move(&file, &dest).await?;
                } else {
                    // Fallback for files not in temp_dir
                    let dest = rom_dir.join(file.file_name().unwrap());
                    recursive_move(&file, &dest).await?;
                }
            } else {
                // For direct files (not from extraction)
                let dest = rom_dir.join(file.file_name().unwrap());
                recursive_move(&file, &dest).await?;
            }
        }

        if let Some(temp_dir) = temp_dir {
            let _ = temp_dir.close();
        }
        Ok(())
    }

    /// Extract an archive to a temporary directory
    async fn extract_archive(
        &self,
        path: &Path,
    ) -> Result<(Vec<PathBuf>, Option<tempfile::TempDir>)> {
        info!(archive_path = ?path, "Extracting archive to temporary directory");

        // Create temporary directory
        let temp_dir = crate::util::tempdir()?;
        let temp_path = temp_dir.path();

        // Extract the archive to the temporary directory
        let extracted_files = extract_zip_to_directory(path, temp_path).await?;

        info!(
            files_extracted = extracted_files.len(),
            temp_dir = ?temp_path,
            "Extraction complete"
        );

        Ok((extracted_files, Some(temp_dir)))
    }
}

/// Helper function to extract an archive to a specific directory and delete the archive afterwards.
/// Returns a list of paths to the extracted files.
async fn extract_archive_to(archive_path: &Path, destination: &Path) -> Result<Vec<PathBuf>> {
    info!(archive = ?archive_path, destination = ?destination, "Extracting archive");

    let extracted_files = extract_zip_to_directory(archive_path, destination).await?;

    // Delete the archive after successful extraction
    if tokio::fs::remove_file(archive_path).await.is_err() {
        debug!(archive = ?archive_path, "Failed to remove archive file after extraction");
    } else {
        info!(archive = ?archive_path, "Removed archive file after successful extraction");
    }

    Ok(extracted_files)
}

pub type Result<T> = std::result::Result<T, ImportError>;

/// Generic function to extract a zip file to any directory
/// Returns a list of paths to the extracted files
pub async fn extract_zip_to_directory(zip_path: &Path, destination: &Path) -> Result<Vec<PathBuf>> {
    info!(archive = ?zip_path, destination = ?destination, "Extracting zip archive");
    let file = BufReader::new(File::open(zip_path).await?);
    let mut zip = ZipFileReader::with_tokio(file).await?;

    let mut extracted_files = Vec::new();
    let entry_count = zip.file().entries().len();

    info!(entries = entry_count, "Scanning zip contents");

    // Process all entries with progress logging
    for index in 0..entry_count {
        // Get entry information
        let path_str;
        let entry_is_dir;
        {
            let entry = zip.file().entries().get(index).unwrap();
            path_str = entry.filename().as_str()?.to_string();
            entry_is_dir = entry.dir()?;
        }
        let path = destination.join(&path_str);

        // Log progress for every file
        info!(
            entry = index + 1,
            total = entry_count,
            path = path_str,
            is_dir = entry_is_dir,
            "Extracting file"
        );

        // Handle directories
        if entry_is_dir {
            if !path.exists() {
                tokio::fs::create_dir_all(&path).await?;
            }
            continue;
        }

        // Handle files
        if let Some(parent) = path.parent() {
            if !parent.is_dir() {
                tokio::fs::create_dir_all(parent).await?;
            }
        }

        // Extract the file
        let entry_reader = zip.reader_with_entry(index).await?;
        let mut output_file = tokio::fs::File::create(&path).await?;
        let bytes_copied = tokio::io::copy(&mut entry_reader.compat(), &mut output_file).await?;

        debug!(
            bytes = bytes_copied,
            path = path_str,
            "File extracted successfully"
        );

        // Track extracted file
        extracted_files.push(path);
    }

    Ok(extracted_files)
}

/// Base trait for importers
pub trait Importer: Send + Sync + Clone + 'static {
    /// The type of request for this importer (must be deserializable from JSON)
    type ImportRequest: serde::de::DeserializeOwned + Send + Sync;

    /// Import using a request
    async fn import(&self, request: Self::ImportRequest) -> Result<ImportSource>;

    /// Return a string name/identifier for this importer
    fn name(&self) -> &'static str;

    /// Return a user-friendly display name for this importer
    fn display_name(&self) -> &'static str;

    /// Return a description of this importer
    fn description(&self) -> &'static str;
}
