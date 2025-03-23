use crate::nst::{get_title_id_and_version, parse_cnmt_output, run_nstool};
use crate::{db::DB, db::NspMetadata};
use color_eyre::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
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

    pub async fn get(path: &Path, all_metadata: &[NspMetadata]) -> Result<Self> {
        let filename = path.file_name().unwrap().to_str().unwrap();
        let extension = path.extension().unwrap_or_default().to_str().unwrap();

        if extension == "nsp" {
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
                tracing::debug!("Reading NSP file: {:?}", filename);
                let cnmt_output = run_nstool(path.to_str().unwrap());
                let cnmt = parse_cnmt_output(&cnmt_output);
                let (title_id, version) = get_title_id_and_version(cnmt);
                tracing::debug!("Title ID: {:?}", title_id);
                tracing::debug!("Version: {:?}", version);

                let metadata = NspMetadata {
                    path: path.to_str().unwrap().to_string(),
                    title_id: title_id.clone(),
                    version: version.clone(),
                    title_name: None,
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
    pub async fn get_from_title_id(lang: &str, title_id: &str) -> Result<Option<Self>> {
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
            format!("SELECT * FROM titles_{lang} WHERE titleId = $tid OR ids CONTAINS $tid");
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
}
#[tracing::instrument(skip(title), fields(title_id = title.title_id.clone(), nsuid = title.nsu_id.unwrap_or_default()))]
async fn import_entry_to_db(title: TitleDbEntry, db_sfx: &str) -> Result<()> {
    let table_name = format!("titles_{}", db_sfx);
    let nsuid = title.nsu_id.unwrap_or_default();
    let name = title.name.clone();
    let title_id = title.title_id.clone();
    let nsuid_str = nsuid.to_string(); // Convert u64 to String
    let _ent: Option<Title> = DB.upsert((&table_name, &nsuid_str)).content(title).await?;

    tracing::trace!(
        "Imported title: {name:?} ([{tid:?}]) ({nsuid:?})",
        name = name,
        tid = title_id,
        nsuid = nsuid
    );

    Ok(())
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TitleDBImport {
    #[serde(flatten)]
    titles: BTreeMap<String, TitleDbEntry>,
}

impl TitleDBImport {
    pub fn new() -> Self {
        TitleDBImport {
            titles: BTreeMap::new(),
        }
    }

    #[tracing::instrument(skip(reader))]
    pub async fn from_json_reader_streaming<R: std::io::Read>(
        reader: R,
        db_sfx: &str,
    ) -> Result<(), serde_json::Error> {
        tracing::info!("Importing TitleDB data for {db_sfx} (Streamed)");
        let mut db = Self::new();
        let mut reader = JsonStreamReader::new(reader);

        // let a = stream_reader.next_string_reader();
        // let s = stream_reader.next_string().unwrap();
        // tracing::info!("Read string: {}", s);

        reader.begin_object().unwrap();

        while reader.has_next().unwrap() {
            // Skip the key
            reader.next_name().unwrap();
            // let key = key_og.clone();
            // drop(key_og);

            let entry: TitleDbEntry = reader.deserialize_next().unwrap();
            // tracing::info!("Read key: {:#?}", entry);
            //
            let nsuid = entry.nsu_id.unwrap_or_default();
            import_entry_to_db(entry.clone(), db_sfx).await.unwrap();

            db.titles.insert(nsuid.to_string(), entry);
        }

        reader.end_object().unwrap();

        tracing::info!("Successfully imported TitleDB data for {db_sfx}");
        Ok(())
    }

    pub fn from_json_reader<R: std::io::Read>(reader: R) -> Result<Self, serde_json::Error> {
        serde_json::from_reader(reader)
    }

    // db is the database suffix, (i.e "US-en" or "US-es")
    pub async fn import_to_db_sync(self, sfx: &str) -> Result<()> {
        for (_nsu_id, title) in self.titles {
            import_entry_to_db(title, &sfx).await?
        }

        Ok(())
    }
}
