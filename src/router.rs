use std::path::Path;

use crate::backend::router::create_router as create_backend_router;
use crate::db::NspMetadata;
use crate::index::{Index, TinfoilResponse};
use crate::titledb::GameFileDataNaive;
use crate::util::format_download_id;
use crate::util::format_game_name;
use axum::{
    Json,
    extract::Request,
    http::{StatusCode, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Redirect, Response},
};
use std::time::Instant;

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

pub type AlumRes<T> = Result<T, Error>;

#[derive(serde::Deserialize, serde::Serialize, Debug, Default)]
pub struct RescanOptions {
    #[serde(default)]
    pub rescan: bool,
}

#[tracing::instrument]
pub async fn update_metadata_from_filesystem(
    path: &str,
    options: RescanOptions,
) -> color_eyre::eyre::Result<()> {
    tracing::info!("Starting full metadata rescan of {}", path);

    let rescan = options.rescan;
    tracing::debug!("Rescan option: {}", rescan);

    // Get all existing metadata
    let all_metadata = match NspMetadata::get_all().await {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!("Failed to get existing metadata: {}", e);
            return Err(color_eyre::eyre::eyre!(
                "Failed to get existing metadata: {}",
                e
            ));
        }
    };

    // Create a lookup map for faster metadata lookups
    let metadata_map: std::collections::HashMap<String, &NspMetadata> =
        all_metadata.iter().map(|m| (m.path.clone(), m)).collect();

    // Define valid extensions once
    const VALID_EXTENSIONS: [&str; 5] = ["nsp", "xci", "nsz", "ncz", "xcz"];

    // Track which files we've seen during this scan
    let mut found_paths = std::collections::HashSet::new();

    // Track statistics
    let mut total_files = 0;
    let mut processed_files = 0;
    let mut skipped_files = 0;
    let mut failed_files = 0;

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
        total_files += 1;
        let path = match entry {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to access file during scan: {}", e);
                failed_files += 1;
                continue; // Skip but don't abort entire operation
            }
        };

        let file_path = path.path();

        // Extract extension early for filtering
        if let Some(ext) = file_path.extension().and_then(|e| e.to_str()) {
            if !VALID_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                skipped_files += 1;
                continue;
            }
        } else {
            skipped_files += 1;
            continue;
        }

        let file_path_str = file_path.to_string_lossy().to_string();
        found_paths.insert(file_path_str.clone());

        // Check if we need to update this file
        // Force rescan if the rescan option is true, otherwise only scan if no metadata exists
        let needs_update = rescan || !metadata_map.contains_key(&file_path_str);

        if needs_update {
            // Use the dedicated scan_file function instead of duplicating code
            match scan_file(&file_path, rescan).await {
                Ok(_) => {
                    processed_files += 1;
                    tracing::debug!("Successfully processed file: {}", file_path_str);
                }
                Err(e) => {
                    failed_files += 1;
                    tracing::error!("Failed to scan file {}: {}", file_path_str, e);
                }
            }
        } else {
            skipped_files += 1;
            tracing::trace!("Skipped file (already up to date): {}", file_path_str);
        }
    }

    // Delete metadata for files that no longer exist
    let mut deleted_count = 0;
    let mut delete_failed_count = 0;

    for metadata in all_metadata
        .iter()
        .filter(|m| !found_paths.contains(&m.path))
    {
        tracing::info!("Removing metadata for non-existent file: {}", metadata.path);
        match metadata.delete().await {
            Ok(_) => {
                deleted_count += 1;
                tracing::debug!("Successfully deleted metadata for: {}", metadata.path);
            }
            Err(e) => {
                delete_failed_count += 1;
                tracing::error!("Failed to delete metadata for {}: {}", metadata.path, e);
            }
        }
    }

    tracing::info!(
        "Metadata rescan complete. Results: total={}, processed={}, skipped={}, failed={}, deleted={}, delete_failed={}",
        total_files,
        processed_files,
        skipped_files,
        failed_files,
        deleted_count,
        delete_failed_count
    );

    if failed_files > 0 || delete_failed_count > 0 {
        tracing::warn!("Some operations failed during metadata rescan. Check logs for details.");
    }

    Ok(())
}

