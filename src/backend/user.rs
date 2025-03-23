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
    pub password: String,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
}

impl User {
    #[tracing::instrument(skip(password))]
    pub async fn create(username: &str, password: &str) -> color_eyre::Result<Self> {
        // First, ensure user doesn't already exist
        let existing: Option<User> = DB
            .query("SELECT * FROM user WHERE username = $username")
            .bind(("username", username.to_string()))
            .await?
            .take(0)?;

        if existing.is_some() {
            return Err(color_eyre::eyre::eyre!("User already exists"));
        }

        let mut req = DB
            .query(
                "CREATE user SET
        username = $username, password = crypto::argon2::generate($password), scopes = []",
            )
            .bind(("username", username.to_string()))
            .bind(("password", password.to_string()))
            .await?;

        // Create user with explicit table insertion instead of signup
        let user: Option<User> = req.take(0)?;

        tracing::trace!("Created user: {:?}", user);
        user.ok_or_else(|| color_eyre::eyre::eyre!("User creation failed"))
    }

    pub async fn get_user(username: &str) -> color_eyre::Result<Self> {
        let mut res = DB
            .query("SELECT * FROM user WHERE username = $username")
            .bind(("username", username.to_string()))
            .await?;

        let user: Option<User> = res.take(0)?;

        user.ok_or_else(|| color_eyre::eyre::eyre!("User not found"))
    }

    pub async fn delete(&self) -> color_eyre::Result<()> {
        let mut res = DB
            .query("DELETE FROM user WHERE username = $username")
            .bind(("username", self.username.to_string()))
            .await?;

        let _user: Option<User> = res.take(0)?;

        Ok(())
    }

    #[tracing::instrument]
    pub async fn login_user(username: &str, password: &str) -> color_eyre::Result<Self> {
        // tracing::info!("User login attempt for: {}", username);

        // let config = crate::config::config();

        // get user from database
        let mut res = DB.query("SELECT * FROM user WHERE username = $username AND crypto::argon2::compare(password, $password)")
            .bind(("username", username.to_string()))
            .bind(("password", password.to_string()))
            .await?;

        let user: Option<User> = res.take(0)?;

        user.ok_or_else(|| color_eyre::eyre::eyre!("Invalid username or password"))
    }
}

pub async fn create_user(username: &str, password: &str) -> Result<(), Box<dyn std::error::Error>> {
    match User::create(username, password).await {
        Ok(_) => {
            // Double-check that the user is now in the database
            let users: Vec<User> = DB.select("user").await?;
            tracing::info!(
                "After creating user, found {} users in database",
                users.len()
            );
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Deserialize)]
pub struct CreateUserRequest {
    username: String,
    password: String,
    scopes: Option<Vec<String>>,
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
    let user = User::get_user(&username)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    user.delete()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn basic_auth(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let users: Vec<User> = match DB.select("user").await {
        Ok(users) => users,
        Err(e) => {
            tracing::error!("Failed to fetch users: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

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

    let auth_header = match req
        .headers()
        .get("Authorization")
        .and_then(|val| val.to_str().ok())
    {
        Some(header) => header,
        None => return unauthorized_response(),
    };

    if !auth_header.starts_with("Basic ") {
        return unauthorized_response();
    }

    let credentials_b64 = auth_header.trim_start_matches("Basic ").trim();
    let decoded = match BASE64.decode(credentials_b64) {
        Ok(decoded) => decoded,
        Err(_) => return unauthorized_response(),
    };

    let decoded_str = match String::from_utf8(decoded) {
        Ok(str) => str,
        Err(_) => return unauthorized_response(),
    };

    let mut parts = decoded_str.splitn(2, ':');
    let username = parts.next();
    let password = parts.next();

    let (username, password) = match (username, password) {
        (Some(u), Some(p)) => (u, p),
        _ => return unauthorized_response(),
    };

    match User::login_user(username, password).await {
        Ok(_) => Ok(next.run(req).await),
        Err(e) => {
            tracing::error!("Authentication failed for user {}: {}", username, e);
            unauthorized_response()
        }
    }
}

fn unauthorized_response() -> Result<Response, StatusCode> {
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
