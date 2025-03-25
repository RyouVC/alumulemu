use std::default;

use super::{IdImporter, Importer, Result};
use scraper::{Html, Selector};

const WEB_URL: &str = "https://not.ultranx.ru/en";

pub struct NotUltranxImporter {
    client: reqwest::Client,
}
#[derive(Debug)]
pub struct NotUltranxTitle {
    pub title_id: String,
    pub base_url: String,
    pub update_url: Option<String>,
    pub dlcs_url: Option<String>,
    pub full_pkg_url: Option<String>,
}

impl NotUltranxImporter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    // find a div with the class "download-buttons, and find all the <a> tags within it
    pub async fn get_download_links(&self, title_id: &str) -> Result<Option<Vec<String>>> {
        let url = format!("{}/game/{}", WEB_URL, title_id);
        let response = self.client.get(&url).send().await?;

        if response.status() == 404 {
            return Ok(None);
        }

        let body = response.text().await?;
        let document = Html::parse_document(&body);
        let selector = Selector::parse(".download-buttons a").unwrap();
        let links: Vec<String> = document
            .select(&selector)
            .filter_map(|element| element.value().attr("href").map(|href| href.to_string()))
            .collect();

        Ok(Some(links))
    }

    pub async fn get_title(&self, title_id: &str) -> Result<Option<NotUltranxTitle>> {
        let links = self.get_download_links(title_id).await?;

        if let Some(links) = links {
            let base_url = links.iter().find(|link| link.ends_with("/base"));
            let update_url = links.iter().find(|link| link.ends_with("/update"));
            let dlcs_url = links.iter().find(|link| link.ends_with("/dlcs"));
            let full_pkg_url = links.iter().find(|link| link.ends_with("/full"));
            if base_url.is_none() {
                return Ok(None);
            }
            let title = NotUltranxTitle {
                title_id: title_id.to_string(),
                base_url: base_url.map(|url| url.to_string()).unwrap_or_default(),
                update_url: update_url.map(|url| url.to_string()),
                dlcs_url: dlcs_url.map(|url| url.to_string()),
                full_pkg_url: full_pkg_url.map(|url| url.to_string()),
            };

            return Ok(Some(title));
        }

        Ok(None)
    }
}
#[derive(Debug, Default)]
pub enum NotUltranxDownloadType {
    Base,
    Update,
    Dlcs,
    #[default]
    FullPkg,
}
#[derive(Debug, Default)]
pub struct UltraNxImportOptions {
    pub download_type: NotUltranxDownloadType,
}

impl Importer for NotUltranxImporter {
    type ImportOptions = UltraNxImportOptions;
}

impl IdImporter for NotUltranxImporter {
    type ImportData = NotUltranxTitle;

    async fn import_by_id(
        &self,
        id: &str,
        options: Option<Self::ImportOptions>,
    ) -> super::Result<super::ImportSource> {
        let options = options.unwrap_or_default();
        let title = self.get_title(id).await?;
        if title.is_none() {
            return Err(super::ImportError::Other(color_eyre::eyre::eyre!(
                "Title not found"
            )));
        }
        match options.download_type {
            NotUltranxDownloadType::Base => {
                let title = self.get_title(id).await?;
                if let Some(title) = title {
                    return Ok(super::ImportSource::RemoteHttp(title.base_url));
                }
            }
            NotUltranxDownloadType::Update => {
                if let Some(title) = title {
                    return Ok(super::ImportSource::RemoteHttp(
                        title.update_url.unwrap_or_default(),
                    ));
                }
            }
            NotUltranxDownloadType::Dlcs => {
                if let Some(title) = title {
                    return Ok(super::ImportSource::RemoteHttpArchive(
                        title.dlcs_url.unwrap_or_default(),
                    ));
                }
            }
            NotUltranxDownloadType::FullPkg => {
                if let Some(title) = title {
                    return Ok(super::ImportSource::RemoteHttpArchive(
                        title.full_pkg_url.unwrap_or_default(),
                    ));
                }
            }
        }

        // Ok(super::ImportSource::RemoteHttp(format!("{}/game/{}", WEB_URL, id)))
        // Download the title
        Err(super::ImportError::Other(color_eyre::eyre::eyre!(
            "Invalid download type"
        )))
    }

    async fn get_import_data(&self, id: &str) -> super::Result<Option<Self::ImportData>> {
        self.get_title(id).await
    }
}

#[cfg(test)]
mod tests {
    use crate::import::downloader::{Downloader, Progress};
    use std::time::Duration;
    use tokio::sync::mpsc;

    use super::*;
    use reqwest::cookie::Jar;
    use reqwest::header::{self, HeaderMap, HeaderValue};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_get_download_links() {
        let importer = NotUltranxImporter::new();
        let title = importer.get_title("010061F01DB7C000").await.unwrap();
        println!("{:?}", title);
    }

    #[tokio::test]
    async fn staging_oneio() {
        // Use the api.ultranx.ru domain with the proper download endpoint
        let url = "https://api.ultranx.ru/download/0100C51021970000/full";

        let downloader = Downloader::new();

        // Create a channel for progress updates
        let (tx, mut rx) = mpsc::channel::<Progress>(32);

        // Spawn a task to handle progress updates
        let progress_task = tokio::spawn(async move {
            let mut last_percentage = 0;
            while let Some(progress) = rx.recv().await {
                if let Some(total_size) = progress.total_size {
                    let percentage =
                        ((progress.downloaded as f64 / total_size as f64) * 100.0) as u32;

                    // Only print when percentage changes or download completes
                    if percentage > last_percentage || progress.complete {
                        println!(
                            "Download progress: {}% ({}/{} bytes)",
                            percentage, progress.downloaded, total_size
                        );
                        last_percentage = percentage;
                    }
                } else {
                    // If total size is unknown, just show bytes downloaded
                    println!("Downloaded: {} bytes", progress.downloaded);
                }

                if progress.complete {
                    println!("Download complete!");
                    break;
                }
            }
        });

        // Start the download with progress tracking
        let file = downloader
            .download_file_with_progress(url, "test.zip", tx)
            .await
            .unwrap();

        // Wait for the progress task to complete
        let _ = tokio::time::timeout(Duration::from_secs(5), progress_task).await;

        println!("Download saved to: {:?}", file);

        let metadata = std::fs::metadata(file).unwrap();
        assert!(metadata.is_file());
        assert!(metadata.len() > 0);
    }
}
