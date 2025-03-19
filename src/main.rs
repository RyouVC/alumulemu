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
use reqwest::header::ACCEPT_LANGUAGE;
use router::create_router;
use std::cmp::min;
use std::fs::File;
use std::io::Write;
use surrealdb::Surreal;
use surrealdb::engine::local::RocksDb;
use titledb::TitleDBImport;

async fn download_titledb(client: &Client, region: &str, language: &str) -> Result<(), String> {
    let url = format!(
        "https://github.com/blawar/titledb/raw/refs/heads/master/{}.{}.json",
        region, language
    );
    let path = format!("{}.{}.json", region, language);
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
        let chunk = item.or(Err(format!("Error while downloading file")))?;
        file.write_all(&chunk)
            .or(Err(format!("Error while writing to file")))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message(format!("Downloaded {} to {}", url, path_clone));
    Ok(())
}

pub fn games_dir() -> String {
    std::env::var("GAMES_DIR").unwrap_or("games/".to_string())
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install().unwrap();

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
        let region = std::env::var("REGION").unwrap_or("US".to_string());
        let language = std::env::var("LANGUAGE").unwrap_or("en".to_string());
        let client = Client::new();
        if !std::path::Path::new(&format!("{}.{}.json", region, language)).exists() {
            download_titledb(&client, &region, &language).await.unwrap();
        } else {
            tracing::info!("TitleDB .json already exists, skipping...");
        }

        let path = format!("{}.{}.json", region, language);
        let us_titledb_file = std::fs::File::open(path).unwrap();
        // let us_titledb_file = std::fs::File::open("src/zeld.json").unwrap();
        // match TitleDBImport::from_json_reader(us_titledb_file) {
        //     Ok(us_titledb) => {
        //         if let Err(e) = us_titledb.import_to_db("US-en").await {
        //             eprintln!("Error importing to DB: {}", e);
        //         }
        //     },
        //     Err(e) => eprintln!("Error reading titledb: {}", e),
        // },
        let _ = TitleDBImport::from_json_reader_streaming(
            us_titledb_file,
            &format!("{region}_{language}"),
        )
        .await;
        //std::fs::remove_file(format!("{}.{}.json", region, language)).unwrap();
        tracing::info!("TitleDB downloaded, Alumulemu running...")
    });

    let app = create_router();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
