use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path as HttpPath, Request},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::{db::DB, index::TinfoilResponse};

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    password_hash: String,
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

    let _created: Option<User> = DB.create(("user", username)).content(user).await?;

    Ok(())
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
}

pub async fn create_user_handler(
    Json(payload): Json<CreateUserRequest>,
) -> Result<StatusCode, StatusCode> {
    match create_user(&payload.username, &payload.password).await {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

#[derive(Serialize)]
pub struct UserInfo {
    username: String,
}

pub async fn list_users() -> Result<Json<Vec<UserInfo>>, StatusCode> {
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

pub async fn delete_user(HttpPath(username): HttpPath<String>) -> Result<StatusCode, StatusCode> {
    let _: Option<User> = DB
        .delete(("user", username))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn basic_auth(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
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

pub fn user_router() -> Router {
    Router::new()
        .route("/users", get(list_users))
        .route("/users", post(create_user_handler))
        .route("/users/{username}", delete(delete_user))
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
    // .layer(middleware::from_fn(basic_auth))
}
