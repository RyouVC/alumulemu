use std::path::Path;

use crate::backend::user::basic_auth_if_public;
use crate::backend::user::user_router;
use crate::db::NspMetadata;
use crate::games_dir;
use crate::index::{Index, TinfoilResponse};

use crate::titledb::GameFileDataNaive;
use crate::titledb::Metaview;
use crate::util::format_download_id;
use crate::util::format_game_name;
use axum::extract::Query;
use axum::middleware;
use axum::{
    Json, Router,
    extract::Path as HttpPath,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};

use tokio_util::io::ReaderStream;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0:?}")]
    Error(#[from] color_eyre::Report),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = StatusCode::INTERNAL_SERVER_ERROR;
        let body = Json(TinfoilResponse::Failure(self.to_string()));
        (status, body).into_response()
    }
}

type AlumRes<T> = Result<T, Error>;

#[derive(serde::Deserialize, serde::Serialize, Debug, Default)]
pub struct RescanOptions {
    #[serde(default)]
    pub rescan: bool,
}

#[tracing::instrument]
pub async fn update_metadata_from_filesystem(path: &str, options: RescanOptions) -> color_eyre::eyre::Result<()> {
    tracing::info!("Starting full metadata rescan of {}", path);

    let rescan = options.rescan;

    // Get all existing metadata
    let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());

    // Create a lookup map for faster metadata lookups
    let metadata_map: std::collections::HashMap<String, &NspMetadata> =
        all_metadata.iter().map(|m| (m.path.clone(), m)).collect();

    // Define valid extensions once
    const VALID_EXTENSIONS: [&str; 5] = ["nsp", "xci", "nsz", "ncz", "xcz"];

    // Track which files we've seen during this scan
    let mut found_paths = std::collections::HashSet::new();

    // Walk the directory and process each file
    let walker = jwalk::WalkDir::new(path)
        .skip_hidden(true)
        .process_read_dir(move |_, _, _, dir_entry_results| {
            // Sort entry results to process largest files first (optimization for typical use cases)
            dir_entry_results.sort_by_cached_key(|entry_result| {
                if let Ok(entry) = entry_result {
                    let metadata = entry.metadata();
                    if let Ok(metadata) = metadata {
                        return std::cmp::Reverse(metadata.len());
                    }
                }
                std::cmp::Reverse(0)
            });
        });

    for entry in walker.into_iter() {
        let path = entry.map_err(|e| Error::Error(color_eyre::eyre::eyre!(e.to_string())))?;
        let file_path = path.path();

        // Extract extension early for filtering
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            if !VALID_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                continue;
            }
        } else {
            continue;
        }

        let file_path_str = file_path.to_string_lossy().to_string();
        found_paths.insert(file_path_str.clone());

        // Check if we need to update this file
        // Force rescan if the rescan option is true, otherwise only scan if no metadata exists
        let needs_update = rescan || !metadata_map.contains_key(&file_path_str);

        if needs_update {
            // Use the dedicated scan_file function instead of duplicating code
            if let Err(e) = scan_file(&file_path, rescan).await {
                tracing::warn!("Failed to scan file {}: {}", file_path_str, e);
            }
        }
    }

    // Delete metadata for files that no longer exist
    let mut deleted_count = 0;
    for metadata in all_metadata.iter().filter(|m| !found_paths.contains(&m.path)) {
        tracing::info!("Removing metadata for non-existent file: {}", metadata.path);
        if let Err(e) = metadata.delete().await {
            tracing::error!("Failed to delete metadata for {}: {}", metadata.path, e);
        } else {
            deleted_count += 1;
        }
    }

    tracing::info!(
        "Metadata rescan complete. Found {} files, removed {} stale entries.",
        found_paths.len(),
        deleted_count
    );

    Ok(())
}

