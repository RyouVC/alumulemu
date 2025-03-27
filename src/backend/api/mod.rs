use crate::{
    index::Index,
    router::{AlumRes, generate_index_from_metadata},
};
use axum::{Json, Router, routing::get};

use super::user::user_router;

pub mod dl;
pub mod metadata;

pub async fn tinfoil_index() -> AlumRes<Json<Index>> {
    let games = generate_index_from_metadata().await?;
    // tracing::trace!("Games retrieved: {:?}", games);
    Ok(Json(games))
}

pub fn api_router() -> Router {
    Router::new()
        .nest("/users", user_router())
        .route("/tinfoil", get(tinfoil_index))
        .route("/get_game/{download_id}", get(dl::download_file))
        // Metadata routes
        .route("/title_meta/{title_id}", get(metadata::title_meta))
        .route(
            "/title_meta/{title_id}/base_game",
            get(metadata::title_meta_base_game),
        )
        .route(
            "/title_meta/{title_id}/download_ids",
            get(metadata::get_download_ids),
        )
        .route(
            "/grouped/{title_id}",
            get(metadata::list_grouped_by_titleid),
        )
        .route("/base_games", get(metadata::list_base_games))
        .route("/base_games/search", get(metadata::search_base_game))
        .route("/titledb/search", get(metadata::search_titledb))
        .route("/search", get(metadata::search_titles))
}
