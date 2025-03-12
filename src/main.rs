mod index;
use axum::{
    Json, Router,
    http::StatusCode,
    routing::{get, post},
};

use serde_json::{Value, json};
use std::collections::HashMap;
use surrealdb::Surreal;
use surrealdb::engine::local::{Db, RocksDb};
use surrealdb::sql::Thing;

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct Game {
    #[serde(skip_serializing_if = "Option::is_none")]
    game_id: Option<Thing>,
    name: String,
    version: u8,
    region: String,
    release_date: String,
    rating: u8,
    publisher: String,
    description: String,
    size: u32,
    rank: u8,
    path: String,
}

async fn jsonify_db(db: &Surreal<Db>) -> surrealdb::Result<String> {
    let all_games: Vec<Game> = db.select("game").await?;
    let mut games_map = HashMap::<String, Value>::new();

    for (index, game) in all_games.into_iter().enumerate() {
        let game_id = game
            .game_id
            .as_ref()
            .expect("game_id not found")
            .id
            .to_string();
        let game_value = json!({
            "app_id": game_id,
            "bannerUrl": "",
            "category": ["Adventure"],
            "extension": "nsp",
            "filename": format!("{}.nsp", game.name),
            "filepath": game.path,
            "folder": format!("/{}", game.name),
            "has_all_dlcs": true,
            "has_base": true,
            "has_latest_version": true,
            "iconUrl": "",
            "id": game_id,
            "identification": "filename",
            "library": "/games",
            "name": game.name,
            "size": game.size,
            "title_id": game_id,
            "title_id_name": game.name,
            "type": "BASE",
            "version": [{
                "owned": false,
                "release_date": game.release_date,
                "update_number": 1,
                "version": 65536
            }]
        });

        games_map.insert(index.to_string(), game_value);
    }

    let final_json = Value::Object(serde_json::Map::from_iter([
        (
            "games".to_string(),
            Value::Object(serde_json::Map::from_iter(
                games_map.clone().into_iter().map(|(k, v)| (k, v)),
            )),
        ),
        ("total".to_string(), Value::Number(games_map.len().into())),
    ]));

    match serde_json::to_string_pretty(&final_json) {
        Ok(json) => Ok(json),
        Err(e) => Err(surrealdb::Error::Db(surrealdb::error::Db::Serialization(
            e.to_string(),
        ))),
    }
}

async fn create_example_games(db: &Surreal<Db>) -> surrealdb::Result<()> {
    let created: Option<Game> = db
        .create(("game", "050000AFAFAF0000"))
        .content(Game {
            game_id: Some(Thing::from(("game", "050000AFAFAF0000"))),
            name: "1 Example Game".to_string(),
            version: 0,
            region: "US".to_string(),
            release_date: "2024-01-01".to_string(),
            rating: 10,
            publisher: "Example Publisher".to_string(),
            description: "An example game".to_string(),
            size: 14000000,
            rank: 1,
            path: "games/example_game.nsp".to_string(),
        })
        .await?;
    println!("Created: {:?}", created);

    let created: Option<Game> = db
        .create(("game", "050000AFAFAF0001"))
        .content(Game {
            game_id: Some(Thing::from(("game", "050000AFAFAF0001"))),
            name: "2 Example Game 2".to_string(),
            version: 0,
            region: "US".to_string(),
            release_date: "2024-01-01".to_string(),
            rating: 10,
            publisher: "Example Publisher".to_string(),
            description: "Another example game".to_string(),
            size: 14000000,
            rank: 1,
            path: "games/example_game_2.nsp".to_string(),
        })
        .await?;
    println!("Created: {:?}", created);

    Ok(())
}

#[tokio::main]
async fn main() -> surrealdb::Result<()> {
    tracing_subscriber::fmt::init();

    // create games directory
    if !std::path::Path::new("games/").exists() {
        std::fs::create_dir("games/").unwrap();
        println!("Directory 'games/' does not exist, creating...");
    } else {
        println!("Directory 'games/' already exists, skipping...");
    }

    // initialize database
    let db = Surreal::new::<RocksDb>("database").await?;
    db.use_ns("tinfoil").use_db("games").await?;

    // create example games
    create_example_games(&db).await.ok();

    // Test retrieval
    //let game: Option<Game> = db.select(("game", "example")).await?;
    //println!("Retrieved: {:?}", game);

    // jason
    tracing::info!("JSONified: {:?}", jsonify_db(&db).await?);

    // get the web thing working

    let app = Router::new()
        .route("/", get(|| async { Json(json!({"success": "connected"})) }))
        .route(
            "/api/titles",
            get(|| async move {
                (
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    jsonify_db(&db).await.unwrap(),
                )
            }),
        );
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