pub async fn scan_file(path: &Path, rescan_files: bool) -> color_eyre::Result<()> {
    tracing::info!("Scanning file: {}", path.display());

    // Get all existing metadata
    let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());

    let file_path_str = path.to_string_lossy().to_string();

    // Log that we're updating this path
    tracing::info!("Updating metadata for file: {}", file_path_str);

    const MAX_RETRIES: usize = 3;
    const RETRY_DELAY_MS: u64 = 500;

    tracing::debug!("Processing file: {}", file_path_str);

    let mut attempt = 0;
    let metadata_result = loop {
        attempt += 1;
        let naive = {
            if rescan_files {
                GameFileDataNaive::get(path).await
            } else {
                GameFileDataNaive::get_cached(path, &all_metadata).await
            }
        };
        match naive {
            Ok(game_data) => {
                let title_id = game_data
                    .title_id
                    .unwrap_or_else(|| "00000000AAAA0000".to_string());

                // Get the title name from metadata or filename
                let title_name = all_metadata
                    .iter()
                    .find(|m| m.path == file_path_str)
                    .and_then(|m| m.title_name.clone())
                    .unwrap_or_else(|| {
                        game_data.name.trim().trim_end_matches(".nsp").to_string()
                    });

                let version = game_data.version.unwrap_or_else(|| "v0".to_string());
                let extension = game_data.extension.unwrap_or_default();
                let download_id = format_download_id(&title_id, &version, &extension);
                break Some(NspMetadata {
                    path: file_path_str.clone(),
                    title_id,
                    version,
                    title_name: Some(title_name),
                    download_id,
                });
            }
            Err(e) => {
                if e.to_string().contains("This transaction can be retried")
                    && attempt < MAX_RETRIES
                {
                    tracing::info!(
                        "Retryable error on attempt {}/{} for {}: {}. Retrying...",
                        attempt,
                        MAX_RETRIES,
                        file_path_str,
                        e
                    );
                    tokio::time::sleep(tokio::time::Duration::from_millis(
                        RETRY_DELAY_MS,
                    ))
                    .await;
                    continue;
                }

                tracing::warn!(
                    "Failed to get game data for {} after {} attempt(s): {}",
                    file_path_str,
                    attempt,
                    e
                );
                break None;
            }
        }
    };

    if let Some(metadata) = metadata_result {
        if let Err(e) = metadata.save().await {
            tracing::warn!("Failed to save metadata: {}", e);
        }
    }
    Ok(())
}

#[tracing::instrument]
pub async fn watch_filesystem_for_changes(path: &str) -> color_eyre::eyre::Result<()> {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
    // use std::path::PathBuf;
    // use std::sync::Arc;
    use tokio::sync::mpsc;

    tracing::info!("Starting filesystem watcher for: {}", path);

    // Define valid extensions once
    // const VALID_EXTENSIONS: [&str; 5] = ["nsp", "xci", "nsz", "ncz", "xcz"];

    // Create a channel to receive events
    let (tx, mut rx) = mpsc::channel(100);

    // Create a watcher with immediate events
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.blocking_send(event); // Send events to our channel
            }
        },
        Config::default(),
    )?;

    // Start watching the specified directory recursively
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    // Clone path for use in async move block
    let path_owned = path.to_owned();

    // Process events in a background task
    tokio::spawn(async move {
        process_fs_events(&mut rx, &path_owned).await;
    });

    // Keep the watcher alive
    std::mem::forget(watcher);

    Ok(())
}

