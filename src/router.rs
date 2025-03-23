use crate::backend::user::basic_auth_if_public;
use crate::backend::user::user_router;
use crate::db::NspMetadata;
use crate::db::create_precomputed_metaview;
use crate::games_dir;
use crate::index::{Index, TinfoilResponse};

use crate::titledb::GameFileDataNaive;
use crate::util::format_game_name;
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
#[tracing::instrument]
pub async fn update_metadata_from_filesystem(path: &str) -> color_eyre::eyre::Result<()> {
    tracing::info!("Starting full metadata rescan of {}", path);

    // Get all existing metadata and wrap in Arc for thread-safe sharing
    let all_metadata =
        std::sync::Arc::new(NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new()));

    // Create a lookup map for faster metadata lookups
    let metadata_map: std::collections::HashMap<String, &NspMetadata> =
        all_metadata.iter().map(|m| (m.path.clone(), m)).collect();

    // Define valid extensions once
    const VALID_EXTENSIONS: [&str; 5] = ["nsp", "xci", "nsz", "ncz", "xcz"];

    // Track which files we've seen during this scan
    let mut found_paths = std::collections::HashSet::new();

    // Prepare batches for DB operations
    let mut metadata_to_save = Vec::with_capacity(100);

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

    // Process files in parallel with bounded concurrency
    let mut tasks = Vec::new();
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(8)); // Limit to 8 concurrent operations

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

        // Check if we already have metadata that's recent enough (could add timestamp checks here)
        let needs_update = !metadata_map.contains_key(&file_path_str);

        if needs_update {
            let all_metadata_clone = all_metadata.clone();
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let task = tokio::spawn(async move {
                let _permit = permit; // Keep permit alive for the duration of this task

                tracing::debug!("Processing file: {}", file_path_str);
                match GameFileDataNaive::get(&file_path, &all_metadata_clone).await {
                    Ok(game_data) => {
                        let title_id = game_data
                            .title_id
                            .unwrap_or_else(|| "00000000AAAA0000".to_string());

                        // Get the title name from metadata or filename
                        let title_name = all_metadata_clone
                            .iter()
                            .find(|m| m.path == file_path_str)
                            .and_then(|m| m.title_name.clone())
                            .unwrap_or_else(|| {
                                game_data.name.trim().trim_end_matches(".nsp").to_string()
                            });

                        Some(NspMetadata {
                            path: file_path_str,
                            title_id,
                            version: game_data.version.unwrap_or_else(|| "v0".to_string()),
                            title_name: Some(title_name),
                        })
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get game data: {}", e);
                        None
                    }
                }
            });
            tasks.push(task);

            // Process in batches to avoid using too much memory
            if tasks.len() >= 20 {
                metadata_to_save.extend(
                    (futures::future::join_all(tasks).await)
                        .into_iter()
                        .flatten()
                        .flatten(),
                );
                tasks = Vec::new();

                // Save batch to DB
                if !metadata_to_save.is_empty() {
                    for metadata in metadata_to_save.drain(..) {
                        if let Err(e) = metadata.save().await {
                            tracing::warn!("Failed to save metadata: {}", e);
                        }
                    }
                }
            }
        }
    }

    // Process remaining tasks
    // Process remaining tasks and save their results directly
    if !tasks.is_empty() {
        for metadata in (futures::future::join_all(tasks).await)
            .into_iter()
            .flatten()
            .flatten()
        {
            if let Err(e) = metadata.save().await {
                tracing::warn!("Failed to save metadata: {}", e);
            }
        }
    }

    // Delete metadata for files that no longer exist
    let delete_futures: Vec<_> = all_metadata
        .iter()
        .filter(|metadata| !found_paths.contains(&metadata.path))
        .map(|metadata| async move {
            tracing::info!("Removing metadata for non-existent file: {}", metadata.path);
            if let Err(e) = metadata.delete().await {
                tracing::error!("Failed to delete metadata for {}: {}", metadata.path, e);
                0
            } else {
                1
            }
        })
        .collect();

    let deleted_results = futures::future::join_all(delete_futures).await;
    let deleted_count: usize = deleted_results.iter().sum();

    tracing::info!(
        "Metadata rescan complete. Found {} files, removed {} stale entries.",
        found_paths.len(),
        deleted_count
    );

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
                match GameFileDataNaive::get(event_path, &all_metadata).await {
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

                        // Save the metadata
                        let metadata = NspMetadata {
                            path: path_str,
                            title_id,
                            version: game_data.version.unwrap_or_else(|| "v0".to_string()),
                            title_name: Some(title_name),
                        };

                        if let Err(e) = metadata.save().await {
                            tracing::error!("Failed to save metadata: {}", e);
                        } else {
                            // Update the precomputed metaview if needed
                            if let Err(e) = create_precomputed_metaview().await {
                                tracing::warn!(
                                    "Failed to update metaview after file change: {}",
                                    e
                                );
                            }
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
                    } else {
                        // Update the precomputed metaview if needed
                        if let Err(e) = create_precomputed_metaview().await {
                            tracing::warn!("Failed to update metaview after file removal: {}", e);
                        }
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
    HttpPath(title_id_param): HttpPath<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Block any path traversal attempts
    if title_id_param.contains("..")
        || title_id_param.contains('/')
        || title_id_param.contains('\\')
    {
        tracing::warn!(
            "Path traversal attempt detected in title ID: {}",
            title_id_param
        );
        return Ok(Json(TinfoilResponse::Failure(
            "path traversal not allowed for this request".to_string(),
        ))
        .into_response());
    }

    tracing::debug!("Looking for title ID: {}", title_id_param);

    let all_metadata = NspMetadata::get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::debug!("Found {} metadata entries", all_metadata.len());

    // Parse parameters: Check for both version (_v) and file extension (.)
    let (base_title_id, version_filter, extension_filter) =
        if let Some(dot_pos) = title_id_param.rfind('.') {
            // We have an extension
            let (base_with_maybe_version, ext) = title_id_param.split_at(dot_pos);
            let extension = ext.trim_start_matches('.');

            // Check if we also have a version
            if let Some(v_pos) = base_with_maybe_version.find("_v") {
                let (base, version_part) = base_with_maybe_version.split_at(v_pos);
                let version = version_part.trim_start_matches("_v");
                let version_num = version.parse::<i32>().ok();

                (base.to_string(), version_num, Some(extension.to_string()))
            } else {
                // Just extension, no version
                (
                    base_with_maybe_version.to_string(),
                    None,
                    Some(extension.to_string()),
                )
            }
        } else if let Some(v_pos) = title_id_param.find("_v") {
            // Just version, no extension
            let (base, version_part) = title_id_param.split_at(v_pos);
            let version = version_part.trim_start_matches("_v");
            let version_num = version.parse::<i32>().ok();

            (base.to_string(), version_num, None)
        } else {
            // No version or extension specified
            (title_id_param, None, None)
        };

    // Debug print all title IDs
    for metadata in all_metadata.iter() {
        tracing::debug!(
            "DB title ID: {}, version: {}, path: {}",
            metadata.title_id,
            metadata.version,
            metadata.path
        );
    }

    // Filter metadata by title_id
    let matching_metadata: Vec<_> = all_metadata
        .iter()
        .filter(|m| m.title_id == base_title_id)
        .collect();

    if matching_metadata.is_empty() {
        tracing::error!("No matching title ID {} found", base_title_id);
        return Err(StatusCode::NOT_FOUND);
    }

    // Apply filters to get the file path - but now we need to keep the metadata too
    let metadata_entry = match (version_filter, extension_filter) {
        (Some(v), Some(ext)) => {
            // Both version and extension specified
            matching_metadata
                .iter()
                .find(|m| {
                    // Parse version from metadata
                    let metadata_version = m
                        .version
                        .trim_start_matches('v')
                        .parse::<i32>()
                        .unwrap_or(0);

                    // Get extension from path
                    let metadata_ext = std::path::Path::new(&m.path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");

                    metadata_version == v && metadata_ext.eq_ignore_ascii_case(&ext)
                })
                .ok_or_else(|| {
                    tracing::error!(
                        "No matching title_id {} with version {} and extension {} found",
                        base_title_id,
                        v,
                        ext
                    );
                    StatusCode::NOT_FOUND
                })?
        }
        (Some(v), None) => {
            // Only version specified
            matching_metadata
                .iter()
                .find(|m| {
                    let metadata_version = m
                        .version
                        .trim_start_matches('v')
                        .parse::<i32>()
                        .unwrap_or(0);

                    metadata_version == v
                })
                .ok_or_else(|| {
                    tracing::error!(
                        "No matching title_id {} with version {} found",
                        base_title_id,
                        v
                    );
                    StatusCode::NOT_FOUND
                })?
        }
        (None, Some(ext)) => {
            // Only extension specified - get latest version with this extension
            matching_metadata
                .iter()
                .filter(|m| {
                    let metadata_ext = std::path::Path::new(&m.path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");

                    metadata_ext.eq_ignore_ascii_case(&ext)
                })
                .max_by_key(|m| {
                    m.version
                        .trim_start_matches('v')
                        .parse::<i32>()
                        .unwrap_or(0)
                })
                .ok_or_else(|| {
                    tracing::error!(
                        "No matching title_id {} with extension {} found",
                        base_title_id,
                        ext
                    );
                    StatusCode::NOT_FOUND
                })?
        }
        (None, None) => {
            // No filters - first try to get latest version in NSP format
            let nsp_metadata: Vec<_> = matching_metadata
                .iter()
                .filter(|m| {
                    let metadata_ext = std::path::Path::new(&m.path)
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("");

                    metadata_ext.eq_ignore_ascii_case("nsp")
                })
                .collect();

            if !nsp_metadata.is_empty() {
                // We have NSP files - get the latest version
                nsp_metadata
                    .iter()
                    .max_by_key(|m| {
                        m.version
                            .trim_start_matches('v')
                            .parse::<i32>()
                            .unwrap_or(0)
                    })
                    .ok_or_else(|| {
                        // This shouldn't happen as we just checked nsp_metadata is not empty
                        tracing::error!(
                            "Failed to determine latest NSP version for title_id {}",
                            base_title_id
                        );
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?
            } else {
                // No NSP files available - fall back to any format
                tracing::info!(
                    "No NSP files found for title_id {}, falling back to any available format",
                    base_title_id
                );

                matching_metadata
                    .iter()
                    .max_by_key(|m| {
                        m.version
                            .trim_start_matches('v')
                            .parse::<i32>()
                            .unwrap_or(0)
                    })
                    .ok_or_else(|| {
                        tracing::error!(
                            "Failed to determine latest version for title_id {}",
                            base_title_id
                        );
                        StatusCode::INTERNAL_SERVER_ERROR
                    })?
            }
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
    let formatted_filename = format_game_name(metadata_entry, &raw_filename, extension);

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
            format!("attachment; filename=\"{}\"", safe_filename),
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
    // // First check if we have this game in our metadata
    // let all_metadata = NspMetadata::get_all()
    //     .await
    //     .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // let exists = all_metadata.iter().any(|m| m.title_id == title_id_param);
    // if !exists {
    //     return Err(StatusCode::NOT_FOUND);
    // }

    // Then get the title info from metaview cache
    let title = crate::titledb::Title::get_from_metaview_cache(&title_id_param)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(title).into_response())
}

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

    Ok(Json(title).into_response())
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
    let nsp_metadata = NspMetadata::get_all()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut base_games = Vec::new();
    for metadata in nsp_metadata.iter().filter(|m| m.title_id.ends_with("000")) {
        if let Ok(Some(title)) =
            crate::titledb::Title::get_from_metaview_cache(&metadata.title_id).await
        {
            base_games.push(title);
        }
    }

    Ok(Json(base_games).into_response())
}
pub async fn rescan_games() -> AlumRes<Json<TinfoilResponse>> {
    tracing::info!("Rescanning games directory");
    update_metadata_from_filesystem(&games_dir()).await?;
    tracing::info!("Games rescanned successfully");
    tracing::info!("(re)Creating precomputed metaview");
    if let Err(e) = create_precomputed_metaview().await {
        tracing::warn!("Failed to create precomputed metaview: {}", e);
    }
    Ok(Json(TinfoilResponse::MiscSuccess(
        "Games rescanned successfully".to_string(),
    )))
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

fn get_content_type(path: &str) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .as_ref()
        .to_string()
}

async fn serve_static_file(path: String) -> impl IntoResponse {
    match tokio::fs::read(&path).await {
        Ok(contents) => {
            let content_type = get_content_type(&path);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, content_type.as_str())],
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
        .route("/", get(list_files))
        // add static router
        .merge(static_router())
        .route("/api/get_game/{title_id}", get(download_file))
        .route("/api/title_meta/{title_id}", get(title_meta))
        .route(
            "/api/title_meta/{title_id}/base_game",
            get(title_meta_base_game),
        )
        .route("/api/grouped/{title_id}", get(list_grouped_by_titleid))
        .route("/api/base_games", get(list_base_games))
        // web ui
        .nest("/admin", admin_router())
        // user things
        .nest("/api", user_router())
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
    .layer(middleware::from_fn(basic_auth_if_public))
}