pub async fn scan_file(path: &Path, rescan_files: bool) -> color_eyre::Result<()> {
    tracing::info!("Scanning file: {}", path.display());
    // Get all existing metadata with proper error handling
    let all_metadata = match NspMetadata::get_all().await {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::warn!("Failed to get existing metadata: {}", e);
            Vec::new() // Continue with empty metadata instead of crashing
        }
    };

    let file_path_str = path.to_string_lossy().to_string();
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
                        let name = game_data.name.trim();
                        name.trim_end_matches(".nsp").to_string()
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
                    tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS)).await;
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
    use tokio::sync::mpsc;

    tracing::info!("Starting filesystem watcher for: {}", path);

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
            _ => continue,
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

                // Get all existing metadata with better error handling
                let all_metadata = match NspMetadata::get_all().await {
                    Ok(metadata) => std::sync::Arc::new(metadata),
                    Err(e) => {
                        tracing::error!("Failed to get metadata: {}", e);
                        std::sync::Arc::new(Vec::new()) // Continue with empty metadata
                    }
                };

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

                // Find and delete the metadata for this file, with proper error handling
                match NspMetadata::get_all().await {
                    Ok(all_metadata) => {
                        if let Some(metadata) = all_metadata.iter().find(|m| m.path == path_str) {
                            if let Err(e) = metadata.delete().await {
                                tracing::error!(
                                    "Failed to delete metadata for {}: {}",
                                    path_str,
                                    e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to get metadata when removing file {}: {}",
                            path_str,
                            e
                        );
                    }
                }
            }
            _ => {} // Ignore other event types
        }
    }
}

#[tracing::instrument]
pub async fn index_from_existing_data() -> color_eyre::eyre::Result<Index> {
    let mut idx = Index::default();

    // Get all metadata with proper error handling
    let all_metadata = match NspMetadata::get_all().await {
        Ok(metadata) => metadata,
        Err(e) => {
            tracing::error!("Failed to get metadata for index generation: {}", e);
            return Err(color_eyre::eyre::eyre!("Failed to generate index: {}", e));
        }
    };

    for metadata in all_metadata {
        let path = std::path::Path::new(&metadata.path);

        // Handle potential missing filename more gracefully
        let filename = match path.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            _ => {
                tracing::warn!("Skipping entry with invalid path: {}", metadata.path);
                continue;
            }
        };

        // Handle potential missing extension more gracefully
        let extension = path
            .extension()
            .and_then(|ext| ext.to_str())
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

// Middleware to handle trailing slashes
async fn normalize_trailing_slash(req: Request, next: Next) -> impl IntoResponse {
    let uri = req.uri().clone();
    let path = uri.path();

    // If path has a trailing slash and is not the root path
    if path.len() > 1 && path.ends_with('/') {
        // Create new URI without the trailing slash
        let new_path = path.trim_end_matches('/');
        let new_path_and_query = match uri.query() {
            Some(query) => format!("{}?{}", new_path, query),
            _ => new_path.to_string(),
        };

        // Try to convert to a valid URI
        if let Ok(new_uri) = Uri::try_from(new_path_and_query) {
            // Redirect to the normalized path
            tracing::debug!("Redirecting from {} to {}", path, new_path);
            return Redirect::permanent(&new_uri.to_string()).into_response();
        }
    }

    next.run(req).await
}

pub const TINFOIL_HEADERS: [&str; 3] = ["uid", "hauth", "uauth"];

// Middleware to redirect Tinfoil clients to /api/tinfoil when accessing root
async fn tinfoil_redirect(req: Request, next: Next) -> impl IntoResponse {
    // Only apply this middleware to the root path
    if req.uri().path() == "/" {
        // Check if all Tinfoil headers are present
        let is_tinfoil_client = TINFOIL_HEADERS
            .iter()
            .all(|&header| req.headers().contains_key(header));

        if is_tinfoil_client {
            tracing::debug!("Tinfoil client detected at root path, redirecting to /api/tinfoil");
            return Redirect::permanent("/api/tinfoil").into_response();
        }
    }

    next.run(req).await
}

// Middleware function to log requests with better error handling
async fn log_request(req: Request, next: Next) -> impl IntoResponse {
    let path = req.uri().path().to_owned();
    let method = req.method().clone();

    // Extract and format request headers with safer handling of binary values
    let headers = req
        .headers()
        .iter()
        .map(|(name, value)| {
            let value_str = value.to_str().unwrap_or("[binary]");
            format!("{}: {}", name, value_str)
        })
        .collect::<Vec<String>>()
        .join(", ");

    let start = Instant::now();
    tracing::trace!("Request started: {} {}\nHeaders: {}", method, path, headers);

    let response = next.run(req).await;
    let duration = start.elapsed();

    tracing::trace!("Request completed: {} {} in {:?}", method, path, duration);
    response
}

pub fn create_router() -> axum::Router {
    let router = create_backend_router();

    // Apply middlewares - normalize paths first, then log requests
    router
        .layer(middleware::from_fn(log_request))
        .layer(middleware::from_fn(normalize_trailing_slash))
        .layer(middleware::from_fn(tinfoil_redirect))
}
