//! Primary module for importers.
//!
//! Importers in this case are responsible for importing packages from various sources
//! into alumulemu. This can be done in a variety of ways, such as:
//!
//! - Manually uploading a package file
//! - Downloading a package from a remote source
//! - Merging to a repo with an existing repository of packages
//!

use async_zip::tokio::read::seek::ZipFileReader;
use downloader::{DOWNLOAD_QUEUE, DownloadQueueItem};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::{fs::File, io::BufReader};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use tracing::{debug, info};
pub mod downloader;
pub mod import_utils;
pub mod not_ultranx;
pub mod registry;
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
    RemoteHttp(String),
    /// A remote archive file accessed via HTTP that will be extracted
    RemoteHttpArchive(String),
    /// (Not implemented) Import from a repository
    Repository,
}

impl ImportSource {
    /// Creates a new Local import source
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
    pub fn new_remote_http(url: impl Into<String>) -> Self {
        Self::RemoteHttp(url.into())
    }

    /// Creates a new RemoteHttpArchive import source
    pub fn new_remote_http_archive(url: impl Into<String>) -> Self {
        Self::RemoteHttpArchive(url.into())
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
            ImportSource::RemoteHttp(url) => {
                let path = Self::download_http(url).await?;
                Ok((vec![path], None))
            }
            ImportSource::RemoteHttpArchive(url) => {
                let path = Self::download_http(url).await?;
                let result = self.extract_archive(&path).await?;
                // Delete the downloaded archive after successful extraction
                if tokio::fs::remove_file(&path).await.is_err() {
                    debug!(archive = ?path, "Failed to remove downloaded archive file after extraction");
                } else {
                    info!(archive = ?path, "Removed downloaded archive file after successful extraction");
                }
                Ok(result)
            }
            ImportSource::Remote => unimplemented!(
                "Generic Remote import source not implemented, this should be a generic remote import, but the details are not yet defined"
            ),
            ImportSource::Repository => unimplemented!(
                "Repository import source not implemented, This should take in a Tinfoil index"
            ),
        }
    }

    pub async fn download_http(url: &str) -> Result<PathBuf> {
        let download_path = download_path();
        let queue_item = DownloadQueueItem::new(url, download_path);

        // Create a scope to ensure the lock is dropped after getting the handle
        let mut handle = {
            // Lock is acquired here
            let mut queue = DOWNLOAD_QUEUE.lock()?;

            // Lock is automatically dropped here when queue goes out of scope
            queue.add(queue_item)
        };

        // let mut handle = dl_queue.add(queue_item);
        // drop(dl_queue);

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
        let temp_dir = crate::util::tempdir();
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
pub trait Importer {
    /// The type of options/configuration for this importer
    type ImportOptions;
}

/// Trait for importers that can import from files
pub trait FileImporter: Importer {
    /// Import data from a specific source (like a file or URL)
    async fn import_from_source(
        &self,
        source: &Path,
        options: Option<Self::ImportOptions>,
    ) -> Result<ImportSource>;
}

/// Trait for importers that can import using an identifier
pub trait IdImporter: Importer {
    /// The type of data this importer can handle
    type ImportData;

    /// Import data using an identifier (like title_id)
    async fn import_by_id(
        &self,
        id: &str,
        options: Option<Self::ImportOptions>,
    ) -> Result<ImportSource>;

    /// Get metadata about what can be imported
    async fn get_import_data(&self, id: &str) -> Result<Option<Self::ImportData>>;
}
