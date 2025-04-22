use crate::{
    backend::kv_config::{KvOptExt, Motd}, // Add Motd import
    db::NspMetadata,
    index::{Index, TinfoilResponse},
    router::{AlumRes, index_from_existing_data},
    util::format_game_name,
};
use axum::{
    Json, Router,
    extract::Path,
    response::{IntoResponse, Response},
    routing::get,
};
use http::{StatusCode, header};
use once_cell::sync::Lazy;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio_util::io::ReaderStream;

use super::{kv_config::ExtraSourcesConfig, user::user_router};

pub mod downloader;
pub mod metadata;

// Default cache lifetime in seconds (5 minutes)
const CACHE_LIFETIME_SECONDS: u64 = 300;

// Structure to hold cached index data with timestamp
struct IndexCache {
    data: Option<Index>,
    last_updated: Option<Instant>,
}

// Create a global cache using lazy_static
static INDEX_CACHE: Lazy<Arc<Mutex<IndexCache>>> = Lazy::new(|| {
    Arc::new(Mutex::new(IndexCache {
        data: None,
        last_updated: None,
    }))
});

/// Generates the Tinfoil index data, merging base data, extras, and Motd.
async fn generate_tinfoil_index_data() -> AlumRes<Index> {
    let mut games = index_from_existing_data().await?;

    // Now, merge it with the extras if possible
    if let Ok(extras) = Index::get_extra_indexes().await {
        extras.iter().for_each(|e_idx| {
            games.merge_file_index(e_idx.clone());
            games.merge_titledb(e_idx.clone());
            tracing::trace!("Merged extra index: {:?}", e_idx);
        });
    }

    // Check for Motd and apply if set
    // Check for Motd and apply if set and enabled
    games.success = match Motd::get().await {
        // Only assign the message if Motd is fetched successfully, enabled, and has a message.
        Ok(Some(motd)) if motd.enabled && motd.message.is_some() => motd.message,
        // In all other cases (error, no Motd, disabled, or empty message), assign None.
        _ => None,
    };

    games.locations = ExtraSourcesConfig::get()
        .await?
        .map(|config| config.sources) // Extract the sources Vec if Some(config)
        .unwrap_or_default(); // Use an empty Vec if None

    Ok(games)
}

#[axum::debug_handler]
pub async fn tinfoil_index() -> AlumRes<Json<Index>> {
    // Try to get cached version first
    {
        let cache = INDEX_CACHE.lock().unwrap();
        if let (Some(cached_data), Some(timestamp)) = (&cache.data, cache.last_updated) {
            // Check if cache is still valid
            if timestamp.elapsed() < Duration::from_secs(CACHE_LIFETIME_SECONDS) {
                tracing::debug!(
                    "Serving tinfoil index from cache (age: {}s)",
                    timestamp.elapsed().as_secs()
                );
                // Return a clone of the cached data
                return Ok(Json(cached_data.clone()));
            } else {
                tracing::debug!(
                    "Cache expired after {}s (max: {}s), regenerating",
                    timestamp.elapsed().as_secs(),
                    CACHE_LIFETIME_SECONDS
                );
                // Cache expired, proceed to regenerate below
            }
        } else {
            tracing::debug!("No cached index available, generating new index");
            // No cache, proceed to regenerate below
        }
        // Lock is dropped here
    }

    // If we got here, cache was missed or expired, regenerate the index
    tracing::debug!("Generating new tinfoil index data");
    let games = generate_tinfoil_index_data().await?;

    // Update the cache with new data
    {
        let mut cache = INDEX_CACHE.lock().unwrap();
        cache.data = Some(games.clone()); // Clone data for the cache
        cache.last_updated = Some(Instant::now());
        tracing::info!("Updated tinfoil index cache");
    } // Lock is dropped here

    Ok(Json(games)) // Return the newly generated data
}

// Function to manually invalidate the cache if needed
pub fn invalidate_index_cache() {
    let mut cache = INDEX_CACHE.lock().unwrap();
    cache.data = None;
    cache.last_updated = None;
    tracing::info!("Tinfoil index cache invalidated");
}

pub async fn download_file(
    Path(download_id_param): Path<String>,
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

    let all_metadata = NspMetadata::get_from_download_id(&download_id_param)
        .await
        .map_err(|err| {
            tracing::error!(
                "Failed to retrieve metadata for download ID {}: {}",
                download_id_param,
                err
            );
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

    // Open the file with better error handling
    let file = match tokio::fs::File::open(file_path).await {
        Ok(file) => file,
        Err(e) => {
            tracing::error!("Failed to open file at {}: {}", file_path, e);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    // Get the raw filename and extension for formatting with better error handling
    let path = std::path::Path::new(file_path);

    let raw_filename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| {
            tracing::warn!("Could not extract filename from path: {}", file_path);
            "unknown".to_string()
        });

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_else(|| {
            tracing::warn!("Could not extract extension from path: {}", file_path);
            "nsp"
        });

    // Create a nicely formatted filename for the download
    let formatted_filename = format_game_name(&metadata_entry, &raw_filename, extension);

    // Sanitize the filename to ensure it's safe for Content-Disposition
    // Replace any characters that might cause issues in headers
    let safe_filename = formatted_filename.replace(['"', '\\', '\n', '\r', '\t'], "_");

    tracing::info!("Serving download with filename: {}", safe_filename);

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    // Build the response with proper error handling
    match Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe_filename}\""),
        )
        .body(body)
    {
        Ok(response) => {
            tracing::debug!("Response headers: {:?}", response.headers());
            Ok(response)
        }
        Err(e) => {
            tracing::error!("Failed to build response: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Function to create the main API router
pub fn api_router() -> Router {
    // User router requires admin access
    let user_routes = Router::new().nest("/users", user_router());

    // Basic routes that all authenticated users can access (viewer level)
    let api_routes = Router::new()
        .nest("/downloads", downloader::downloader_api())
        .merge(metadata::metadata_api()) // Use merge to maintain original paths
        .route("/tinfoil", get(tinfoil_index))
        .route("/get_game/{download_id}", get(download_file));

    // Combine the routes
    Router::new()
        .merge(user_routes)
        .merge(api_routes)
        // Require viewer authentication for all API routes
        .layer(axum::middleware::from_fn(
            crate::backend::user::auth_optional_viewer,
        ))
}
