use crate::db::NspMetadata;
use crate::titledb_cache_dir;
use color_eyre::Result;
use reqwest::Client;

const TITLEDB_BASEURL: &str = "https://github.com/blawar/titledb/raw/refs/heads/master";

/// Downloads a TitleDB file from the internet
pub async fn download_titledb(client: &Client, region: &str, lang: &str) -> Result<String> {
    let url = format!("{TITLEDB_BASEURL}/{}.{}.json", region, lang);

    let cache_dir = titledb_cache_dir();
    let file_path = cache_dir.join(format!("{}.{}.json", region, lang))
        .to_str()
        .unwrap()
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

    let version = &metadata
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

