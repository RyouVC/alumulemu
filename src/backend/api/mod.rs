use crate::{
    db::NspMetadata,
    index::{Index, TinfoilResponse},
    router::{AlumRes, generate_index_from_metadata},
    util::format_game_name,
};
use axum::{
    Json, Router,
    extract::Path,
    response::{IntoResponse, Response},
    routing::get,
};
use http::{StatusCode, header};
use tokio_util::io::ReaderStream;

use super::user::user_router;

pub mod downloader;
pub mod metadata;

pub async fn tinfoil_index() -> AlumRes<Json<Index>> {
    let games = generate_index_from_metadata().await?;
    // tracing::trace!("Games retrieved: {:?}", games);
    Ok(Json(games))
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

pub fn api_router() -> Router {
    Router::new()
        .nest("/users", user_router())
        .nest("/downloads", downloader::downloader_api())
        .merge(metadata::metadata_api()) // Use merge to maintain original paths
        .route("/tinfoil", get(tinfoil_index))
        .route("/get_game/{download_id}", get(download_file))
}
