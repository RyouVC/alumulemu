use crate::LOCALE;
use crate::db::{DB, NspMetadata, create_precomputed_metaview};
use crate::router::SearchQuery;
use crate::util::format_download_id;
use color_eyre::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use struson::reader::{JsonReader, JsonStreamReader};
use surrealdb::sql::Thing;
/// Represents a naive game data type, parsed with regex
///
/// Example: `Video Game [TITLEID][v0][US].nsp`
/// All fields between the file name and the extension (in square brackets) are optional.
///
///
/// if tag contains exactly 16 characters it's a titleid, if it starts with a `v` (lowercase v) it's a version
/// assume last tag is region
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct GameFileDataNaive {
    pub name: String,
    pub title_id: Option<String>,
    pub version: Option<String>,
    pub region: Option<String>,
    pub other_tags: Vec<String>,
    pub extension: Option<String>,
}

impl GameFileDataNaive {
    pub fn parse_from_filename(filename: &str) -> Self {
        // the name is regex stripped by the tags and extension
        let regex = Regex::new(r"\[(.*?)\]").unwrap();
        // First, extract the extension
        let extension = filename.split('.').last().map(|s| s.to_string());

        // Extract all tags from the filename
        let mut tags: Vec<String> = regex
            .captures_iter(filename)
            .map(|cap| cap[1].to_string())
            .collect();

        // Find the title_id (exactly 16 characters)
        let title_id = tags.iter().find(|tag| tag.len() == 16).cloned();

        // Remove title_id from tags if found
        if let Some(tid) = &title_id {
            if let Some(pos) = tags.iter().position(|t| t == tid) {
                tags.remove(pos);
            }
        }

        // Find version (starts with 'v')
        let version = tags.iter().find(|tag| tag.starts_with("v")).cloned();

        // Remove version from tags if found
        if let Some(ver) = &version {
            if let Some(pos) = tags.iter().position(|t| t == ver) {
                tags.remove(pos);
            }
        }

        let other_tags = tags;

        // Assume the last remaining tag is the region, if any
        let region = None;

        // Get the base name without tags
        let name = regex.replace_all(filename, "").trim().to_string();

        Self {
            name,
            title_id,
            version,
            region,
            extension,
            other_tags,
        }
    }

    const VALID_EXTENSIONS: [&str; 4] = ["nsp", "nsz", "xci", "xcz"];
    /// Try to get the cached naive metadata for a file
    pub async fn get_cached(path: &Path, all_metadata: &[NspMetadata]) -> Result<Self> {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let extension = path.extension().unwrap_or_default().to_str().unwrap();

        if Self::VALID_EXTENSIONS.contains(&extension) {
            //let nsp_data = NspData::read_file(path).unwrap();
            //let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());
            if let Some(existing_metadata) = all_metadata
                .iter()
                .find(|m| m.path == path.to_str().unwrap())
            {
                tracing::debug!("Found cached metadata for {}", path.display());
                let mut naive = Self::parse_from_filename(filename);
                naive.title_id = Some(existing_metadata.title_id.clone());
                naive.version = Some(existing_metadata.version.clone());
                return Ok(naive);
            } else {
                tracing::debug!("Reading NSP/NSZ/XCI file: {:?}", filename);
                let cnmt = crate::nsp::read_cnmt_merged(path.to_str().unwrap())?;
                let extension = path.extension().unwrap().to_str().unwrap();
                let title_id = cnmt.get_title_id_string();
                let version = cnmt.header.title_version.to_string();
                // let cnmt_output = run_nstool(path.to_str().unwrap());
                // let cnmt = parse_cnmt_output(&cnmt_output);
                // let (title_id, version) = get_title_id_and_version(cnmt);
                tracing::debug!("Title ID: {:?}", title_id);
                tracing::debug!("Version: {:?}", version);

                let metadata = NspMetadata {
                    path: path.to_str().unwrap().to_string(),
                    title_id: title_id.clone(),
                    version: version.clone(),
                    title_name: None,
                    download_id: format_download_id(&title_id, &version, extension),
                };
                if let Err(e) = metadata.save().await {
                    tracing::warn!("Failed to save metadata: {}", e);
                }

                // Only query title DB for new files
                let title_query_start = std::time::Instant::now();
                let config = crate::config::config();
                let locale = config.backend_config.get_locale_string();
                let title = Title::get_from_title_id(&locale, &title_id).await?;
                tracing::debug!("Title query took {:?}", title_query_start.elapsed());

                // If we got a title, return it
                if let Some(title) = title {
                    let title_name = title.name.clone();
                    let metadata = NspMetadata {
                        path: path.to_str().unwrap().to_string(),
                        title_id: title_id.clone(),
                        version: version.clone(),
                        title_name: title_name.clone(),
                        download_id: format_download_id(&title_id, &version, extension),
                    };
                    if let Err(e) = metadata.save().await {
                        tracing::warn!("Failed to save metadata with title name: {}", e);
                    }
                    return Ok(Self {
                        name: title_name.unwrap_or_default(),
                        title_id: Some(title_id.to_string()),
                        version: Some(version.clone()), // Use the NSP file's version instead of title.version
                        region: title.region,
                        other_tags: Vec::new(),
                        extension: Some(extension.to_string()),
                    });
                // else we got a title ID but no title, we can still return the title ID
                } else {
                    let mut naive = Self::parse_from_filename(filename);
                    naive.title_id = Some(title_id.to_string());
                    return Ok(naive);
                }
            }
        }

        Ok(Self::parse_from_filename(filename))
    }

