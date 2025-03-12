use crate::db::DB;
use color_eyre::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    pub fn parse(filename: &str) -> Self {
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
}

use serde::Deserializer;

fn deser_u64_to_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<u64>::deserialize(deserializer)?;
    Ok(opt.map(|val| val.to_string()))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Title {
    // #[serde(skip_deserializing)]
    // #[serde(default, rename(serialize = "id"))]
    // pub index_id: Option<Thing>,

    #[serde(rename = "id")]
    #[serde(default)]
    pub title_id: Option<String>,

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
    #[serde(deserialize_with = "deser_u64_to_string")]
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
pub struct TitleDBImport {
    #[serde(flatten)]
    titles: BTreeMap<String, Title>,
}

impl TitleDBImport {
    pub fn new() -> Self {
        TitleDBImport {
            titles: BTreeMap::new(),
        }
    }
    pub fn from_json_reader<R: std::io::Read>(reader: R) -> Result<Self, serde_json::Error> {
        serde_json::from_reader(reader)
    }

    // db is the database suffix, (i.e "US-en" or "US-es")
    pub async fn import_to_db(self, sfx: &str) -> Result<()> {
        let table_name = format!("titles_{}", sfx);
        for (_nsu_id, mut title) in self.titles {
            let nsuid = title.nsu_id.clone().unwrap_or_default();
            // let tid = title.clone().title_id;

            // let mut new_title = title.clone();
            // title.index_id = Some(Thing::from((table_name.clone(), nsuid.to_string())));

            // Store the name before title is moved
            let name = title.name.clone();
            let title_id = title.title_id.clone();

            let _ent: Option<Title> = DB.upsert((&table_name, &nsuid.to_string())).content(title).await?;

            tracing::info!(
                "Imported title: {name:?} ([{tid:?}]) ({nsuid:?})",
                name = name,
                tid = title_id,
                nsuid = nsuid
            );
        }

        Ok(())
    }
}
