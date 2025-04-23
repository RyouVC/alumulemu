//! This module handles DBI (DB Installer) package repository imports.
//!
//!
//! DBI is Russian package manager for the Nintendo Switch.
//!
//! It's repository format is simply a HTML scraper that parses a list of <a> tags, using the href attribute
//! to retrieve the download link.

use std::str::FromStr;

use bytesize::ByteSize;
use color_eyre::Result;
use http::{HeaderMap, HeaderValue, header::USER_AGENT};
use scraper::{Html, Selector};

use crate::index::TinfoilFileEntry;

use super::NxDevice;

pub struct DbiImporter {
    pub client: reqwest::Client,
    pub base_url: String,
    pub device: NxDevice,
}

pub struct DbiFile {
    pub name: String,
    pub url: String,
    // file size in approximate bytes
    pub size: Option<ByteSize>,
}

impl DbiFile {
    pub fn new(name: String, url: String, size: Option<ByteSize>) -> Self {
        Self { name, url, size }
    }

    // <a href="https://example.com/file.zip">file.zip</a>
    pub fn from_html_atag(tag: &scraper::ElementRef) -> Self {
        let url = tag.value().attr("href").unwrap_or("").to_string();
        let text = tag.text().collect::<Vec<_>>().join("");
        // size is after name, delimited by ; (e.g. file.zip; 10 GB)
        let size: Option<ByteSize> = text
            .split(';')
            .nth(1)
            .map(|s| s.chars().filter(|c| !c.is_whitespace()).collect::<String>())
            .and_then(|s| ByteSize::from_str(&s).ok());
        // Remove the size from the name
        let name = text.split(';').next().unwrap_or("").trim().to_string();
        Self::new(name, url, size)
    }
}

impl From<DbiFile> for TinfoilFileEntry {
    fn from(file: DbiFile) -> Self {
        let formatted_url = format!("{url}#{name}", url = file.url, name = file.name);
        Self {
            url: formatted_url,
            size: 0,
        }
    }
}

pub struct DbiDirectory {
    pub name: String,
    // list of <a> tags with href attributes
    pub files: Vec<DbiFile>,
    // A file called folder.jpg that is used as a folder icon
    pub banner: Option<DbiFile>,
}

impl DbiImporter {
    pub fn new(base_url: String, device: NxDevice) -> Self {
        let client = reqwest::Client::new();

        Self {
            client,
            base_url,
            device,
        }
    }

    pub fn parse_html(html: &str) -> Result<(Option<DbiFile>, Vec<DbiFile>)> {
        let document = Html::parse_document(&html);
        let body_selector = Selector::parse("body")
            .map_err(|_| color_eyre::eyre::eyre!("Failed to parse HTML selector"))?;
        let body_element = document.select(&body_selector).next().ok_or_else(|| {
            color_eyre::eyre::eyre!("Could not find body element in HTML document")
        })?;

        let a_selector = Selector::parse("a")
            .map_err(|_| color_eyre::eyre::eyre!("Failed to parse HTML selector"))?;

        let (files, banner) = body_element
            .select(&a_selector)
            .map(|element| DbiFile::from_html_atag(&element))
            .filter(|file| !file.name.is_empty() && !file.url.is_empty()) // Ignore empty links or names
            .fold((Vec::new(), None), |(mut acc_files, acc_banner), file| {
                if file.name == "folder.jpg" {
                    (acc_files, Some(file)) // Found the banner
                } else {
                    acc_files.push(file); // Add other files to the list
                    (acc_files, acc_banner)
                }
            });
        Ok((banner, files))
    }

    pub async fn get_files(&self) -> Result<DbiDirectory> {
        let user_agent = self.device.dbi_user_agent();
        let mut headers = HeaderMap::new();

        headers.insert(USER_AGENT, HeaderValue::from_str(&user_agent).unwrap());

        // get the last part of the base_url as the folder name
        let url = url::Url::parse(&self.base_url)?;

        let folder_name = url.path();

        let response = self
            .client
            .get(&self.base_url)
            .headers(headers)
            .send()
            .await?;
        if response.status() != 200 {
            return Err(color_eyre::eyre::eyre!(
                "Failed to fetch files from {}: {}: {}",
                self.base_url,
                response.status(),
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string())
            ));
        }

        let body = response.text().await?;

        let (banner, files) = Self::parse_html(&body)?;
        let directory = DbiDirectory {
            name: folder_name.to_string(),
            files,
            banner,
        };
        Ok(directory)
    }
}