// todo: refactor stuff into a single function
async fn process_fs_events(rx: &mut tokio::sync::mpsc::Receiver<notify::Event>, _path: &str) {
    use notify::EventKind;

    // Define valid extensions
    const VALID_EXTENSIONS: [&str; 5] = ["nsp", "xci", "nsz", "ncz", "xcz"];

    while let Some(event) = rx.recv().await {
        // Get the path from the event
        let event_path = match event.paths.first() {
            Some(path) => path,
            None => continue,
        };

        // Check if the file has a valid extension
        if let Some(ext) = event_path.extension().and_then(|e| e.to_str()) {
            if !VALID_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                continue;
            }
        } else {
            continue;
        }

        let path_str = event_path.to_string_lossy().to_string();

        match event.kind {
            EventKind::Create(_) | EventKind::Modify(_) => {
                tracing::info!("File created/modified: {}", path_str);

                // Get all existing metadata
                let all_metadata = std::sync::Arc::new(
                    NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new()),
                );

                // Process the new/modified file
                match GameFileDataNaive::get_cached(event_path, &all_metadata).await {
                    Ok(game_data) => {
                        let title_id = game_data
                            .title_id
                            .unwrap_or_else(|| "00000000AAAA0000".to_string());

                        // Get the title name
                        let title_name = all_metadata
                            .iter()
                            .find(|m| m.path == path_str)
                            .and_then(|m| m.title_name.clone())
                            .unwrap_or_else(|| {
                                game_data.name.trim().trim_end_matches(".nsp").to_string()
                            });
                        
                        let extension = game_data.extension.unwrap_or_default();
                        let version = game_data.version.unwrap_or_else(|| "v0".to_string());
                        let download_id = format_download_id(&title_id, &version, &extension);

                        // Save the metadata
                        let metadata = NspMetadata {
                            path: path_str,
                            title_id,
                            version,
                            title_name: Some(title_name),
                            download_id,
                        };

                        if let Err(e) = metadata.save().await {
                            tracing::error!("Failed to save metadata: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get game data for {}: {}", path_str, e);
                    }
                }
            }
            EventKind::Remove(_) => {
                tracing::info!("File removed: {}", path_str);

                // Find and delete the metadata for this file
                let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());

                if let Some(metadata) = all_metadata.iter().find(|m| m.path == path_str) {
                    if let Err(e) = metadata.delete().await {
                        tracing::error!("Failed to delete metadata for {}: {}", path_str, e);
                    }
                }
            }
            _ => {} // Ignore other event types
        }
    }
}

#[tracing::instrument]
pub async fn generate_index_from_metadata() -> color_eyre::eyre::Result<Index> {
    let mut idx = Index::default();
    let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());

    for metadata in all_metadata {
        let path = std::path::Path::new(&metadata.path);
        let filename = path.file_name().unwrap().to_string_lossy().into_owned();
        let extension = path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or("nsp");

        // Use the refactored function to format the name
        let formatted_name = format_game_name(&metadata, &filename, extension);

        // Extract the version number without 'v' prefix
        let version_num = metadata.version.trim_start_matches('v');

        // Create a title ID with version and file extension appended
        let versioned_title_id = format!("{}_v{}.{}", metadata.title_id, version_num, extension);

        // Add the file with the versioned title ID to ensure we get the exact version and format
        idx.add_file(
            path,
            "/api/get_game",
            &formatted_name,
            Some(&versioned_title_id),
        );
    }

    Ok(idx)
}

pub async fn list_files() -> AlumRes<Json<Index>> {
    let games = generate_index_from_metadata().await?;
    tracing::trace!("Games retrieved: {:?}", games);
    Ok(Json(games))
}

