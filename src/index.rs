//! Tinfoil "Index" JSON response data types.
//! Tinfoil expects to read a json "index", which essentially just acts as a response format
//! and lists all the files available for serving to the client.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Title {
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

/// A file entry in the tinfoil index.
///
/// Reference: https://blawar.github.io/tinfoil/custom_index/
#[derive(Serialize, Deserialize, Debug)]
pub struct FileEntry {
    /// Path or URL to the file.
    /// Can be a relative HTTP path or some kind of Tinfoil path spec.
    /// For example, `/games/MyGame.nsp` will point to the current server with the specified path.
    /// `http://example.com/games/MyGame.nsp` will point to the specified URL directly, etc.
    ///
    /// This path may also contain a `#`, which works similarly to RPM and pacman's `#` syntax,
    /// overriding the file name to be downloaded/saved as.
    /// For example, `/games/file/#MyGame.nsp` will download the file as `MyGame.nsp`.
    pub path: String,
    /// File size in bytes.
    pub size: u64,
}

/// Actions to be commited to the client's sources list.
///
#[derive(Serialize, Deserialize, Debug)]
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
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum SourceList {
    /// Simply adds a source to the client's sources list
    AddSource(String),
    /// Custom actions to be commited to the client's sources list.
    CustomAction(SourceAction),
}

// todo: something like this?
pub enum TinfoilResponse {
    Success(Index),
    Failure(String),
    ThemeError(String),
}

// {
//     "clientCertPub": "-----BEGIN PUBLIC KEY----- ....",
//     "clientCertKey": "-----BEGIN PRIVATE KEY----- ...."
// }
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClientCerts {
    pub client_cert_pub: String,
    pub client_cert_key: String,
}

/// A Tinfoil index, which is the primary response type for Tinfoil.
///
/// You shouldn't need to use this type directly, it's meant for serialization.
///
/// Consider writing wrapper types that become serialized as this JSON format instead.
/// See [`TinfoilResponse`] for an example.
#[derive(Serialize, Deserialize, Debug, Default)]
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
    pub files: Vec<FileEntry>,

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
    #[serde(flatten)]
    pub client_certs: Option<ClientCerts>,

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
    pub titledb: BTreeMap<String, Title>,

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
