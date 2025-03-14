// use crate::db::scan_games_path;
use crate::db::DB;
use crate::games_dir;
use crate::index::{Index, TinfoilResponse};
use crate::nsp::get_title_id_from_nsp;
use crate::titledb::GameFileDataNaive;
use axum::{
    BoxError, Json, Router,
    error_handling::{HandleError, HandleErrorLayer},
    extract::Path as HttpPath,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::env;
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
#[tracing::instrument]
pub async fn scan_games_path(path: &str) -> color_eyre::eyre::Result<Index> {
    // use regex::Regex;
    // let correct_format =
    //     Regex::new(r"^.+\s\[[A-Fa-f0-9]{16}\]\[v\d+\]\.(nsp|xci|nsz|ncz|xcz)$").unwrap();

    let mut idx = Index::default();
    let walker = jwalk::WalkDir::new(path);
    let paths = walker.into_iter();

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
        let game_data = GameFileDataNaive::get(&path.path()).await?;
        // println!("{:?}", game_data);

        let formatted_name = {
            match game_data.extension {
                Some(ext) => format!(
                    "{} [{}][{}].{}",
                    game_data.name,
                    game_data
                        .title_id
                        .unwrap_or_else(|| "00000000AAAA0000".to_string()),
                    game_data.version.unwrap_or_else(|| "v0".to_string()),
                    ext
                ),
                None => format!(
                    "{} [{}][{}]",
                    game_data.name,
                    game_data
                        .title_id
                        .unwrap_or_else(|| "00000000AAAA0000".to_string()),
                    game_data.version.unwrap_or_else(|| "v0".to_string())
                ),
            }
        };

        tracing::trace!("Formatted name: {}", formatted_name);

        idx.add_file(&path.path(), "/api/get_game", &formatted_name);
    }

    Ok(idx)
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    let response = TinfoilResponse::Failure(format!("Server error: {}", error));
    Json(response)
}

pub async fn list_files() -> AlumRes<Json<Index>> {
    let games = scan_games_path(&games_dir()).await?;

    tracing::trace!("Games retrieved: {:?}", games);
    Ok(Json(games))
}

pub async fn download_file(
    HttpPath(filename): HttpPath<String>,
) -> Result<impl IntoResponse, StatusCode> {
    if filename.contains("..") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let file_path = format!("{}/{}", games_dir(), filename);
    let file = match tokio::fs::File::open(&file_path).await {
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
