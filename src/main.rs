mod backend;
mod config;
mod db;
mod index;
mod nsp;
mod nst;
mod router;
mod titledb;
mod util;

use cron::Schedule;
use db::{create_precomputed_metaview, init_database};
use reqwest::Client;
use router::create_router;
use std::str::FromStr;
use std::time::Duration;
use titledb::TitleDBImport;
use util::download_titledb;

pub fn games_dir() -> String {
    let config = config::config();
    config.backend_config.rom_dir
}

fn parse_secondary_locale_string(locale: &str) -> (String, String) {
    let parts: Vec<&str> = locale.split('_').collect();
    if parts.len() == 2 {
        (parts[0].to_uppercase(), parts[1].to_lowercase())
    } else {
        panic!("Invalid locale string: {}", locale);
    }
}

async fn import_titledb(lang: &str, region: &str) {
    let client = Client::new();
    let path = format!("{}.{}.json", region, lang);

    let should_download = if let Ok(metadata) = std::fs::metadata(&path) {
        if let Ok(modified) = metadata.modified() {
            let age = modified.elapsed().unwrap_or_default();
            age > Duration::from_secs(6 * 3600)
        } else {
            true
        }
    } else {
        true
    };

    // Check if the Title table is empty first
    if titledb::Title::count(&format!("{region}_{lang}"))
        .await
        .unwrap()
        == 0
    {
        // Force download if table is empty
        let path = download_titledb(&client, region, lang).await.unwrap();
        let titledb_file = std::fs::File::open(&path).unwrap();

        let _ =
            TitleDBImport::from_json_reader_streaming(titledb_file, &format!("{region}_{lang}"))
                .await;
        tracing::info!("TitleDB import complete for {region}_{lang}");
        return;
    }

    // Only check recency if we already have data
    if !should_download {
        tracing::info!("TitleDB .json is recent, skipping...");
        return;
    }

    // Update existing data
    let path = download_titledb(&client, region, lang).await.unwrap();
    let titledb_file = std::fs::File::open(&path).unwrap();
    let _ =
        TitleDBImport::from_json_reader_streaming(titledb_file, &format!("{region}_{lang}")).await;
    tracing::info!("TitleDB update complete!");
}

async fn import_titledb_background(config: config::Config) {
    let span = tracing::info_span!("titledb_import");
    let _enter = span.enter();

    import_titledb(
        &config.backend_config.primary_lang,
        &config.backend_config.primary_region,
    )
    .await;

    tracing::info!("TitleDB import complete for primary locale");

    for locale in config.backend_config.get_valid_secondary_locales() {
        let (region, lang) = parse_secondary_locale_string(&locale);
        import_titledb(&lang, &region).await;
    }

    tracing::info!("TitleDB import complete for all locales");

    create_precomputed_metaview().await.unwrap();
    tracing::info!("Precomputed metaviews created");
}

async fn schedule_titledb_imports(config: config::Config) {
    // Schedule for every 6 hours: midnight, 6am, noon, 6pm
    let expression = "0 0 0,6,12,18 * * * *";
    let schedule = Schedule::from_str(expression).expect("Invalid cron expression");

    loop {
        // Get the current local time
        let now = chrono::Local::now();

        // Find the next scheduled time
        if let Some(next_time) = schedule.upcoming(chrono::Local).next() {
            // Calculate duration until the next run
            let duration_until_next = next_time - now;
            let seconds_until_next = duration_until_next.num_seconds();

            tracing::info!(
                "Next scheduled TitleDB import at {} (in {} hours and {} minutes)",
                next_time.format("%Y-%m-%d %H:%M:%S"),
                seconds_until_next / 3600,
                (seconds_until_next % 3600) / 60
            );

            // Sleep until the next scheduled time
            if seconds_until_next > 0 {
                tokio::time::sleep(Duration::from_secs(seconds_until_next as u64)).await;
            }

            // Run the import task
            tracing::info!("Scheduled TitleDB import starting");
            import_titledb_background(config.clone()).await;
        } else {
            // This should never happen with a valid cron expression
            tracing::error!("Failed to determine next schedule time");
            tokio::time::sleep(Duration::from_secs(3600)).await; // Wait an hour and try again
        }
    }
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

    // Run the initial TitleDB import and schedule future imports
    let config_clone = config.clone();
    tokio::spawn(async move {
        // Run immediately the first time
        import_titledb_background(config_clone.clone()).await;

        // Then schedule recurring imports
        schedule_titledb_imports(config_clone).await;
    });

    tracing::info!("Building frontend...");
    let app = create_router();
    let listener = tokio::net::TcpListener::bind(config.host).await.unwrap();
    tracing::info!("Listening on: {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
