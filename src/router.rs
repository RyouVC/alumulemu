// use crate::db::scan_games_path;
use crate::db::DB;
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

pub async fn scan_games_path(path: &str) -> color_eyre::eyre::Result<Index> {
    use regex::Regex;
    let correct_format =
        Regex::new(r"^.+\s\[[A-Fa-f0-9]{16}\]\[v\d+\]\.(nsp|xci|nsz|ncz|xcz)$").unwrap();

    let mut idx = Index::default();
    let paths = std::fs::read_dir(path)
        .map_err(|e| Error::Error(color_eyre::eyre::eyre!(e.to_string())))?;

    for (_, path) in paths.enumerate() {
        let path = path.map_err(|e| Error::Error(color_eyre::eyre::eyre!(e.to_string())))?;
        let filename = path.file_name().to_string_lossy().into_owned();

        if !filename.ends_with(".nsp")
            && !filename.ends_with(".xci")
            && !filename.ends_with(".nsz")
            && !filename.ends_with(".ncz")
            && !filename.ends_with(".xcz")
        {
            continue;
        }

        let game_data = GameFileDataNaive::parse(&filename);

        let extension = filename.rsplit('.').next().unwrap_or("nsp");

        let formatted_name = if let Some(title_id) = game_data.title_id {
            if correct_format.is_match(&filename) {
                let name = filename
                    .trim_end_matches(&format!(".{}", extension))
                    .to_string();
                format!("{}.{}", name, extension)
            } else {
                format!(
                    "{} [{}][{}].{}",
                    // god damn double spaces
                    game_data
                        .name
                        .trim_end_matches(&format!(".{}", extension))
                        .split_whitespace()
                        .collect::<Vec<&str>>()
                        .join(" "),
                    title_id,
                    game_data.version.unwrap_or_else(|| "v0".to_string()),
                    extension
                )
            }
        } else {
            // if it's NSP, try to get title ID from TIK file embedded in NSP
            let title_id = if filename.ends_with(".nsp") {
                match get_title_id_from_nsp(path.path().to_str().unwrap_or_default()) {
                    Ok(id) => id,
                    Err(_) => "00000000AAAA0000".to_string(), // if it errors out we just use the fallback title ID
                }
            } else {
                "00000000AAAA0000".to_string()
            };

            format!(
                "{} [{}][v0].{}",
                // get the double spaces OUT
                game_data
                    .name
                    .trim_end_matches(&format!(".{}", extension))
                    .split_whitespace()
                    .collect::<Vec<&str>>()
                    .join(" "),
                title_id,
                extension
            )
        };

        idx.add_file(&path.path(), "/api/get_game", &formatted_name);
    }

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
