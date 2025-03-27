use crate::db::NspMetadata;
use color_eyre::Result;
use reqwest::Client;
use std::{fs::File, io, path::PathBuf};
use tempfile::TempDir;

const TITLEDB_BASEURL: &str = "https://github.com/blawar/titledb/raw/refs/heads/master";

/// Create a temporary directory in the cache directory
/// Returns None if the directory couldn't be created
pub fn tempdir() -> Result<TempDir> {
    tempfile::tempdir_in(cache_dir().map_err(|e| {
        tracing::error!("Failed to get cache directory: {}", e);
        e
    })?)
    .map_err(|e| {
        tracing::error!("Failed to create temporary directory: {}", e);
        e.into()
    })
}

/// Create a temporary file in the cache directory
/// Returns None if the file couldn't be created
pub fn tempfile() -> Result<File> {
    tempfile::tempfile_in(cache_dir().map_err(|e| {
        tracing::error!("Failed to get cache directory: {}", e);
        e
    })?)
    .map_err(|e| {
        tracing::error!("Failed to create temporary file: {}", e);
        e.into()
    })
}

/// Get the path to the cache directory, creating it if it doesn't exist
pub fn cache_dir() -> Result<PathBuf> {
    let cache_dir = crate::config::config().backend_config.cache_dir;
    // create if not exists
    std::fs::create_dir_all(&cache_dir).map_err(|e| {
        tracing::error!("Failed to create cache directory {}: {}", cache_dir, e);
        color_eyre::eyre::eyre!("IO error: {}", e)
    })?;
    Ok(cache_dir.into())
}

/// Get the path to the titledb cache directory
pub fn titledb_cache_dir() -> PathBuf {
    let cache_path = cache_dir().unwrap_or_else(|_| {
        let fallback = PathBuf::from("/tmp/alumulemu/titledb");
        tracing::warn!(
            "Failed to get cache directory, using fallback: {}",
            fallback.display()
        );
        fallback
    });

    let cache_dir = cache_path.join("titledb");

    // Ensure the directory exists
    if !std::path::Path::new(&cache_dir).exists() {
        if let Err(e) = std::fs::create_dir_all(&cache_dir) {
            tracing::error!("Failed to create titledb cache directory: {}", e);
        } else {
            tracing::debug!("Created titledb cache directory: {}", cache_dir.display());
        }
    }
    cache_dir
}

/// Downloads a TitleDB file from the internet
pub async fn download_titledb(client: &Client, region: &str, lang: &str) -> Result<String> {
    let url = format!("{TITLEDB_BASEURL}/{}.{}.json", region, lang);
    let cache_dir = titledb_cache_dir();
    let file_path = cache_dir
        .join(format!("{}.{}.json", region, lang))
        .to_str()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Invalid path for TitleDB file: {}/{}.{}.json",
                    cache_dir.display(),
                    region,
                    lang
                ),
            )
        })?
        .to_string();

    tracing::info!(
        "Downloading TitleDB for {} {} to {}",
        region,
        lang,
        file_path
    );

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(color_eyre::eyre::eyre!(
            "Failed to download TitleDB: {}",
            resp.status()
        ));
    }

    let bytes = resp.bytes().await?;
    std::fs::write(&file_path, bytes)?;
    Ok(file_path)
}

/// Formats a game name for display with title ID and version information
pub fn format_game_name(metadata: &NspMetadata, filename: &str, extension: &str) -> String {
    let name = match &metadata.title_name {
        Some(n) => n.clone(),
        None => filename.trim().trim_end_matches(extension).to_string(),
    };

    let version = metadata
        .version
        .strip_prefix('v')
        .unwrap_or(&metadata.version);

    format!(
        "{} [{}][v{}].{}",
        name, metadata.title_id, version, extension
    )
}

/// Creates a download ID for a game based on the title ID, extension and version information
// example: 010005501E68C000_v65536.xci
pub fn format_download_id(title_id: &str, version: &str, ext: &str) -> String {
    let version = version.strip_prefix('v').unwrap_or(version);
    format!("{}_v{}.{}", title_id, version, ext)
}
