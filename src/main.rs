mod config;
mod db;
mod index;
mod nsp;
mod nst;
mod router;
mod titledb;
mod util;

use db::init_database;

use reqwest::Client;
use router::create_router;

use titledb::TitleDBImport;
use util::download_titledb;

pub fn games_dir() -> String {
    let config = config::config();
    config.backend_config.rom_dir
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();
    color_eyre::install().unwrap();

    let config = config::config();

    // create games directory
    if !std::path::Path::new(&games_dir()).exists() {
        std::fs::create_dir(games_dir()).unwrap();
        println!("Directory '{}' does not exist, creating...", games_dir());
    } else {
        println!("Directory '{}' already exists, skipping...", games_dir());
    }

    // initialize database
    init_database().await?;

    // run the TitleDB import in the background
    tokio::spawn(async {
        let span = tracing::info_span!("titledb_import");
        let _enter = span.enter();

        tracing::info!("Importing TitleDB...");
        let region = config.backend_config.primary_region;
        let language = config.backend_config.primary_lang;
        let client = Client::new();
        if !std::path::Path::new(&format!("{}.{}.json", region, language)).exists() {
            download_titledb(&client, &region, &language).await.unwrap();
        } else {
            tracing::info!("TitleDB .json already exists, skipping...");
        }

        let path = format!("{}.{}.json", region, language);
        let us_titledb_file = std::fs::File::open(path).unwrap();

        let _ = TitleDBImport::from_json_reader_streaming(
            us_titledb_file,
            &format!("{region}_{language}"),
        )
        .await;
        tracing::info!("TitleDB import complete!");
    });
    tracing::info!("Building frontend...");
    let app = create_router();
    let listener = tokio::net::TcpListener::bind(config.host).await.unwrap();
    tracing::info!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
