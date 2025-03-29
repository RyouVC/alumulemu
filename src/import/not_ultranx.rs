use super::{ImportError, ImportSource, Importer, Result};
use scraper::{Html, Selector};

const WEB_URL: &str = "https://not.ultranx.ru/en";

/// A JSON import request from the UltraNX archive
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UltraNxImportRequest {
    #[serde(default)]
    pub download_type: NotUltranxDownloadType,
    pub title_id: String,
}

#[derive(Clone)]
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

#[derive(Debug, Default, serde::Deserialize, serde::Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum NotUltranxDownloadType {
    Base,
    Update,
    Dlcs,
    #[default]
    FullPkg,
}

impl Importer for NotUltranxImporter {
    type ImportRequest = UltraNxImportRequest;

    async fn import(&self, request: Self::ImportRequest) -> Result<ImportSource> {
        let title = self.get_title(&request.title_id).await?;

        if title.is_none() {
            return Err(ImportError::GameNotFound);
        }

        let title = title.unwrap();

        // Determine the URL based on download type
        match request.download_type {
            NotUltranxDownloadType::Base => Ok(ImportSource::RemoteHttp(title.base_url)),
            NotUltranxDownloadType::Update => {
                if let Some(url) = title.update_url {
                    Ok(ImportSource::RemoteHttp(url))
                } else {
                    Err(ImportError::Other(color_eyre::eyre::eyre!(
                        "Update not available for this title"
                    )))
                }
            }
            NotUltranxDownloadType::Dlcs => {
                if let Some(url) = title.dlcs_url {
                    Ok(ImportSource::RemoteHttpArchive(url))
                } else {
                    Err(ImportError::Other(color_eyre::eyre::eyre!(
                        "DLCs not available for this title"
                    )))
                }
            }
            NotUltranxDownloadType::FullPkg => {
                if let Some(url) = title.full_pkg_url {
                    Ok(ImportSource::RemoteHttpArchive(url))
                } else {
                    Err(ImportError::Other(color_eyre::eyre::eyre!(
                        "Full package not available for this title"
                    )))
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "not_ultranx_importer"
    }

    fn display_name(&self) -> &'static str {
        "UltraNX Importer"
    }

    fn description(&self) -> &'static str {
        "Imports games from the not.ultranx.ru game archive"
    }
}