    /// Try to get the naive metadata for a file without using the cache
    pub async fn get(path: &Path) -> Result<Self> {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let extension = path.extension().unwrap_or_default().to_str().unwrap();

        if Self::VALID_EXTENSIONS.contains(&extension) {
            tracing::debug!("Reading NSP/NSZ/XCI file: {:?}", filename);
            let cnmt = crate::nsp::read_cnmt_merged(path.to_str().unwrap())?;
            let extension = path.extension().unwrap().to_str().unwrap();
            let title_id = cnmt.get_title_id_string();
            let version = cnmt.header.title_version.to_string();
            tracing::debug!("Title ID: {:?}", title_id);
            tracing::debug!("Version: {:?}", version);

            // Only query title DB for new files
            let title_query_start = std::time::Instant::now();
            let config = crate::config::config();
            let locale = config.backend_config.get_locale_string();
            let title = Title::get_from_title_id(&locale, &title_id).await?;
            tracing::debug!("Title query took {:?}", title_query_start.elapsed());

            // If we got a title, return it
            if let Some(title) = title {
                let title_name = title.name.clone();
                return Ok(Self {
                    name: title_name.unwrap_or_default(),
                    title_id: Some(title_id.to_string()),
                    version: Some(version.clone()), // Use the NSP file's version instead of title.version
                    region: title.region,
                    other_tags: Vec::new(),
                    extension: Some(extension.to_string()),
                });
            // else we got a title ID but no title, we can still return the title ID
            } else {
                let mut naive = Self::parse_from_filename(filename);
                naive.title_id = Some(title_id.to_string());
                naive.version = Some(version);
                return Ok(naive);
            }
        }

        Ok(Self::parse_from_filename(filename))
    }
}

use serde::Deserializer;

fn deser_to_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    // use serde::de::Error;

    // Create a temporary enum to hold the possible types
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrU64 {
        String(String),
        U64(u64),
    }

    // Deserialize to Option<StringOrU64>
    let opt = Option::<StringOrU64>::deserialize(deserializer)?;

    // Convert the result to Option<String>
    Ok(opt.map(|val| match val {
        StringOrU64::String(s) => s,
        StringOrU64::U64(n) => n.to_string(),
    }))
}

// HACK: Some really cursed metaview fuckery to get metaviews working
// todo: maybe get surrealdb to support this natively
/// An entry for the metaview cache, created by the schema using a precomputed view
/// comparing the title ID of the NspMetadata with the title ID of titledb
#[derive(Debug, Deserialize, Serialize)]
pub struct Metaview {
    pub title: Option<Title>,
    pub path: String,
    pub title_id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub download_id: Option<String>,
}

pub fn default_locale() -> String {
    LOCALE.to_string()
}

impl Metaview {
    pub async fn get_from_title_id(title_id: &str) -> Result<Option<Self>> {
        let locale = default_locale();
        let query = format!("SELECT * FROM metaview_{locale} WHERE title_id = $tid");
        let mut query = DB.query(query).bind(("tid", title_id.to_string())).await?;
        let data: Option<Metaview> = query.take(0)?;
        Ok(data)
    }

    pub async fn get_from_download_id(download_id: &str) -> Result<Option<Self>> {
        let locale = default_locale();
        let query = format!("SELECT * FROM metaview_{locale} WHERE download_id = $did");
        let mut query = DB
            .query(query)
            .bind(("did", download_id.to_string()))
            .await?;
        let data: Option<Metaview> = query.take(0)?;
        Ok(data)
    }