pub async fn download_file(
    HttpPath(download_id_param): HttpPath<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Block any path traversal attempts
    if download_id_param.contains("..")
        || download_id_param.contains('/')
        || download_id_param.contains('\\')
    {
        tracing::warn!(
            "Path traversal attempt detected in download ID: {}",
            download_id_param
        );
        return Ok(Json(TinfoilResponse::Failure(
            "path traversal not allowed for this request".to_string(),
        ))
        .into_response());
    }

    tracing::debug!("Looking for download ID: {}", download_id_param);

    let all_metadata = NspMetadata::get_from_download_id(&download_id_param).await.map_err(|err| {
        tracing::error!("Failed to retrieve metadata for download ID {}: {}", download_id_param, err);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check that we found the metadata entry
    let metadata_entry = match all_metadata {
        Some(entry) => entry,
        None => {
            tracing::error!("No metadata found for download ID: {}", download_id_param);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    let file_path = &metadata_entry.path;
    tracing::debug!("Found file path: {}", file_path);

    let file = match tokio::fs::File::open(file_path).await {
        Ok(file) => file,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    // Get the raw filename and extension for formatting
    let path = std::path::Path::new(file_path);
    let raw_filename = path.file_name().unwrap().to_string_lossy().into_owned();
    let extension = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or("nsp");

    // Create a nicely formatted filename for the download
    let formatted_filename = format_game_name(&metadata_entry, &raw_filename, extension);

    // Sanitize the filename to ensure it's safe for Content-Disposition
    // Replace any characters that might cause issues in headers
    let safe_filename = formatted_filename.replace(['"', '\\'], "_");

    tracing::info!("Serving download with filename: {}", safe_filename);

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let response = Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe_filename}\""),
        )
        .body(body)
        .unwrap();

    Ok(response)
}

// todo: create precomputed view for this
#[tracing::instrument]
pub async fn title_meta(
    HttpPath(title_id_param): HttpPath<String>,
) -> Result<impl IntoResponse, StatusCode> {
    tracing::trace!("Getting title metadata for {}", title_id_param);

    // Then get the title info from metaview cache
    let title = crate::titledb::Title::get_from_metaview_cache(&title_id_param)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(title).into_response())
}

/// Get base game of a title
#[tracing::instrument]
pub async fn title_meta_base_game(
    HttpPath(title_id_param): HttpPath<String>,
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
    let title = crate::titledb::Title::get_from_metaview_cache(&base_metadata.title_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(title))
}


/// Get all alternate (non-base) versions of a title
pub async fn get_download_ids(HttpPath(title_id): HttpPath<String>) -> AlumRes<Json<Vec<String>>> {
    let view = Metaview::get_download_ids(&title_id).await?;
    Ok(Json(view))
}

#[derive(serde::Serialize, Debug)]
pub struct GroupedGameListResponse {
    pub base_game: crate::titledb::Title,
    pub versions: Vec<crate::titledb::Title>,
}

/// Enter in the base title ID of the game (or the first 13 characters of the title ID) to get all versions of the game
/// This is useful for games that have multiple versions, like updates or DLCs
#[tracing::instrument]
pub async fn list_grouped_by_titleid(
    HttpPath(title_id_param): HttpPath<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // tracing::debug!("Getting grouped game list for {}", title_id_param);
    // if title_id_param.len() < 13 {
    //     return Err(StatusCode::BAD_REQUEST);
    // }
    // tracing::debug!("Getting title metadata for {}", title_id_param);

    let nsp_metadata = NspMetadata::get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let base_game_id = title_id_param[..12].to_string();
    // tracing::debug!("Base game ID: {}", base_game_id);
    // First try to find the base game in our local metadata
    let base_game_metadata = nsp_metadata
        .iter()
        .find(|m| m.title_id.starts_with(&base_game_id[..12]) && m.title_id.ends_with("000"))
        .ok_or(StatusCode::NOT_FOUND)?;

    // Then get the full title info from cache
    let base_game = crate::titledb::Title::get_from_metaview_cache(&base_game_metadata.title_id)
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
                crate::titledb::Title::get_from_metaview_cache(&metadata.title_id).await
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

    let base_games = crate::titledb::Metaview::get_base_games()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .into_iter()
        .filter_map(|meta| meta.title)
        .collect::<Vec<_>>();

    Ok(Json(base_games).into_response())
}

pub async fn rescan_games(options: Query<RescanOptions>) -> AlumRes<Json<TinfoilResponse>> {
    tracing::info!("Rescanning games directory");
    update_metadata_from_filesystem(&games_dir(), options.0).await?;
    tracing::info!("Games rescanned successfully");
    // tracing::info!("(re)Creating precomputed metaview");
    // if let Err(e) = create_precomputed_metaview().await {
    //     tracing::warn!("Failed to create precomputed metaview: {}", e);
    // }
    Ok(Json(TinfoilResponse::MiscSuccess(
        "Games rescanned successfully".to_string(),
    )))
}

// test download RPC, won't be used in prod 
pub async fn test_dl() -> impl IntoResponse {
    let url = "http://example.com/download"; // Example URL for testing

    StatusCode::OK.into_response()
}

pub fn admin_router() -> Router {
    Router::new()
        .route("/rescan", post(rescan_games))
        .fallback(|| async {
            match std::fs::read_to_string("alu-panel/dist/index.html") {
                Ok(contents) => Html(contents).into_response(),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        })
        // Fix the middleware layering by using proper syntax
        .layer(axum::middleware::from_fn(crate::backend::user::basic_auth))
}

pub async fn search_titledb(query: Query<SearchQuery>) -> AlumRes<Json<Vec<crate::titledb::Title>>> {

    tracing::debug!(?query, "Searching for title with query");

    let search = crate::titledb::Title::search(&query)
        .await?;

    Ok(Json(search))
}

fn get_content_type(path: &str) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .as_ref()
        .to_string()
}
#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub struct SearchQuery {
    #[serde(rename = "q")]
    pub query: String,
    #[serde(rename = "limit")]
    pub limit: Option<usize>,
}

