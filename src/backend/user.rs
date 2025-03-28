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
use std::fmt;
use std::str::FromStr;

use crate::{db::DB, index::TinfoilResponse};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserScope {
    Admin,
    Editor,
    Viewer,
}

impl UserScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserScope::Admin => "admin",
            UserScope::Editor => "editor",
            UserScope::Viewer => "viewer",
        }
    }

    // Utility functions to check permissions
    pub fn can_view(&self) -> bool {
        true // All scopes can view
    }

    pub fn can_edit(&self) -> bool {
        matches!(self, UserScope::Admin | UserScope::Editor)
    }

    pub fn can_admin(&self) -> bool {
        matches!(self, UserScope::Admin)
    }
}

impl fmt::Display for UserScope {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for UserScope {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(UserScope::Admin),
            "editor" => Ok(UserScope::Editor),
            "viewer" => Ok(UserScope::Viewer),
            _ => Err(format!("Unknown scope: {}", s)),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
}

impl User {
    // Get user scopes as enum
    pub fn get_scopes(&self) -> Vec<UserScope> {
        self.scopes
            .clone()
            .unwrap_or_default()
            .iter()
            .filter_map(|s| UserScope::from_str(s).ok())
            .collect()
    }

    // Check if user has a specific scope
    pub fn has_scope(&self, scope: &UserScope) -> bool {
        self.get_scopes().contains(scope)
    }

    // Helper methods for permission checks
    pub fn can_view(&self) -> bool {
        !self.get_scopes().is_empty() || self.get_scopes().iter().any(|s| s.can_view())
    }

    pub fn can_edit(&self) -> bool {
        self.get_scopes().iter().any(|s| s.can_edit())
    }

    pub fn can_admin(&self) -> bool {
        self.get_scopes().iter().any(|s| s.can_admin())
    }

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

pub async fn create_user(
    username: &str,
    password: &str,
    scopes: Option<Vec<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if this is the first user being created
    let users: Vec<User> = match DB.select("user").await {
        Ok(users) => users,
        Err(e) => {
            tracing::error!("Failed to fetch users during user creation: {}", e);
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to check existing users: {}", e),
            )));
        }
    };

    let is_first_user = users.is_empty();

    // Create the user
    match User::create(username, password).await {
        Ok(user) => {
            // Determine scopes: if first user, make admin; otherwise use provided scopes
            let final_scopes = if is_first_user {
                tracing::info!("Creating first user '{}' with admin scope", username);
                Some(vec!["admin".to_string()])
            } else {
                scopes
            };

            // If scopes are provided, update the user with the specified scopes
            if let Some(scopes) = final_scopes {
                // Only if scopes is not empty
                if !scopes.is_empty() {
                    // Update user with scopes
                    let result = DB
                        .query("UPDATE user SET scopes = $scopes WHERE username = $username")
                        .bind(("username", username.to_string()))
                        .bind(("scopes", scopes))
                        .await;

                    if let Err(e) = result {
                        tracing::error!("Failed to update user scopes: {}", e);
                        return Err(Box::new(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to update user scopes: {}", e),
                        )));
                    }
                }
            }

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

    // Add support for setting user scopes during creation
    #[serde(default)]
    pub scopes: Option<Vec<String>>,
}