    pub async fn get_all_titles() -> Result<Vec<Self>> {
        let locale = default_locale();
        let query = format!("SELECT * FROM metaview_{locale}");
        let mut query = DB.query(query).await?;
        let data: Vec<Metaview> = query.take(0)?;
        Ok(data)
    }

    pub async fn get_all_non_base_titles() -> Result<Vec<Self>> {
        let locale = default_locale();
        let query = format!(
            "SELECT * FROM metaview_{locale}
            WHERE title_id
            AND not(string::ends_with(title_id, '000'))"
        );
        let mut query = DB.query(query).await?;
        let data: Vec<Metaview> = query.take(0)?;
        Ok(data)
    }

    /// Get all download IDs of a give
    pub async fn get_download_ids(title_id: &str) -> Result<Vec<String>> {
        let locale = default_locale();
        let title_id_prefix = &title_id[..12];

        let query = format!(
            "SELECT * FROM metaview_{locale}
            WHERE string::starts_with(title_id, $tid_pfx)"
        );
        let mut query = DB
            .query(query)
            .bind(("tid_pfx", title_id_prefix.to_string()))
            .await?;

        let data: Vec<Metaview> = query.take(0)?;

        // Convert Metaview items to titleId strings
        let title_ids = data
            .into_iter()
            .filter_map(|t| t.download_id)
            .collect();

        Ok(title_ids)
    }

    pub async fn get_base_games() -> Result<Vec<Self>> {
        let locale = default_locale();
        let query = format!(
            "SELECT * FROM metaview_{locale} WHERE title.titleId AND string::ends_with(title.titleId, '000')"
        );
        let mut query = DB.query(query).await?;
        let data: Vec<Metaview> = query.take(0)?;
        Ok(data)
    }

    pub async fn get_updates(locale: &str) -> Result<Vec<Self>> {
        let query = format!(
            "SELECT * FROM metaview_{locale} WHERE title.titleId AND string::ends_with(title.titleId, '800')"
        );
        let mut query = DB.query(query).await?;
        let data: Vec<Metaview> = query.take(0)?;
        Ok(data)
    }

    /// Search for all DLC titles.
    pub async fn get_dlc() -> Result<Vec<Self>> {
        let locale = default_locale();
        let query = format!(
            "SELECT * FROM metaview_{locale}
            WHERE title.titleId
            AND not string::ends_with(title.titleId, '000')
            AND not string::ends_with(title.titleId, '800')"
        );
        let mut query = DB.query(query).await?;
        let data: Vec<Metaview> = query.take(0)?;
        Ok(data)
    }

    /// Search for all base game titles.
    pub async fn search_base_game(search_query: &SearchQuery) -> Result<Vec<Title>> {
        let locale = default_locale();
        let mut query = format!(
            "SELECT * FROM metaview_{locale}
            WHERE string::ends_with(title_id, '000')
            AND title_name @@ $query"
        );

        if search_query.limit.is_some() {
            query.push_str(" LIMIT $limit");
        }
        let mut query = DB
            .query(query)
            .bind(("query", search_query.query.clone()))
            .bind(("limit", search_query.limit.unwrap_or(100)))
            .await?;
        let data: Vec<Self> = query.take(0)?;

        let data = data.into_iter().filter_map(|m| m.title).collect();
        Ok(data)
    }

