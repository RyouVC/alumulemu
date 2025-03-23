// use crate::db::scan_games_path;
use crate::db::DB;
use crate::db::NspMetadata;
use crate::games_dir;
use crate::index::{Index, TinfoilResponse};
use tower_http::services::ServeDir;

use crate::titledb::GameFileDataNaive;
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordVerifier},
};
use axum::middleware::{self, Next};
use axum::{
    BoxError, Json, Router,
    body::Body,
    extract::{Path as HttpPath, Query},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use http::Request;
use serde::{Deserialize, Serialize};
use tokio_util::bytes;
use tokio_util::io::ReaderStream;
// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub struct ErrorResponse {
//     pub failure: String,
// }
#[derive(Debug, Serialize, Deserialize)]
struct User {
    username: String,
    password_hash: String,
}

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
    let walker = jwalk::WalkDir::new(path);
    let paths = walker.into_iter();
    let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());

    for entry in paths {
        let path = entry.map_err(|e| Error::Error(color_eyre::eyre::eyre!(e.to_string())))?;
        let filename = path.file_name().to_string_lossy().into_owned();

        if !filename.ends_with(".nsp")
            && !filename.ends_with(".xci")
            && !filename.ends_with(".nsz")
            && !filename.ends_with(".ncz")
            && !filename.ends_with(".xcz")
        {
            continue;
        }

        // Skip if we already have metadata for this file
        if all_metadata
            .iter()
            .any(|m| m.path == path.path().to_str().unwrap())
        {
            continue;
        }

        let game_data = GameFileDataNaive::get(&path.path(), &all_metadata).await?;
        let title_id = game_data
            .title_id
            .clone()
            .unwrap_or_else(|| "00000000AAAA0000".to_string());

        // Get the title name from metadata if available
        let title_name = all_metadata
            .iter()
            .find(|m| m.path == path.path().to_str().unwrap())
            .and_then(|m| m.title_name.clone())
            .unwrap_or_else(|| game_data.name.trim().trim_end_matches(".nsp").to_string());

        let metadata = NspMetadata {
            path: path.path().to_str().unwrap().to_string(),
            title_id: title_id.clone(),
            version: game_data.version.unwrap_or_else(|| "v0".to_string()),
            title_name: Some(title_name),
        };

        if let Err(e) = metadata.save().await {
            tracing::warn!(
                "Failed to save metadata for {}: {}",
                path.path().display(),
                e
            );
        }
    }

    Ok(())
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    let response = TinfoilResponse::Failure(format!("Server error: {}", error));
    Json(response)
}

#[tracing::instrument]
pub async fn generate_index_from_metadata() -> color_eyre::eyre::Result<Index> {
    let mut idx = Index::default();
    let all_metadata = NspMetadata::get_all().await.unwrap_or_else(|_| Vec::new());

    for metadata in all_metadata {
        let path = std::path::Path::new(&metadata.path);
        let filename = path.file_name().unwrap().to_string_lossy().into_owned();
        let extension = path.extension().unwrap_or_default().to_str().unwrap();

        let formatted_name = format!(
            "{} [{}][{}].{}",
            metadata
                .title_name
                .unwrap_or_else(|| filename.trim().trim_end_matches(extension).to_string()),
            metadata.title_id,
            metadata.version,
            extension
        );

        idx.add_file(
            path,
            "/api/get_game",
            &formatted_name,
            Some(&metadata.title_id),
        );
    }

    Ok(idx)
}

#[derive(Deserialize)]
struct ListFilesQuery {
    update: Option<bool>,
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

    // Check if the title_id_param contains a version specifier (_v)
    let (title_id, version_filter) = if let Some(pos) = title_id_param.find("_v") {
        let (base, version_part) = title_id_param.split_at(pos);
        let version = version_part.trim_start_matches("_v");
        (base.to_string(), Some(version.to_string()))
    } else {
        (title_id_param, None)
    };

    // Debug print all title IDs
    for metadata in all_metadata.iter() {
        tracing::debug!(
            "DB title ID: {}, version: {}",
            metadata.title_id,
            metadata.version
        );
    }

    let file_path = all_metadata
        .iter()
        .find(|m| {
            let title_id_matches = m.title_id == title_id;

            // If we have a version filter, check if the version matches as well
            let version_matches = if let Some(ref v) = version_filter {
                // Remove 'v' prefix if present for comparison
                let normalized_version = m.version.trim_start_matches('v');
                let normalized_filter = v.trim_start_matches('v');

                normalized_version == normalized_filter
            } else {
                true // No version filter, so this is automatically a match
            };

            tracing::debug!(
                "Comparing {} with {}, version {} with filter {:?}: matches={}/{}",
                m.title_id,
                title_id,
                m.version,
                version_filter,
                title_id_matches,
                version_matches
            );

            title_id_matches && version_matches
        })
        .map(|m| m.path.clone())
        .ok_or_else(|| {
            let filter_info = if let Some(v) = &version_filter {
                format!("title_id {} with version {}", title_id, v)
            } else {
                format!("title_id {}", title_id)
            };
            tracing::error!("No matching {} found", filter_info);
            StatusCode::NOT_FOUND
        })?;

    tracing::debug!("Found file path: {}", file_path);

