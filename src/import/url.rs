use crate::import::{ImportSource, Importer, Result};

/// Request type for URL importer
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UrlImportRequest {
    pub url: String,
}

#[derive(Clone, Debug)]
pub struct UrlImporter;

impl UrlImporter {
    pub fn new() -> Self {
        Self
    }
}

impl Importer for UrlImporter {
    type ImportRequest = UrlImportRequest;

    async fn import(&self, request: Self::ImportRequest) -> Result<ImportSource> {
        // Use the RemoteHttpAuto type to automatically determine if it's an archive
        Ok(ImportSource::RemoteHttpAuto {
            url: request.url,
            headers: None,
        })
    }

    fn name(&self) -> &'static str {
        "url_importer"
    }

    fn display_name(&self) -> &'static str {
        "URL Importer"
    }

    fn description(&self) -> &'static str {
        "Imports games from URLs"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_url_importer_decode() {
        let importer = UrlImporter::new();

        // Test with a URL-encoded URL
        let encoded = "https%3A%2F%2Fexample.com%2Ftest.nsp";
        let result = importer
            .import(UrlImportRequest {
                url: encoded.to_string(),
            })
            .await
            .unwrap();

        match result {
            ImportSource::RemoteHttpAuto { url, headers } => {
                assert_eq!(url, "https://example.com/test.nsp");
                assert!(headers.is_none(), "Headers should be None");
            }
            _ => panic!("Expected RemoteHttpAuto import source"),
        }
    }
}