    /// Search for all titles, excluding updates.
    pub async fn search_all(search_query: &SearchQuery) -> Result<Vec<Title>> {
        let locale = LOCALE.parse::<String>()?;
        let mut query = format!(
            "SELECT * FROM metaview_{locale}
            WHERE not(string::ends_with(title_id, '800'))
            AND title_name @@ $query"
        );

        if search_query.limit.is_some() {
            query.push_str(" LIMIT $limit");
        }
        let mut query = DB
            .query(query)
            .bind(("query", search_query.query.clone()))
            .bind(("limit", search_query.limit.unwrap_or(100)))
            .await?;
        let data: Vec<Self> = query.take(0)?;

        let data = data.into_iter().filter_map(|m| m.title).collect();
        Ok(data)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Index {
    Thing(Thing),
    // #[serde(default)]
    TitleId(String),
}

/// TitleDB import entry
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleDbEntry {
    #[serde(rename(deserialize = "id"))]
    pub title_id: Option<String>,

    #[serde(rename = "ids")]
    #[serde(default)]
    pub title_ids: Vec<String>,

    #[serde(default)]
    pub banner_url: Option<String>,

    #[serde(default)]
    pub developer: Option<String>,

    #[serde(default)]
    pub front_box_art: Option<String>,

    #[serde(default)]
    pub icon_url: Option<String>,

    #[serde(default)]
    pub intro: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub category: Option<Vec<String>>,

    #[serde(default)]
    pub is_demo: Option<bool>,

    #[serde(default)]
    pub key: Option<String>,

    #[serde(default)]
    pub languages: Option<Vec<String>>,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub nsu_id: Option<u64>,

    #[serde(default)]
    pub number_of_players: Option<u8>,

    #[serde(default)]
    pub rating: Option<u32>,

    #[serde(default)]
    pub rating_content: Option<Vec<String>>,

    #[serde(default)]
    pub region: Option<String>,

    #[serde(default)]
    #[serde(deserialize_with = "deser_to_string")]
    pub release_date: Option<String>,

    #[serde(default)]
    pub rights_id: Option<String>,

    #[serde(default)]
    pub screenshots: Option<Vec<String>>,

    #[serde(default)]
    pub size: Option<u64>,

    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub publisher: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Title {
    #[serde(default, rename = "id")]
    pub index_id: Option<Thing>,

    // #[serde(rename = "id")]
    // #[serde(rename(serialize = "title_id"))]
    // #[serde(default)]
    pub title_id: Option<String>,

    /// Multiple Title IDs
    #[serde(default)]
    pub title_ids: Vec<String>,

    #[serde(default)]
    pub banner_url: Option<String>,

    #[serde(default)]
    pub developer: Option<String>,

    #[serde(default)]
    pub front_box_art: Option<String>,

    #[serde(default)]
    pub icon_url: Option<String>,

    #[serde(default)]
    pub intro: Option<String>,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub category: Option<Vec<String>>,

    #[serde(default)]
    pub is_demo: Option<bool>,

    #[serde(default)]
    pub key: Option<String>,

    #[serde(default)]
    pub languages: Option<Vec<String>>,

    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    #[serde(rename(serialize = "id"))]
    pub nsu_id: Option<u64>,

    #[serde(default)]
    pub number_of_players: Option<u8>,

    #[serde(default)]
    pub rating: Option<u32>,

    #[serde(default)]
    pub rating_content: Option<Vec<String>>,

    #[serde(default)]
    pub region: Option<String>,

    #[serde(default)]
    #[serde(deserialize_with = "deser_to_string")]
    pub release_date: Option<String>,

    #[serde(default)]
    pub rights_id: Option<String>,

    #[serde(default)]
    pub screenshots: Option<Vec<String>>,

    #[serde(default)]
    pub size: Option<u64>,

    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub publisher: Option<String>,
}

impl Title {
    pub async fn count(locale: &str) -> Result<i64> {
        #[derive(Debug, Deserialize)]
        struct CountResult {
            count: i64,
        }

        let query = format!("SELECT count() FROM titles_{} GROUP BY count", locale);
        let mut res = DB.query(query).await?;
        tracing::trace!("Count query: {:?}", res);
        let result: Option<CountResult> = res.take(0)?;
        let count = result.map(|r| r.count).unwrap_or_default();
        tracing::trace!("Title count: {:?}", count);
        Ok(count)
    }

    pub async fn get_from_title_id(locale: &str, title_id: &str) -> Result<Option<Self>> {
        // If the title ID ends with *800, it's an update for a game,
        // So we can replace it with 000 to get the base game
        let is_update = title_id.ends_with("800");

        let title_id_query = if is_update {
            tracing::trace!("Fetching base game metadata for update");
            title_id.replace("800", "000")
        } else {
            title_id.to_string()
        };

        let query =
            format!("SELECT * FROM titles_{locale} WHERE titleId = $tid OR ids CONTAINS $tid");
        let mut query = DB
            .query(query)
            // .bind(("table", format!("titles_{lang}")))
            .bind(("tid", title_id_query))
            .await?;
        let mut data: Option<Self> = query.take(0)?;

        if is_update {
            if let Some(title) = data.as_mut() {
                // Append (Update) to the title name
                if let Some(name) = &mut title.name {
                    name.push_str(" (Update)");
                }
                title.title_id = Some(title_id.to_string());
                title.title_ids = vec![title_id.to_string()];
            }
        }

        // modify the title id

        // todo:
        // else, get otherApplicationId from index and query again

        Ok(data)
    }

    pub async fn search(search_query: &SearchQuery) -> Result<Vec<Self>> {
        let locale = crate::config::config().backend_config.get_locale_string();
        let mut query = format!(
            "SELECT * FROM titles_{locale}
            WHERE name @@ $query
            AND titleId
            AND string::ends_with(titleId, '000')
            "
        );

        if search_query.limit.is_some() {
            query.push_str(" LIMIT $limit");
        }
        let mut query = DB
            .query(query)
            .bind(("query", search_query.query.clone()))
            .bind(("limit", search_query.limit.unwrap_or(100)))
            .await?;
        let data: Vec<Self> = query.take(0)?;

        Ok(data)
    }

    pub async fn get_from_metaview_cache(title_id: &str) -> Result<Option<Self>> {
        let is_update = title_id.ends_with("800");
        let locale = crate::config::config().backend_config.get_locale_string();

        let title_id_query = if is_update {
            tracing::trace!("Fetching base game metadata for update");
            title_id.replace("800", "000")
        } else {
            title_id.to_string()
        };

        tracing::trace!("Fetching title metadata for {title_id_query}");

        let query = format!("SELECT * FROM metaview_{locale} WHERE title.titleId = $tid");
        let mut query = DB.query(query).bind(("tid", title_id_query)).await?;
        let data: Option<Metaview> = query.take(0)?;
        let mut data = data.and_then(|r| r.title);

        if is_update {
            if let Some(title) = data.as_mut() {
                // Append (Update) to the title name
                if let Some(name) = &mut title.name {
                    name.push_str(" (Update)");
                }
                title.title_id = Some(title_id.to_string());
                title.title_ids = vec![title_id.to_string()];
            }
        }
        Ok(data)
    }
}

#[tracing::instrument(skip(title), fields(
    title_id = title.title_id.clone(),
    nsuid = title.nsu_id.unwrap_or_default(),
    locale,
))]

