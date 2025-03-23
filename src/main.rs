mod config;
mod db;
mod index;
mod nsp;
mod nst;
mod router;
mod titledb;

use db::init_database;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use router::create_router;
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use titledb::TitleDBImport;

async fn download_titledb(client: &Client, region: &str, language: &str) -> Result<(), String> {
    tracing::info!("Pulling TitleDB data for {region}-{language}");
    let url = format!(
        "https://github.com/blawar/titledb/raw/refs/heads/master/{}.{}.json",
        region, language
    );
    let path = format!("{region}.{language}.json");
    let res = client
        .get(&url)
        .send()
        .await
        .or(Err(format!("Failed to GET from '{}'", &url)))?;
    let total_size = res
        .content_length()
        .ok_or(format!("Failed to get content length from '{}'", &url))?;

    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})").unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading file {}", url));
    let path_clone = path.clone();
    let mut file = File::create(path).or(Err(format!("Failed to create file '{}'", path_clone)))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err("Error while downloading file".to_string()))?;
        file.write_all(&chunk)
            .or(Err("Error while writing to file".to_string()))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message(format!("Downloaded {} to {}", url, path_clone));
    tracing::info!("Pulled TitleDB data for {region}-{language}");
    Ok(())
}

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
        tracing::info!("TitleDB downloaded, Alumulemu running...")
    });
    tracing::info!("Building frontend...");
    let app = create_router();
    let listener = tokio::net::TcpListener::bind(config.host).await.unwrap();
    tracing::info!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
