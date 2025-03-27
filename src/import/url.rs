use crate::import::{IdImporter, ImportError, ImportSource, Importer, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};
use url::Url;

/// A simple importer that takes a URL-encoded URL and imports the file it points to
#[derive(Clone, Debug)]
pub struct UrlImporter;

/// Metadata about a URL import
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlImportData {
    /// The decoded URL
    pub url: String,
    /// The filename extracted from the URL
    pub filename: String,
    /// The estimated file size in bytes (if available)
    pub size: Option<u64>,
    /// The content type (if available)
    pub content_type: Option<String>,
}

/// Options for URL imports
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UrlImportOptions {
    /// Custom filename to use instead of the one in the URL
    pub custom_filename: Option<String>,
}

impl Importer for UrlImporter {
    type ImportOptions = UrlImportOptions;
}

impl IdImporter for UrlImporter {
    type ImportData = UrlImportData;

    async fn import_by_id(
        &self,
        encoded_url: &str,
        options: Option<Self::ImportOptions>,
    ) -> Result<ImportSource> {
        // Decode the URL-encoded URL
        let decoded_url = match urlencoding::decode(encoded_url) {
            Ok(url) => url.into_owned(),
            Err(e) => {
                return Err(ImportError::Other(color_eyre::eyre::eyre!(
                    "Failed to decode URL: {}",
                    e
                )));
            }
        };

        debug!(url = decoded_url, "Decoded URL for import");

        // Parse the URL to validate it
        let parsed_url = match Url::parse(&decoded_url) {
            Ok(url) => url,
            Err(e) => {
                return Err(ImportError::Other(color_eyre::eyre::eyre!(
                    "Invalid URL: {}",
                    e
                )));
            }
        };

        // Check if the URL uses supported protocols
        if parsed_url.scheme() != "http" && parsed_url.scheme() != "https" {
            return Err(ImportError::Other(color_eyre::eyre::eyre!(
                "Unsupported URL scheme: {}. Only http and https are supported.",
                parsed_url.scheme()
            )));
        }

        // Create a RemoteHttpAuto import source that will automatically determine
        // whether to extract the file based on its extension after download
        info!(url = decoded_url, "Creating RemoteHttpAuto import source");
        Ok(ImportSource::RemoteHttpAuto(decoded_url))
    }

    async fn get_import_data(&self, encoded_url: &str) -> Result<Option<Self::ImportData>> {
        // Decode the URL-encoded URL
        let decoded_url = match urlencoding::decode(encoded_url) {
            Ok(url) => url.into_owned(),
            Err(e) => {
                return Err(ImportError::Other(color_eyre::eyre::eyre!(
                    "Failed to decode URL: {}",
                    e
                )));
            }
        };

        // Parse the URL to validate it
        let parsed_url = match Url::parse(&decoded_url) {
            Ok(url) => url,
            Err(e) => {
                return Err(ImportError::Other(color_eyre::eyre::eyre!(
                    "Invalid URL: {}",
                    e
                )));
            }
        };

        // Extract the filename from the URL path
        let filename = parsed_url
            .path_segments()
            .and_then(|segments| segments.last())
            .unwrap_or("unknown.file")
            .to_string();

        // For metadata, we can try to make a HEAD request to get information about the file
        let client = reqwest::Client::new();
        match client.head(&decoded_url).send().await {
            Ok(response) => {
                let status = response.status();

                if !status.is_success() {
                    debug!(
                        url = decoded_url,
                        status = %status,
                        "URL not accessible"
                    );
                    return Ok(None);
                }

                // Extract content type and size if available
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(String::from);

                let size = response
                    .headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());

                // Try to get filename from Content-Disposition header if available
                let filename = response
                    .headers()
                    .get(reqwest::header::CONTENT_DISPOSITION)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|cd| {
                        // Simple parser for Content-Disposition: attachment; filename="file.ext"
                        cd.split(';').find_map(|part| {
                            let part = part.trim();
                            if part.starts_with("filename=") {
                                let filename = part["filename=".len()..].trim();
                                // Remove quotes if present
                                if (filename.starts_with('"') && filename.ends_with('"'))
                                    || (filename.starts_with('\'') && filename.ends_with('\''))
                                {
                                    Some(filename[1..filename.len() - 1].to_string())
                                } else {
                                    Some(filename.to_string())
                                }
                            } else {
                                None
                            }
                        })
                    })
                    .unwrap_or(filename);

                Ok(Some(UrlImportData {
                    url: decoded_url,
                    filename,
                    size,
                    content_type,
                }))
            }
            Err(e) => {
                debug!(
                    url = decoded_url,
                    error = %e,
                    "Failed to fetch URL metadata"
                );

                // Return basic information without the extra metadata
                Ok(Some(UrlImportData {
                    url: decoded_url,
                    filename,
                    size: None,
                    content_type: None,
                }))
            }
        }
    }
}

impl UrlImporter {
    /// Create a new URL importer
    pub fn new() -> Self {
        UrlImporter
    }

    /// Convenience method to create the URL importer
    pub fn create() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::ImportSource;

    #[tokio::test]
    async fn test_url_importer_decode() {
        let importer = UrlImporter::new();

        // Test with a URL-encoded URL
        let encoded = "https%3A%2F%2Fexample.com%2Ftest.nsp";
        let result = importer.import_by_id(encoded, None).await.unwrap();

        match result {
            ImportSource::RemoteHttpAuto(url) => {
                assert_eq!(url, "https://example.com/test.nsp");
            }
            _ => panic!("Expected RemoteHttpAuto import source"),
        }
    }
}