async fn import_entry_to_db(title: TitleDbEntry, locale: &str) -> Result<()> {
    if title.title_id.is_none() {
        return Ok(());
    }

    let table_name = format!("titles_{}", locale);
    let nsuid = title.nsu_id.unwrap_or_default();
    let nsuid_str = nsuid.to_string(); // Convert u64 to String because surrealdb doesnt like numbered indexes
    let _ent: Option<Title> = DB.upsert((&table_name, &nsuid_str)).content(title).await?;

    tracing::trace!("Title imported");

    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TitleDBImport {
    // #[serde(flatten)]
    // titles: BTreeMap<String, TitleDbEntry>,
}

impl TitleDBImport {
    // pub fn new() -> Self {
    //     TitleDBImport {
    //         titles: BTreeMap::new(),
    //     }
    // }

    #[tracing::instrument(skip(reader))]
    pub async fn from_json_reader_streaming<R: std::io::Read>(
        reader: R,
        locale: &str,
    ) -> color_eyre::Result<()> {
        tracing::info!("Importing TitleDB data for {locale} (Streamed)");

        // Create schema for table
        let schema = include_str!("surql/titledb.surql").replace("%LOCALE%", locale);
        let _q = DB.query(schema).await?;
        create_precomputed_metaview(locale).await?;

        let mut reader = JsonStreamReader::new(reader);

        reader.begin_object().unwrap();

        while reader.has_next().unwrap() {
            // Skip the key
            reader.next_name().unwrap();
            // let key = key_og.clone();
            // drop(key_og);

            let entry: TitleDbEntry = reader.deserialize_next().unwrap();
            // tracing::info!("Read key: {:#?}", entry);
            //
            // let nsuid = entry.nsu_id.unwrap_or_default();
            import_entry_to_db(entry.clone(), locale).await.unwrap();

            // db.titles.insert(nsuid.to_string(), entry);
        }

        reader.end_object().unwrap();

        tracing::info!("Successfully imported TitleDB data for {locale}");
        Ok(())
    }

    // pub fn from_json_reader<R: std::io::Read>(reader: R) -> Result<Self, serde_json::Error> {
    //     serde_json::from_reader(reader)
    // }

    // // db is the database suffix, (i.e "US-en" or "US-es")
    // pub async fn import_to_db_sync(self, sfx: &str) -> Result<()> {
    //     for (_nsu_id, title) in self.titles {
    //         import_entry_to_db(title, &sfx).await?
    //     }

    //     Ok(())
    // }
}
