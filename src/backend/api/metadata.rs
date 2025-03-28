use axum::{
    Json, Router,
    extract::{Path, Query},
    response::IntoResponse,
    routing::get,
};
use http::StatusCode;

use crate::{
    db::NspMetadata,
    router::AlumRes,
    titledb::{Metaview, Title},
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

    // Then get the title info from metaview cache with better error handling
    match Title::get_from_metaview_cache(&title_id_param).await {
        Ok(Some(title)) => Ok(Json(title).into_response()),
        Ok(None) => {
            tracing::warn!("Title not found for ID: {}", title_id_param);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            tracing::error!(
                "Database error when fetching title {}: {}",
                title_id_param,
                e
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get base game of a title
#[tracing::instrument]
pub async fn title_meta_base_game(
    Path(title_id_param): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    tracing::trace!("Getting base game metadata for {}", title_id_param);

    // Get all metadata entries with better error handling
    let nsp_metadata = match NspMetadata::get_all().await {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!("Failed to get metadata: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Validate title_id_param length before extracting substring
    if title_id_param.len() < 12 {
        tracing::error!("Invalid title ID format: {}", title_id_param);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Find base game that matches first 12 chars and ends with 000
    let base_game_id = &title_id_param[..12];

    let base_metadata = match nsp_metadata
        .iter()
        .find(|m| m.title_id.starts_with(base_game_id) && m.title_id.ends_with("000"))
    {
        Some(metadata) => metadata,
        None => {
            tracing::warn!("Base game not found for ID prefix: {}", base_game_id);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Get full title info from cache using the found base game ID
    match Title::get_from_metaview_cache(&base_metadata.title_id).await {
        Ok(Some(title)) => Ok(Json(title)),
        Ok(None) => {
            tracing::warn!(
                "Title metadata not found for ID: {}",
                base_metadata.title_id
            );
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get all alternate (non-base) versions of a title
pub async fn get_download_ids(Path(title_id): Path<String>) -> AlumRes<Json<Vec<String>>> {
    match Metaview::get_download_ids(&title_id).await {
        Ok(view) => Ok(Json(view)),
        Err(e) => {
            tracing::error!("Failed to get download IDs for {}: {}", title_id, e);
            Err(e.into())
        }
    }
}

/// Enter in the base title ID of the game (or the first 13 characters of the title ID) to get all versions of the game
/// This is useful for games that have multiple versions, like updates or DLCs
#[tracing::instrument]
pub async fn list_grouped_by_titleid(
    Path(title_id_param): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Validate title ID length
    if title_id_param.len() < 12 {
        tracing::error!("Invalid title ID format (too short): {}", title_id_param);
        return Err(StatusCode::BAD_REQUEST);
    }

    let nsp_metadata = match NspMetadata::get_all().await {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!("Failed to get metadata: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let base_game_id = title_id_param[..12].to_string();

    // First try to find the base game in our local metadata
    let base_game_metadata = match nsp_metadata
        .iter()
        .find(|m| m.title_id.starts_with(&base_game_id) && m.title_id.ends_with("000"))
    {
        Some(metadata) => metadata,
        None => {
            tracing::warn!("Base game not found for ID prefix: {}", base_game_id);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Then get the full title info from cache
    let base_game = match Title::get_from_metaview_cache(&base_game_metadata.title_id).await {
        Ok(Some(title)) => title,
        Ok(None) => {
            tracing::warn!(
                "Title metadata not found for ID: {}",
                base_game_metadata.title_id
            );
            return Err(StatusCode::NOT_FOUND);
        }
        Err(e) => {
            tracing::error!("Database error: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let mut versions = Vec::new();

    for metadata in nsp_metadata
        .iter()
        .filter(|m| m.title_id.starts_with(&base_game_id))
    {
        if !metadata.title_id.ends_with("000") {
            match Title::get_from_metaview_cache(&metadata.title_id).await {
                Ok(Some(title)) => versions.push(title),
                Ok(None) => {
                    tracing::debug!("No title metadata for ID: {}", metadata.title_id);
                    // Continue without this version rather than failing
                }
                Err(e) => {
                    tracing::warn!("Error getting metadata for {}: {}", metadata.title_id, e);
                    // Continue without this version rather than failing
                }
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
    match Metaview::get_base_games().await {
        Ok(base_games) => {
            let filtered_games = base_games
                .into_iter()
                .filter_map(|meta| meta.title)
                .collect::<Vec<_>>();

            Ok(Json(filtered_games).into_response())
        }
        Err(e) => {
            tracing::error!("Failed to get base games: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn search_titledb(query: Query<SearchQuery>) -> AlumRes<Json<Vec<Title>>> {
    tracing::debug!(?query, "Searching for title with query");

    match Title::search(&query).await {
        Ok(search) => Ok(Json(search)),
        Err(e) => {
            tracing::error!("Search failed: {}", e);
            Err(e.into())
        }
    }
}

pub async fn search_base_game(query: Query<SearchQuery>) -> AlumRes<Json<Vec<Title>>> {
    let query = query.0;
    tracing::debug!(?query, "Searching for base game with query");

    match Metaview::search_base_game(&query).await {
        Ok(search) => Ok(Json(search)),
        Err(e) => {
            tracing::error!("Base game search failed: {}", e);
            Err(e.into())
        }
    }
}

pub async fn search_titles(query: Query<SearchQuery>) -> AlumRes<Json<Vec<Title>>> {
    let query = query.0;
    tracing::debug!(?query, "Searching for title with query");

    match Title::search(&query).await {
        Ok(search) => Ok(Json(search)),
        Err(e) => {
            tracing::error!("Title search failed: {}", e);
            Err(e.into())
        }
    }
}

/// Creates a router for all metadata-related endpoints
pub fn metadata_api() -> Router {
    Router::new()
        .route("/title_meta/{title_id}", get(title_meta))
        .route(
            "/title_meta/{title_id}/base_game",
            get(title_meta_base_game),
        )
        .route("/title_meta/{title_id}/download_ids", get(get_download_ids))
        .route("/grouped/{title_id}", get(list_grouped_by_titleid))
        .route("/base_games", get(list_base_games))
        .route("/base_games/search", get(search_base_game))
        .route("/titledb/search", get(search_titledb))
        .route("/search", get(search_titles))
}
