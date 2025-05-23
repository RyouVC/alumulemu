//! Tinfoil "Index" JSON response data types.
//! Tinfoil expects to read a json "index", which essentially just acts as a response format
//! and lists all the files available for serving to the client.

use axum::{
    Json,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const EXTRA_INDEXES_LIST_TABLE: &str = "extra_indexes_list";

/// Additional indexes list to sync
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ExtraIndexesImport {
    /// URL to download the index from
    pub url: String,
}

impl ExtraIndexesImport {
    pub fn new(url: String) -> Self {
        ExtraIndexesImport { url }
    }

    pub async fn list() -> color_eyre::Result<Vec<Self>> {
        let db: Vec<Self> = DB
            .select(EXTRA_INDEXES_LIST_TABLE)
            .await?
            .into_iter()
            .collect();
        Ok(db)
    }

    pub async fn add(&self) -> color_eyre::Result<()> {
        let db: Option<Self> = DB
            .upsert((EXTRA_INDEXES_LIST_TABLE, &self.url))
            .content(self.clone())
            .await?;
        if db.is_none() {
            tracing::error!("Failed to save extra index to database");
            return Err(color_eyre::Report::msg(
                "Failed to save extra index to database",
            ));
        }
        tracing::info!("Saved extra index to database");
        Ok(())
    }
    /// Deletes the extra index from the database.
    ///
    /// This does not actually delete the imported data itself, only the task to import it.
    pub async fn delete(&self) -> color_eyre::Result<()> {
        let db: Option<Self> = DB.delete((EXTRA_INDEXES_LIST_TABLE, &self.url)).await?;
        if db.is_none() {
            tracing::error!("Failed to delete extra index from database");
            return Err(color_eyre::Report::msg(
                "Failed to delete extra index from database",
            ));
        }
        tracing::info!("Deleted extra index from database");
        Ok(())
    }
}

use crate::db::DB;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TinfoilTitleMeta {
    #[serde(rename = "id")]
    pub title_id: String,
    pub name: String,
    pub version: u32,
    pub region: String,
    /// Release date in the format of `YYYYMMDD`.
    // todo: port to chrono::NaiveDate
    pub release_date: String,
    pub rating: u8,
    pub publisher: String,
    pub description: String,
    pub size: u64,
    pub rank: u32,
}

impl TryFrom<crate::titledb::Title> for TinfoilTitleMeta {
    type Error = crate::router::Error;

    fn try_from(title: crate::titledb::Title) -> Result<Self, Self::Error> {
        Ok(TinfoilTitleMeta {
            title_id: title.title_id.unwrap_or_default(),
            name: title.name.unwrap_or_default(),
            version: title.version.unwrap_or_default().parse().unwrap(),
            region: title.region.unwrap_or_default(),
            release_date: title.release_date.unwrap_or_default(),
            rating: title.rating.unwrap_or_default() as u8,
            publisher: title.publisher.unwrap_or_default(),
            description: title.description.unwrap_or_default(),
            size: title.size.unwrap_or_default(),
            rank: 0,
        })
    }
}

/// A file entry in the tinfoil index.
///
/// Reference: https://blawar.github.io/tinfoil/custom_index/
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TinfoilFileEntry {
    /// Path or URL to the file.
    /// Can be a relative HTTP path or some kind of Tinfoil path spec.
    /// For example, `/games/MyGame.nsp` will point to the current server with the specified path.
    /// `http://example.com/games/MyGame.nsp` will point to the specified URL directly, etc.
    ///
    /// This path may also contain a `#`, which works similarly to RPM and pacman's `#` syntax,
    /// overriding the file name to be downloaded/saved as.
    /// For example, `/games/file/#MyGame.nsp` will download the file as `MyGame.nsp`.
    pub url: String,
    /// File size in bytes.
    pub size: u64,
}

/// Actions to be commited to the client's sources list.
///
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceAction {
    pub url: Option<String>,
    pub title: Option<String>,
    pub action: Option<String>,
}

/// Source list actions to be commited to the client's sources list.
///
/// Tinfoil can be instructed to make changes to its source list using a JSON response by adding this field to the response.
///
/// May be useful if you want to bundle fallback repositories or additional sources for the client to use.
///
/// You should use this sparingly, as this arbitarily modifies the client's sources list, which may be ***dangerous***.
///
/// ```json
/// {
///   "locations": [
///     "https://abc123.com/456/",
///     {"url": "https://xyz.com/blah", "title": "xyz", "action": "disable"},
///     {"url": "https://xyz.com/blah2", "title": "xyz2", "action": "enable"},
///     {"url": "https://xyz.com/blah3", "title": "xyz3", "action": "add"}
///   ]
/// }
/// ```
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum SourceList {
    /// Simply adds a source to the client's sources list
    AddSource(String),
    /// Custom actions to be commited to the client's sources list.
    CustomAction(SourceAction),
}
#[derive(Debug, thiserror::Error, Serialize, Deserialize, Clone)]
pub enum TinfoilError {
    // #[error("{failure:?}")]
    #[error("Failure: {0}")]
    Failure(String),
}

// todo: something like this?
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum TinfoilResponse {
    Success(Index),
    MiscSuccess(String),
    Failure(String),
    ThemeError(String),
}

impl IntoResponse for TinfoilResponse {
    fn into_response(self) -> Response {
        match self {
            TinfoilResponse::Success(index) => index.into_response(),
            TinfoilResponse::Failure(failure) => {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(failure)).into_response()
            }
            TinfoilResponse::ThemeError(theme_error) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(theme_error),
            )
                .into_response(),
            TinfoilResponse::MiscSuccess(misc_success) => {
                (axum::http::StatusCode::OK, Json(misc_success)).into_response()
            }
        }
    }
}

