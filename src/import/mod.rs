//! Primary module for importers.
//!
//! Importers in this case are responsible for importing packages from various sources
//! into alumulemu. This can be done in a variety of ways, such as:
//!
//! - Manually uploading a package file
//! - Downloading a package from a remote source
//! - Merging to a repo with an existing repository of packages
//!

use std::path::{Path, PathBuf};
use thiserror::Error;
pub mod not_ultranx;
pub mod downloader;
#[derive(Error, Debug)]
pub enum ImportError {
    // IO errors
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    // Serde errors
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),

    // Other errors
    #[error("{0:?}")]
    Other(#[from] color_eyre::eyre::Report),
}

pub enum ImportSource {
    Local(PathBuf),
    /// A local archive file
    LocalArchive(PathBuf),
    LocalDir(PathBuf),
    Remote,
    RemoteHttp(String),
    RemoteHttpArchive(String),
    Repository,
}

pub fn process_import_source(source: ImportSource) -> Result<Vec<PathBuf>> {
    match source {
        ImportSource::Local(path) => Ok(vec![path]),
        ImportSource::LocalArchive(path) => {
            todo!()
        },
        ImportSource::LocalDir(path) => {
            let walker = jwalk::WalkDir::new(path);
            let files: Vec<PathBuf> = walker
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_file())
                .map(|entry| entry.path().to_path_buf())
                .collect();
            Ok(files)
        }
        _ => todo!(),
    }
}

pub type Result<T> = std::result::Result<T, ImportError>;

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