pub async fn create_user_handler(
    Json(payload): Json<CreateUserRequest>,
) -> Result<StatusCode, StatusCode> {
    match create_user(&payload.username, &payload.password, payload.scopes).await {
        Ok(_) => Ok(StatusCode::CREATED),
        Err(e) => {
            tracing::error!("Failed to create user: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Serialize)]
pub struct UserInfo {
    username: String,
    scopes: Vec<String>,
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
                scopes: u.scopes.unwrap_or_default(),
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

/// Middleware for optional basic authentication, can be toggled on/off with an environment variable
///
/// Checks if the backend is public *or* if there are no users in the database. If there are no users,
/// authentication is bypassed and a warning header is added to the response.
///
/// If there are users, redirects to `basic_auth` middleware.
pub async fn basic_auth_if_public(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let config = crate::config::config();
    let is_public = config.backend_config.public;

    if (!is_public) {
        basic_auth(req, next).await
    } else {
        Ok(next.run(req).await)
    }
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

pub async fn auth_require_viewer(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // First authenticate the user
    let (user, auth_req) = match authenticate_user(req).await {
        Ok((user, auth_req)) => (user, auth_req),
        Err(resp) => return resp,
    };

    // Viewer permission check - all authenticated users have at least viewer permissions
    if !user.can_view() {
        tracing::warn!("User {} does not have viewer permission", user.username);
        return unauthorized_response();
    }

    // Create extension with user info for downstream handlers
    let mut auth_req = auth_req;
    auth_req.extensions_mut().insert(user);

    Ok(next.run(auth_req).await)
}

pub async fn auth_require_editor(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // First authenticate the user
    let (user, auth_req) = match authenticate_user(req).await {
        Ok((user, auth_req)) => (user, auth_req),
        Err(resp) => return resp,
    };

    // Editor permission check
    if !user.can_edit() {
        tracing::warn!("User {} does not have editor permission", user.username);
        return unauthorized_response();
    }

    // Create extension with user info for downstream handlers
    let mut auth_req = auth_req;
    auth_req.extensions_mut().insert(user);

    Ok(next.run(auth_req).await)
}

pub async fn auth_require_admin(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    // First authenticate the user
    let (user, auth_req) = match authenticate_user(req).await {
        Ok((user, auth_req)) => (user, auth_req),
        Err(resp) => return resp,
    };

    // Admin permission check
    if !user.can_admin() {
        tracing::warn!("User {} does not have admin permission", user.username);
        return unauthorized_response();
    }

    // Create extension with user info for downstream handlers
    let mut auth_req = auth_req;
    auth_req.extensions_mut().insert(user);

    Ok(next.run(auth_req).await)
}

// Helper function to authenticate a user and return the User along with the Request
async fn authenticate_user(
    req: Request<Body>,
) -> Result<(User, Request<Body>), Result<Response, StatusCode>> {
    let users: Vec<User> = match DB.select("user").await {
        Ok(users) => users,
        Err(e) => {
            tracing::error!("Failed to fetch users: {}", e);
            return Err(Err(StatusCode::INTERNAL_SERVER_ERROR));
        }
    };

    if users.is_empty() {
        tracing::warn!(
            "No users found in database. Authentication bypassed! Please create at least 1 admin user"
        );

        // Create a default admin user for systems with no users
        // This allows bypassing all auth gates when there are no users
        let default_user = User {
            username: "anonymous".to_string(),
            password: "".to_string(),
            scopes: Some(vec![
                "admin".to_string(),
                "editor".to_string(),
                "viewer".to_string(),
            ]),
        };

        return Ok((default_user, req));
    }

    let auth_header = match req
        .headers()
        .get("Authorization")
        .and_then(|val| val.to_str().ok())
    {
        Some(header) => header,
        None => return Err(unauthorized_response()),
    };

    if !auth_header.starts_with("Basic ") {
        return Err(unauthorized_response());
    }

    let credentials_b64 = auth_header.trim_start_matches("Basic ").trim();
    let decoded = match BASE64.decode(credentials_b64) {
        Ok(decoded) => decoded,
        Err(_) => return Err(unauthorized_response()),
    };

    let decoded_str = match String::from_utf8(decoded) {
        Ok(str) => str,
        Err(_) => return Err(unauthorized_response()),
    };

    let mut parts = decoded_str.splitn(2, ':');
    let username = parts.next();
    let password = parts.next();

    let (username, password) = match (username, password) {
        (Some(u), Some(p)) => (u, p),
        _ => return Err(unauthorized_response()),
    };

    match User::login_user(username, password).await {
        Ok(user) => Ok((user, req)),
        Err(e) => {
            tracing::error!("Authentication failed for user {}: {}", username, e);
            Err(unauthorized_response())
        }
    }
}

/// Middleware for optional authentication that provides viewer access for public systems
/// This is a replacement for basic_auth_if_public that integrates with the HRBAC system
pub async fn auth_optional_viewer(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let config = crate::config::config();
    let is_public = config.backend_config.public;

    if !is_public {
        auth_require_viewer(req, next).await
    } else {
        // For public systems, we don't require authentication but we do add a default
        // viewer user to the request context
        let default_user = User {
            username: "anonymous".to_string(),
            password: "".to_string(),
            scopes: Some(vec!["viewer".to_string()]),
        };

        // Add the default user to the request context
        let mut public_req = req;
        public_req.extensions_mut().insert(default_user);

        Ok(next.run(public_req).await)
    }
}

pub fn user_router() -> Router {
    Router::new()
        .route("/", get(list_users))
        .route("/", post(create_user_handler))
        .route("/{username}", delete(delete_user))
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
        .layer(axum::middleware::from_fn(auth_require_admin))
    // Authentication is handled at the API router level now
}
