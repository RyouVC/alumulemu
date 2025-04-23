use std::collections::HashMap;

use crate::backend::kv_config::KvOptExt;
use rand::seq::IndexedRandom;
use reqwest::header::{COOKIE, HeaderMap, HeaderValue}; // Import the Rng trait

use super::{ImportError, ImportSource, Importer, NxDevice, Result};
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub struct UltraNxDownloadConfig {
    pub token: Option<String>,
    pub device: Option<NxDevice>,
}

impl UltraNxDownloadConfig {
    pub fn headers(&self) -> HeaderMap {
        let user_agents = [
            "Mozilla/5.0 (Windows NT 10.0; WOW64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/45.0.2454.85 Safari/537.36", // windows
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/42.0.2311.135 Safari/537.36 Edge/12.10240", // edge on windows
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_10_11) AppleWebKit/564.7 (KHTML, like Gecko) Chrome/105.0.0.0 Safari/564.7 Edg/105.0.4557.73", // edge on macos
            "Mozilla/5.0 (X11; Linux x86_64; rv:137.0) Gecko/20100101 Firefox/137.0", // firefox linux
        ];
        let mut headers = HeaderMap::new();

        // Add the User-Agent header
        // i think they're trying to block requests now. lmfao.
        // Use from_str instead of from_static as useragent is not static
        let random_user_agent = user_agents
            .choose(&mut rand::rng())
            .unwrap_or(&user_agents[0]);
        if let Ok(ua_header) = HeaderValue::from_str(random_user_agent) {
            headers.insert(reqwest::header::USER_AGENT, ua_header);
        }

        if let Some(token_value) = &self.token {
            if self.device.is_none() && !token_value.is_empty() {
                let cookie_value = format!("auth_token={}", token_value);
                if let Ok(header_val) = HeaderValue::from_str(&cookie_value) {
                    headers.insert(COOKIE, header_val);
                } else {
                    // Handle error: Invalid header value, maybe log it
                    eprintln!("Warning: Invalid characters in auth_token cookie value.");
                }
            }
        }

        headers
    }
}

impl KvOptExt for UltraNxDownloadConfig {
    const KEY_NAME: &'static str = "ultranx_config";
}

impl NotUltranxImporter {
    pub async fn new() -> Self {
        let config = UltraNxDownloadConfig::get()
            .await
            .unwrap_or_default()
            .unwrap_or_default();

        let headers = config.headers();

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new()); // Fallback to default client on build error

        Self { client }
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

    pub async fn get_dlc_links(&self, title_id: &str) -> Result<Option<Vec<String>>> {
        let url = format!("{}/game/{}", WEB_URL, title_id);
        let response = self.client.get(&url).send().await?;

        if response.status() == 404 {
            return Ok(None);
        }

        let body = response.text().await?;
        let document = Html::parse_document(&body);
        // Using a selector inspired by the user input, targeting links within #dlcsList
        // Assuming the user wants all links within #dlcsList, not just one specific link.
        let selector = Selector::parse("#dlcsList a").unwrap();
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
    AllSplit,
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
        // Clone and convert headers once
        // Manually create headers based on current config state
        let config = UltraNxDownloadConfig::get()
            .await
            .unwrap_or_default()
            .unwrap_or_default(); // Fetch config again

        let headers_map = config.headers();
        // If you needed to add other static headers, you could do it here:
        // headers_map.insert("X-Custom-Header".to_string(), "SomeValue".to_string());
        let headers_option = if headers_map.is_empty() {
            None
        } else {
            Some(headers_map)
        }
        .map(|h| {
            h.iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                .collect::<HashMap<String, String>>()
        });

        match request.download_type {
            NotUltranxDownloadType::Base => Ok(ImportSource::RemoteHttp {
                url: title.base_url,
                headers: headers_option,
            }),
            NotUltranxDownloadType::Update => {
                if let Some(url) = title.update_url {
                    Ok(ImportSource::RemoteHttp {
                        url,
                        headers: headers_option,
                    })
                } else {
                    Err(ImportError::Other(color_eyre::eyre::eyre!(
                        "Update not available for this title"
                    )))
                }
            }
            NotUltranxDownloadType::Dlcs => {
                if let Some(url) = title.dlcs_url {
                    // Assuming DLCs might be archives or multiple files handled by downloader
                    Ok(ImportSource::RemoteHttpArchive {
                        url,
                        headers: headers_option,
                    })
                } else {
                    Err(ImportError::Other(color_eyre::eyre::eyre!(
                        "DLCs not available for this title"
                    )))
                }
            }
            NotUltranxDownloadType::FullPkg => {
                if let Some(url) = title.full_pkg_url {
                    // Assuming FullPkg might be an archive or multiple files
                    Ok(ImportSource::RemoteHttpArchive {
                        url,
                        headers: headers_option,
                    })
                } else {
                    Err(ImportError::Other(color_eyre::eyre::eyre!(
                        "Full package not available for this title"
                    )))
                }
            }
            NotUltranxDownloadType::AllSplit => {
                let basegame_url = title.base_url;
                let update_url = title.update_url;
                let dlcs = self.get_dlc_links(&request.title_id).await?;
                let dlcs_url = dlcs.unwrap_or_default();

                let mut all_urls = vec![basegame_url];
                if let Some(update_url) = update_url {
                    all_urls.push(update_url);
                }
                all_urls.extend(dlcs_url);

                tracing::debug!("All URLs: {:?}", all_urls);
                // Assuming all URLs are valid and need to be downloaded
                Ok(ImportSource::RemoteHttpAutoList {
                    urls: all_urls,
                    headers: headers_option,
                })
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