impl From<Result<Index, String>> for TinfoilResponse {
    fn from(result: Result<Index, String>) -> Self {
        match result {
            Ok(index) => index.into(),
            Err(error) => TinfoilResponse::Failure(error),
        }
    }
}

impl From<Index> for TinfoilResponse {
    fn from(index: Index) -> Self {
        if let Some(failure) = index.failure {
            TinfoilResponse::Failure(failure)
        } else if let Some(theme_error) = index.theme_error {
            TinfoilResponse::ThemeError(theme_error)
        } else if let Some(success) = index.success {
            TinfoilResponse::MiscSuccess(success)
        } else {
            TinfoilResponse::Success(index)
        }
    }
}

impl From<TinfoilResponse> for Result<Index, String> {
    fn from(response: TinfoilResponse) -> Self {
        match response {
            TinfoilResponse::Success(index) => Ok(index),
            TinfoilResponse::Failure(error) => Err(error),
            TinfoilResponse::ThemeError(error) => Err(error),
            TinfoilResponse::MiscSuccess(success) => Ok(Index {
                success: Some(success),
                ..Default::default()
            }),
        }
    }
}

// {
//     "clientCertPub": "-----BEGIN PUBLIC KEY----- ....",
//     "clientCertKey": "-----BEGIN PRIVATE KEY----- ...."
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ClientCerts {
    pub client_cert_pub: String,
    pub client_cert_key: String,
}

pub const EXTRA_INDEXES_TABLE: &str = "extra_indexes";

/// A Tinfoil index, which is the primary response type for Tinfoil.
///
/// You shouldn't need to use this type directly, it's meant for serialization.
///
/// Consider writing wrapper types that become serialized as this JSON format instead.
/// See [`TinfoilResponse`] for an example.
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(default)]
pub struct Index {
    /// Message to display to the user on connection success.
    /// Can also be used as an MOTD (Message of the Day) for clients.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success: Option<String>,
    /// Message to display to the user on connection failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,

    /// File to be served to the client.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<TinfoilFileEntry>,

    /// Sub-directories to list to the client, if any.
    ///
    /// Else the client will list all files displayed in one big list.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub directories: Vec<String>,

    /// Optional referrer URL to prevent hotlinking by clients, optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,

    /// Optional Google API key if hosting files on Google Drive, optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "googleApiKey")]
    pub google_api_key: Option<String>,

    /// Optional 1Fichier API key if hosting files on 1Fichier, optional.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "oneFichierKeys")]
    pub fichier_keys: Vec<String>,

    /// custom HTTP headers to be sent with Tinfoil requests
    ///
    /// Should be in the format of `Header: Value`.
    ///
    /// ```json
    /// {
    ///   "headers": ["My-Custom_header: hello", "My-Custom_header2: world"]
    /// }
    /// ```
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,

