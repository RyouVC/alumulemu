use crate::backend::user::basic_auth;
use crate::backend::user::user_router;
use crate::db::NspMetadata;
use crate::games_dir;
use crate::index::{Index, TinfoilResponse};

use crate::titledb::GameFileDataNaive;
use crate::util::format_game_name;
use axum::middleware::{self};
use axum::{
    BoxError, Json, Router,
    extract::Path as HttpPath,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};

use tokio_util::io::ReaderStream;
// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub struct ErrorResponse {
//     pub failure: String,
// }

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("0:?")]
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

    // Get all existing metadata
    let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());

    // Track which files we've seen during this scan
    let mut found_paths = std::collections::HashSet::new();

    // Walk the directory and process each file
    let walker = jwalk::WalkDir::new(path);
    let paths = walker.into_iter();

    // First pass: scan filesystem and add/update metadata
    for entry in paths {
        let path = entry.map_err(|e| Error::Error(color_eyre::eyre::eyre!(e.to_string())))?;
        let file_path = path.path();
        let file_path_str = file_path.to_string_lossy().to_string();
        let filename = path.file_name().to_string_lossy().into_owned();

        // Only process supported game files
        if !filename.ends_with(".nsp")
            && !filename.ends_with(".xci")
            && !filename.ends_with(".nsz")
            && !filename.ends_with(".ncz")
            && !filename.ends_with(".xcz")
        {
            continue;
        }

        // Add this path to our found set
        found_paths.insert(file_path_str.clone());

        // Process file and update/add metadata
        // Always process the file - no caching
        tracing::debug!("Processing file: {}", file_path_str);

        let game_data = match GameFileDataNaive::get(&file_path, &all_metadata).await {
            Ok(data) => data,
            Err(e) => {
                tracing::warn!("Failed to get game data for {}: {}", file_path_str, e);
                continue;
            }
        };

        let title_id = game_data
            .title_id
            .clone()
            .unwrap_or_else(|| "00000000AAAA0000".to_string());

        // Get the title name from metadata if available
        let title_name = all_metadata
            .iter()
            .find(|m| m.path == file_path_str)
            .and_then(|m| m.title_name.clone())
            .unwrap_or_else(|| game_data.name.trim().trim_end_matches(".nsp").to_string());

        let metadata = NspMetadata {
            path: file_path_str.clone(),
            title_id: title_id.clone(),
            version: game_data.version.unwrap_or_else(|| "v0".to_string()),
            title_name: Some(title_name),
        };

        if let Err(e) = metadata.save().await {
            tracing::warn!("Failed to save metadata for {}: {}", file_path_str, e);
        } else {
            tracing::debug!("Saved metadata for {}", file_path_str);
        }
    }

    // Second pass: delete metadata for files that no longer exist
    let mut deleted_count = 0;
    for metadata in all_metadata {
        if !found_paths.contains(&metadata.path) {
            tracing::info!("Removing metadata for non-existent file: {}", metadata.path);
            if let Err(e) = metadata.delete().await {
                tracing::error!("Failed to delete metadata for {}: {}", metadata.path, e);
            } else {
                deleted_count += 1;
            }
        }
    }

    tracing::info!(
        "Metadata rescan complete. Found {} files, removed {} stale entries.",
        found_paths.len(),
        deleted_count
    );

    Ok(())
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
    if title_id_param.contains("..") {
        return Err(StatusCode::BAD_REQUEST);
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

pub async fn rescan_games() -> AlumRes<Json<TinfoilResponse>> {
    tracing::info!("Rescanning games directory");
    update_metadata_from_filesystem(&games_dir()).await?;
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

// todo: use mime_types crate
fn get_content_type(path: &str) -> String {
    mime_guess::from_path(path)
        .first_or_octet_stream()
        .as_ref()
        .to_string()
}

async fn serve_static_file(path: String) -> impl IntoResponse {
    match std::fs::read(&path) {
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
        // web ui
        .nest("/admin", admin_router())
        // user things
        .nest("/api", user_router())
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
    // .layer(middleware::from_fn(basic_auth))
}
