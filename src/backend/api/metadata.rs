// filepath: /home/cappy/Projects/alumulemu/src/backend/api/metadata.rs
use axum::{
    extract::{Path, Query},
    Json,
    response::{IntoResponse, Response},
};
use http::StatusCode;

use crate::{
    db::NspMetadata,
    index::TinfoilResponse,
    router::AlumRes,
    titledb::{GameFileDataNaive, Metaview, Title},
};

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SearchQuery {
    #[serde(rename = "q")]
    pub query: String,
    #[serde(rename = "limit")]
    pub limit: Option<usize>,
}

#[derive(serde::Serialize, Debug)]
pub struct GroupedGameListResponse {
    pub base_game: Title,
    pub versions: Vec<Title>,
}

#[tracing::instrument]
pub async fn title_meta(
    Path(title_id_param): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    tracing::trace!("Getting title metadata for {}", title_id_param);

    // Then get the title info from metaview cache
    let title = Title::get_from_metaview_cache(&title_id_param)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(title).into_response())
}

/// Get base game of a title
#[tracing::instrument]
pub async fn title_meta_base_game(
    Path(title_id_param): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    tracing::trace!("Getting base game metadata for {}", title_id_param);

    // Get all metadata entries
    let nsp_metadata = NspMetadata::get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Find base game that matches first 12 chars and ends with 000
    let base_game_id = &title_id_param[..12];
    let base_metadata = nsp_metadata
        .iter()
        .find(|m| m.title_id.starts_with(base_game_id) && m.title_id.ends_with("000"))
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get full title info from cache using the found base game ID
    let title = Title::get_from_metaview_cache(&base_metadata.title_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(title))
}

/// Get all alternate (non-base) versions of a title
pub async fn get_download_ids(Path(title_id): Path<String>) -> AlumRes<Json<Vec<String>>> {
    let view = Metaview::get_download_ids(&title_id).await?;
    Ok(Json(view))
}

/// Enter in the base title ID of the game (or the first 13 characters of the title ID) to get all versions of the game
/// This is useful for games that have multiple versions, like updates or DLCs
#[tracing::instrument]
pub async fn list_grouped_by_titleid(
    Path(title_id_param): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let nsp_metadata = NspMetadata::get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let base_game_id = title_id_param[..12].to_string();
    // First try to find the base game in our local metadata
    let base_game_metadata = nsp_metadata
        .iter()
        .find(|m| m.title_id.starts_with(&base_game_id[..12]) && m.title_id.ends_with("000"))
        .ok_or(StatusCode::NOT_FOUND)?;

    // Then get the full title info from cache
    let base_game = Title::get_from_metaview_cache(&base_game_metadata.title_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut versions = Vec::new();
    for metadata in nsp_metadata
        .iter()
        .filter(|m| m.title_id.starts_with(&base_game_id[..12]))
    {
        if !metadata.title_id.ends_with("000") {
            if let Ok(Some(title)) =
                Title::get_from_metaview_cache(&metadata.title_id).await
            {
                versions.push(title);
            }
        }
    }

    let response = GroupedGameListResponse {
        base_game,
        versions,
    };

    Ok(Json(response).into_response())
}

/// List base games only (games that end with 000)
#[tracing::instrument]
pub async fn list_base_games() -> Result<impl IntoResponse, StatusCode> {
    let base_games = Metaview::get_base_games()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter_map(|meta| meta.title)
        .collect::<Vec<_>>();

    Ok(Json(base_games).into_response())
}

pub async fn search_titledb(
    query: Query<SearchQuery>,
) -> AlumRes<Json<Vec<Title>>> {
    tracing::debug!(?query, "Searching for title with query");

    let search = Title::search(&query).await?;

    Ok(Json(search))
}

pub async fn search_base_game(
    query: Query<SearchQuery>,
) -> AlumRes<Json<Vec<Title>>> {
    let query = query.0;

    tracing::debug!(?query, "Searching for base game with query");

    let search = Metaview::search_base_game(&query)
        .await?
        .to_vec();

    Ok(Json(search))
}

pub async fn search_titles(query: Query<SearchQuery>) -> AlumRes<Json<Vec<Title>>> {
    let query = query.0;

    tracing::debug!(?query, "Searching for title with query");

    let search = Title::search(&query).await?.to_vec();

    Ok(Json(search))
}