    /// Minimum Tinfoil client version required to connect.
    /// If the client version is lower than this, the client will refuse to connect.
    /// This is useful for enforcing updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// Optional client certificate and key for mutual TLS authentication.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "clientCertPub")]
    pub client_cert_pub: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "clientCertKey")]
    pub client_cert_key: Option<String>,

    /// Source list actions to be commited to the client's sources list.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<SourceList>,

    /// Additional metadata for titles to be sent to the client, optional.
    ///
    /// This is very useful for providing custom metadata for homebrew titles that can't be
    /// found in Tinfoil's upstream databases.
    // personal note: I wish there was a way to make Tinfoil itself not fetch the upstream database, but
    // we can only wish.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub titledb: BTreeMap<String, TinfoilTitleMeta>,

    /// Theme blacklists to be sent to the client, optional.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "themeBlacklist")]
    pub theme_blacklist: Vec<String>,

    /// Theme whitelists to be sent to the client, optional.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(rename = "themeWhitelist")]
    pub theme_whitelist: Vec<String>,

    /// Theme error message to be sent to the client, optional.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "themeError")]
    pub theme_error: Option<String>,
}

impl Index {
    pub fn add_file(
        &mut self,
        path: &std::path::Path,
        prefix: &str,
        suffix: &str,
        title_id: Option<&str>,
    ) {
        let metadata = std::fs::metadata(path).unwrap();
        let size = metadata.len();
        let prefix = prefix.strip_suffix("/").unwrap_or(prefix);

        let url = if let Some(tid) = title_id {
            format!("{prefix}/{tid}#{suffix}")
        } else {
            format!(
                "{prefix}/{}#{suffix}",
                path.file_name().unwrap().to_string_lossy()
            )
        };

        let file = TinfoilFileEntry { url, size };

        self.files.push(file);
    }

    /// Naively merges the file index from another index.
    ///
    /// Useful for aggregrating multiple indexes into one.
    /// This is a naive merge, and will not check for duplicates.
    pub fn merge_file_index(&mut self, other: Index) {
        self.files.extend(other.files);
    }

    pub fn merge_titledb(&mut self, other: Index) {
        self.titledb.extend(other.titledb);
    }

    /// Naively adds a file to the index.
    pub fn naive_add_file(&mut self, url: &str, size: u64) {
        let file_link = TinfoilFileEntry {
            url: url.to_string(),
            size,
        };
        self.files.push(file_link);
    }

    pub async fn load_index_url(url: &str) -> color_eyre::Result<Self> {
        let response = reqwest::get(url).await?;
        let index: Index = response.json().await?;
        Ok(index)
    }

    /// Saves the extra index to the database table.
    pub async fn save_extra_index(self, src_name: &str) -> color_eyre::Result<()> {
        let db: Option<Self> = DB
            .upsert((EXTRA_INDEXES_TABLE, src_name))
            .content(self)
            .await?;
        if db.is_none() {
            tracing::error!("Failed to save extra index to database");
            return Err(color_eyre::Report::msg(
                "Failed to save extra index to database",
            ));
        }
        tracing::info!("Saved extra index to database");
        Ok(())
    }

    pub async fn get_extra_indexes() -> color_eyre::Result<Vec<Index>> {
        let db: Vec<Self> = DB.select(EXTRA_INDEXES_TABLE).await?.into_iter().collect();
        Ok(db)
    }

    /// Add a custom metadata entry for a title.
    pub fn add_title_metadata(&mut self, title: TinfoilTitleMeta) {
        self.titledb.insert(title.title_id.clone(), title);
    }
}

impl IntoResponse for Index {
    fn into_response(self) -> axum::response::Response {
        let json = serde_json::to_string(&self).unwrap();
        (axum::http::StatusCode::OK, json).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success_from_index() {
        let index = Index {
            success: Some("Connection successful!".to_string()),
            ..Default::default()
        };
        let response: TinfoilResponse = index.into();
        assert!(matches!(response, TinfoilResponse::Success(_)));
    }

    #[test]
    fn test_parse_success_json() {
        let raw_json = r#"{
            "success": "Connection successful!"
        }"#;
        let index: Index = serde_json::from_str(raw_json).unwrap();
        let response: TinfoilResponse = index.into();
        assert!(matches!(response, TinfoilResponse::Success(_)));
    }

    #[test]
    fn test_parse_empty_json() {
        let raw_json = r#"{}"#;
        let index: Index = serde_json::from_str(raw_json).unwrap();
        let response: TinfoilResponse = index.into();
        assert!(matches!(response, TinfoilResponse::Success(_)));
    }

    #[test]
    fn test_failure_case() {
        let index = Index {
            failure: Some("Error message".to_string()),
            ..Default::default()
        };
        let response: TinfoilResponse = index.into();
        assert!(matches!(response, TinfoilResponse::Failure(_)));
    }

    #[test]
    fn test_theme_error_case() {
        let index = Index {
            theme_error: Some("Theme error".to_string()),
            ..Default::default()
        };
        let response: TinfoilResponse = index.into();
        assert!(matches!(response, TinfoilResponse::ThemeError(_)));
    }
}
