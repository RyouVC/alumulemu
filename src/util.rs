use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use std::cmp::min;
use std::fs::File;
use std::io::Write;

use crate::db::NspMetadata;

const TITLEDB_BASEURL: &str = "https://github.com/blawar/titledb/raw/refs/heads/master/";

/// Downloads a TitleDB file from the internet
pub async fn download_titledb(client: &Client, region: &str, language: &str) -> Result<String, String> {
    tracing::info!("Pulling TitleDB data for {region}-{language}");
    let url = format!("{TITLEDB_BASEURL}/{region}.{language}.json");

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
    let mut file = File::create(&path).or(Err(format!("Failed to create file '{}'", path_clone)))?;
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
    Ok(path_clone)
}

/// Formats a game name for display with title ID and version information
pub fn format_game_name(metadata: &NspMetadata, filename: &str, extension: &str) -> String {
    let name = match &metadata.title_name {
        Some(n) => n.clone(),
        None => filename.trim().trim_end_matches(extension).to_string(),
    };
    format!(
        "{} [{}][{}].{}",
        name, metadata.title_id, metadata.version, extension
    )
}
