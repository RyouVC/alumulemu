// use crate::db::scan_games_path;
use crate::index::{Index, TinfoilResponse};
use axum::{
    BoxError, Json, Router,
    error_handling::{HandleError, HandleErrorLayer},
    extract::Path as HttpPath,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::path::Path;
use tokio_util::io::ReaderStream;

// #[derive(Debug, serde::Serialize, serde::Deserialize)]
// pub struct ErrorResponse {
//     pub failure: String,
// }

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("0:?")]
    Error(#[from] color_eyre::Report),
    #[error("Internal error: {0}")]
    InternalError(#[from] BoxError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
            let status = StatusCode::INTERNAL_SERVER_ERROR;
            let body = Json(TinfoilResponse::Failure(self.to_string()));
            (status, body).into_response()
    }
}

// impl IntoResponse for ErrorResponse {
//     fn into_response(self) -> Response {
//         let status = StatusCode::INTERNAL_SERVER_ERROR;
//         let body = Json(self);
//         (status, body).into_response()
//     }
// }

type AlumRes<T> = Result<T, Error>;

pub async fn scan_games_path(path: &str) -> color_eyre::eyre::Result<Index> {
    let mut idx = Index::default();
    let paths = std::fs::read_dir(path).map_err(|e| Error::Error(color_eyre::eyre::eyre!(e.to_string())))?;

    // todo: use walkdir or jwalk for recursive scanning
    for (_, path) in paths.enumerate() {
        let path = path.map_err(|e| Error::Error(color_eyre::eyre::eyre!(e.to_string())))?;
        // let metadata = path.metadata().map_err(|e| e.to_string())?;
        // let size = metadata.len() as u32;
        // let url = format!("/api/get_game/{}", path.file_name().to_string_lossy());

        idx.add_file(&path.path(), "/api/get_game");
    }

    // Ok(idx)

    // let json_string = serde_json::to_string(&response).map_err(|e| e.to_string())?;
    Ok(idx)
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    let response = TinfoilResponse::Failure(format!("Server error: {}", error));
    Json(response)
}

pub async fn list_files() -> AlumRes<Json<Index>> {
    let games = scan_games_path("games/").await?;
    println!("{:?}", games);

    Ok(Json(games))
}

pub async fn download_file(
    HttpPath(filename): HttpPath<String>,
) -> Result<impl IntoResponse, StatusCode> {
    if filename.contains("..") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file = match tokio::fs::File::open(format!("games/{}", filename)).await {
        Ok(file) => file,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

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

pub fn create_router() -> Router {
    Router::new()
        .route("/", get(list_files))
        .route("/api/get_game/{filename}", get(download_file))
        .fallback(|| async { Json(TinfoilResponse::Failure("Not Found".to_string())) })
    // .layer(tower::ServiceBuilder::new().layer(HandleErrorLayer::new(handle_error)))
}