    let file = match tokio::fs::File::open(&file_path).await {
        Ok(file) => file,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    let filename = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("game.nsp");

    let response = Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename),
        )
        .body(body)
        .unwrap();

    Ok(response)
}

async fn basic_auth(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // First check if there are any users in the database
    let users: Vec<User> = DB
        .select("user")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If there are no users, bypass authentication and add a warning header
    if users.is_empty() {
        tracing::warn!(
            "No users found in database. Authentication bypassed! Please create at least 1 admin user"
        );
        let mut response = next.run(req).await;
        response.headers_mut().insert(
            "X-Auth-Warning",
            "No users found in database. Authentication bypassed."
                .parse()
                .unwrap(),
        );
        return Ok(response);
    }

    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Basic ") {
                let credentials_b64 = auth_str.trim_start_matches("Basic ").trim();
                if let Ok(decoded) = BASE64.decode(credentials_b64) {
                    if let Ok(decoded_str) = String::from_utf8(decoded) {
                        let parts: Vec<&str> = decoded_str.splitn(2, ':').collect();
                        if parts.len() == 2 {
                            let username = parts[0];
                            let password = parts[1];

                            let user: Option<User> = DB
                                .select(("user", username))
                                .await
                                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                            if let Some(user) = user {
                                let parsed_hash = PasswordHash::new(&user.password_hash)
                                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

                                if Argon2::default()
                                    .verify_password(password.as_bytes(), &parsed_hash)
                                    .is_ok()
                                {
                                    return Ok(next.run(req).await);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let mut response = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    response.headers_mut().insert(
        axum::http::header::WWW_AUTHENTICATE,
        axum::http::header::HeaderValue::from_static("Basic"),
    );
    Ok(response)
}

pub async fn create_user(username: &str, password: &str) -> Result<(), Box<dyn std::error::Error>> {
    use argon2::password_hash::{PasswordHasher, SaltString, rand_core::OsRng};

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string();

    let user = User {
        username: username.to_string(),
        password_hash,
    };

    let created: Option<User> = DB.create(("user", username)).content(user).await?;

    Ok(())
}

async fn serve_index() -> Html<String> {
    Html(std::fs::read_to_string("webui/index.html").unwrap())
}

async fn serve_users() -> Html<String> {
    Html(std::fs::read_to_string("webui/users.html").unwrap())
}

async fn serve_games() -> Html<String> {
    Html(std::fs::read_to_string("webui/games.html").unwrap())
}

async fn serve_js(HttpPath(file): HttpPath<String>) -> impl IntoResponse {
    let content = std::fs::read_to_string(format!("webui/js/{}", file)).unwrap();
    Response::builder()
        .header("Content-Type", "application/javascript")
        .body(content)
        .unwrap()
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
}

async fn create_user_handler(
    Json(payload): Json<CreateUserRequest>,
) -> Result<StatusCode, StatusCode> {
    match create_user(&payload.username, &payload.password).await {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Serialize)]
struct UserInfo {
    username: String,
}

async fn list_users() -> Result<Json<Vec<UserInfo>>, StatusCode> {
    let users: Vec<User> = DB
        .select("user")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(
        users
            .into_iter()
            .map(|u| UserInfo {
                username: u.username,
            })
            .collect(),
    ))
}

async fn delete_user(HttpPath(username): HttpPath<String>) -> Result<StatusCode, StatusCode> {
    let _: Option<User> = DB
        .delete(("user", username))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn rescan_games() -> AlumRes<Json<TinfoilResponse>> {
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
        .layer(middleware::from_fn(basic_auth))
}

pub fn api_router() -> Router {
    Router::new()
        .route("/users", get(list_users))
        .route("/users", post(create_user_handler))
        .route("/users/{username}", delete(delete_user))
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
    // .layer(middleware::from_fn(basic_auth))
}

// todo: use mime_types crate
fn get_content_type(path: &str) -> &'static str {
    match path.split('.').last() {
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        Some("otf") => "font/otf",
        _ => "application/octet-stream",
    }
}

async fn serve_static_file(path: String) -> impl IntoResponse {
    match std::fs::read(&path) {
        Ok(contents) => {
            let content_type = get_content_type(&path);
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, content_type)],
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
        .nest("/api", api_router())
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
    // .layer(middleware::from_fn(basic_auth))
}

#[cfg(test)]
mod tests {
    use crate::db::init_database;

    use super::*;

    #[tokio::test]
    async fn test_create_user() {
        init_database()
            .await
            .expect("Failed to initialize database");
        let username = "testuser";
        let password = "testpassword";

        let result = create_user(username, password).await;
        assert!(result.is_ok(), "Failed to create user: {:?}", result);

        let user: Option<User> = DB
            .select(("user", username))
            .await
            .expect("Failed to fetch user");

        assert!(user.is_some(), "User was not found in database");

        let user = user.unwrap();
        assert_eq!(user.username, username);

        let parsed_hash = PasswordHash::new(&user.password_hash).expect("Failed to parse hash");

        let verification = Argon2::default().verify_password(password.as_bytes(), &parsed_hash);

        assert!(verification.is_ok(), "Password verification failed");

        // delete the user at the end
        // let _: Option<User> = DB
        //     .delete(("user", username))
        //     .await
        //     .expect("Failed to delete test user");
    }
}