pub async fn search_base_game(
    query: Query<SearchQuery>,
) -> AlumRes<Json<Vec<crate::titledb::Title>>> {
    let query = query.0;
    // let sq = query.query.clone();

    tracing::debug!(?query, "Searching for base game with query");

    let search = crate::titledb::Metaview::search_base_game(&query)
        .await?
        .to_vec();

    Ok(Json(search))
}

pub async fn search_titles(query: Query<SearchQuery>) -> AlumRes<Json<Vec<crate::titledb::Title>>> {
    let query = query.0;
    // let sq = query.query.clone();

    tracing::debug!(?query, "Searching for title with query");

    let search = crate::titledb::Metaview::search_all(&query).await?.to_vec();

    Ok(Json(search))
}

async fn serve_static_file(path: String) -> impl IntoResponse {
    match tokio::fs::read(&path).await {
        Ok(contents) => {
            let content_type = get_content_type(&path);

            // Determine appropriate cache duration based on file type
            let cache_control = if path.ends_with(".html") {
                // Don't cache HTML files as aggressively
                "public, max-age=3600" // 1 hour
            } else if path.contains("/static/")
                && (path.contains(".js") || path.contains(".css"))
                && path.contains(".")
            {
                // For hashed static assets (typically contain hash in filename)
                "public, max-age=31536000, immutable" // 1 year
            } else {
                // Default for other static assets
                "public, max-age=86400" // 1 day
            };

            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, content_type.as_str()),
                    (header::CACHE_CONTROL, cache_control),
                ],
                contents,
            )
                .into_response()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

pub fn static_router() -> Router {
    Router::new()
        .route(
            "/static/{*path}",
            get(|path: HttpPath<String>| async move {
                serve_static_file(format!("alu-panel/dist/static/{}", path.0)).await
            }),
        )
        .route(
            "/favicon.ico",
            get(|| async { serve_static_file("alu-panel/dist/favicon.ico".to_string()).await }),
        )
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
}

pub fn create_router() -> Router {
    Router::new()
        // .route("/", get(list_files))
        // add static router
        .merge(static_router())
        .route("/api/index", get(list_files))
        .route("/api/get_game/{title_id}", get(download_file))
        .route("/api/title_meta/{title_id}", get(title_meta))
        .route(
            "/api/title_meta/{title_id}/base_game",
            get(title_meta_base_game),
        )
        .route("/api/title_meta/{title_id}/download_ids", get(get_download_ids))
        .route("/api/grouped/{title_id}", get(list_grouped_by_titleid))
        .route("/api/base_games", get(list_base_games))
        .route("/api/base_games/search", get(search_base_game))
        .route("/api/titledb/search", get(search_titledb))
        .route("/api/search", get(search_titles))
        // web ui
        .nest("/admin", admin_router())
        // user things
        .nest("/api", user_router())
        // .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
        .fallback(|| async {
            match std::fs::read_to_string("alu-panel/dist/index.html") {
                Ok(contents) => Html(contents).into_response(),
                Err(_) => StatusCode::NOT_FOUND.into_response(),
            }
        })
        .layer(middleware::from_fn(basic_auth_if_public))
}
