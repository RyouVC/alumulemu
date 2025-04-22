mod backend;
mod config;
mod db;
mod import;
mod index;
mod nsp;
mod router;
mod titledb;
mod util;

use backend::kv_config::{ExtraBackendConfig, KvOptExt};
use color_eyre::Result;
use cron::Schedule;
use db::init_database;
use import::registry::init_registry;
use index::ExtraIndexesImport;
use reqwest::Client;
use router::{create_router, watch_filesystem_for_changes};
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::Duration;
use titledb::TitleDBImport;
use util::download_titledb;

static LOCALE: LazyLock<String> =
    LazyLock::new(|| crate::config::config().backend_config.get_locale_string());

pub fn games_dir() -> String {
    let config = config::config();
    tracing::debug!("Games directory: {}", config.backend_config.rom_dir);
    config.backend_config.rom_dir
}

pub async fn romdir_inotify() {
    if let Err(e) = watch_filesystem_for_changes(&games_dir()).await {
        tracing::error!("Failed to start filesystem watcher: {}", e);
    }
}

fn parse_secondary_locale_string(locale: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = locale.split('_').collect();
    if parts.len() == 2 {
        Ok((parts[0].to_uppercase(), parts[1].to_lowercase()))
    } else {
        Err(color_eyre::eyre::eyre!("Invalid locale string: {}", locale))
    }
}

async fn import_extra_indexes() -> Result<()> {
    let config = config::config();
    // Use the helper method to get only valid indexes
    let idx_to_addlist = config.backend_config.get_valid_extra_indexes();

    if idx_to_addlist.is_empty() {
        tracing::info!("No extra indexes to import");
        return Ok(());
    }

    // Add indexes to list
    for url in idx_to_addlist {
        tracing::info!(%url, "Adding extra index to the list");

        let e_idx = ExtraIndexesImport::new(url);
        e_idx.add().await?;
    }

    let idx_to_import = ExtraIndexesImport::list()
        .await?
        .iter()
        .map(|i| i.url.clone())
        .collect::<Vec<_>>();

    for index in idx_to_import {
        tracing::info!(%index, "Loading extra index");
        // we will just name indexes after the URL
        let idx = index::Index::load_index_url(&index).await?;

        idx.save_extra_index(&index).await?;
        tracing::info!(%index, "Index loaded and saved");
    }
    Ok(())
}

async fn schedule_idx_downloads() -> Result<()> {
    const EXPRESSION: &str = "0 0 0,6,12,18 * * * *";

    let schedule = Schedule::from_str(EXPRESSION)
        .map_err(|e| color_eyre::Report::msg(format!("Invalid cron expression: {}", e)))?;

    loop {
        let now = chrono::Utc::now();
        if let Some(next_time) = schedule.upcoming(chrono::Utc).next() {
            let duration_until_next = next_time - now;
            let seconds_until_next = duration_until_next.num_seconds();

            tracing::info!(
                "Next scheduled index download at {} (in {} hours and {} minutes)",
                next_time,
                seconds_until_next / 3600,
                (seconds_until_next % 3600) / 60
            );

            if seconds_until_next > 0 {
                tokio::time::sleep(Duration::from_secs(seconds_until_next as u64)).await;
            }

            tracing::info!("Scheduled index download starting");
            if let Err(e) = import_extra_indexes().await {
                tracing::error!("Scheduled index download failed: {}", e);
            }
        } else {
            tracing::error!("Failed to determine next schedule time");
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }
}

async fn import_titledb(lang: &str, region: &str) -> Result<()> {
    let client = Client::new();
    let cache_dir = util::titledb_cache_dir();
    let path = cache_dir.join(format!("{}.{}.json", region, lang));

    let should_download = if let Ok(metadata) = std::fs::metadata(&path) {
        if let Ok(modified) = metadata.modified() {
            let age = modified.elapsed().unwrap_or_default();
            age > Duration::from_secs(6 * 3600)
        } else {
            tracing::warn!(
                "Could not get modification time for {:?}, will download again",
                path
            );
            true
        }
    } else {
        tracing::debug!("File {:?} does not exist, will download", path);
        true
    };

    if should_download {
        match download_titledb(&client, region, lang).await {
            Ok(path_str) => match std::fs::File::open(&path_str) {
                Ok(titledb_file) => {
                    if let Err(e) = TitleDBImport::from_json_reader_streaming(
                        titledb_file,
                        &format!("{region}_{lang}"),
                    )
                    .await
                    {
                        tracing::error!("Failed to import TitleDB: {}", e);
                    } else {
                        tracing::info!("TitleDB update complete!");
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to open TitleDB file {}: {}", path_str, e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to download TitleDB for {}-{}: {}", region, lang, e);
            }
        }
        return Ok(());
    }

    // Check if the Title table is empty
    match titledb::Title::count(&format!("{region}_{lang}")).await {
        Ok(count) => {
            if count == 0 {
                // Force import if table is empty, but don't re-download
                match std::fs::File::open(&path) {
                    Ok(titledb_file) => {
                        let start = std::time::Instant::now();
                        let result = TitleDBImport::from_json_reader_streaming(
                            titledb_file,
                            &format!("{region}_{lang}"),
                        )
                        .await;

                        let duration = start.elapsed();

                        if let Err(e) = result {
                            tracing::error!("TitleDB import failed for {region}_{lang}: {}", e);
                        } else {
                            tracing::info!(
                                "TitleDB import for {region}_{lang} took: {:?}",
                                duration
                            );
                            tracing::info!("TitleDB import complete for {region}_{lang}");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to open TitleDB file {:?}: {}", path, e);
                    }
                }
            } else {
                tracing::info!("TitleDB .json is recent and table has data, skipping...");
            }
        }
        Err(e) => {
            tracing::error!("Failed to get title count: {}", e);
        }
    }

    Ok(())
}

async fn import_titledb_background(config: config::Config) -> Result<()> {
    let span = tracing::info_span!("titledb_import");
    let _enter = span.enter();

    // Create primary import task
    let primary_task: tokio::task::JoinHandle<Result<()>> = tokio::spawn({
        let backend_config = config.backend_config.clone();
        let lang = backend_config.primary_lang.clone();
        let region = backend_config.primary_region.clone();
        async move {
            if let Err(e) = import_titledb(&lang, &region).await {
                tracing::error!("Primary TitleDB import failed: {}", e);
            } else {
                tracing::info!("TitleDB import complete for primary locale");
            }
            Ok(())
        }
    });

    // Create secondary import tasks
    let secondary_tasks: Vec<_> = config
        .backend_config
        .get_valid_secondary_locales()
        .into_iter()
        .map(|locale| {
            tokio::spawn(async move {
                match parse_secondary_locale_string(&locale) {
                    Ok((region, lang)) => {
                        if let Err(e) = import_titledb(&lang, &region).await {
                            tracing::error!(
                                "Secondary TitleDB import failed for {}: {}",
                                locale,
                                e
                            );
                        } else {
                            tracing::info!("TitleDB import complete for {}", locale);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Invalid secondary locale '{}': {}", locale, e);
                    }
                }
                Ok(())
            })
        })
        .collect();

    // Wait for all tasks to complete concurrently
    let mut all_tasks = vec![primary_task];
    all_tasks.extend(secondary_tasks);

    let results = futures::future::join_all(all_tasks).await;

    // Check for errors but don't fail the entire process
    for (i, result) in results.into_iter().enumerate() {
        if let Err(e) = result {
            tracing::error!("Import task {} failed: {}", i, e);
        }
    }

    tracing::info!("TitleDB import complete for all locales");
    Ok(())
}

async fn schedule_titledb_imports(config: config::Config) -> Result<()> {
    // Schedule for every 6 hours: midnight, 6am, noon, 6pm
    const EXPRESSION: &str = "0 0 0,6,12,18 * * * *";
    let schedule = match Schedule::from_str(EXPRESSION) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Invalid cron expression: {}", e);
            return Err(color_eyre::eyre::eyre!("Invalid cron expression: {}", e));
        }
    };

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
            if let Err(e) = import_titledb_background(config.clone()).await {
                tracing::error!("Scheduled TitleDB import failed: {}", e);
            }
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

    // Set up tracing without unwraps
    let tracing_builder = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                // .add_directive("alumulemu=info".parse().unwrap())
                .add_directive("tungstenite=error".parse().unwrap())
                .add_directive("tokio_tungstenite=error".parse().unwrap())
                .add_directive("hyper=error".parse().unwrap())
                .add_directive("reqwest=error".parse().unwrap())
                .add_directive("tokio=error".parse().unwrap())
                .add_directive("tower_http=debug".parse().unwrap()),
        )
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(true)
        .with_target(false);

    #[cfg(debug_assertions)]
    tracing_builder.pretty().init();
    #[cfg(not(debug_assertions))]
    tracing_builder.compact().init();

    if let Err(e) = color_eyre::install() {
        eprintln!("Failed to install color_eyre: {}", e);
    }

    let config = config::config();

    // Initialize importer registry
    init_registry();
    tracing::info!("Importer registry initialized");

    // create games directory
    if !std::path::Path::new(&games_dir()).exists() {
        match std::fs::create_dir(games_dir()) {
            Ok(_) => tracing::info!("Directory '{}' created successfully", games_dir()),
            Err(e) => {
                tracing::error!("Failed to create directory '{}': {}", games_dir(), e);
                // Continue anyway, failure will be handled when trying to access
            }
        }
    } else {
        tracing::info!("Directory '{}' already exists, skipping...", games_dir());
    }

    // initialize database
    init_database().await?;

    // Run the initial TitleDB import and schedule future imports
    let config_clone = config.clone();

    let extra_cfg = ExtraBackendConfig::get().await?.unwrap_or_default();

    tokio::spawn(async move {
        // Run immediately the first time

        if extra_cfg.import_titledb_on_start {
            if let Err(e) = import_titledb_background(config_clone.clone()).await {
                tracing::error!("Initial TitleDB import failed: {}", e);
            }
        }

        // Then schedule recurring imports
        if let Err(e) = schedule_titledb_imports(config_clone).await {
            tracing::error!("TitleDB scheduling failed: {}", e);
        }

        // Start the inotify watcher
        romdir_inotify().await;

        Ok::<(), color_eyre::Report>(())
    });

    // index importer job
    tokio::spawn(async move {
        // Run the initial import of extra indexes if configured
        if extra_cfg.import_indexes_on_start {
            if let Err(e) = import_extra_indexes().await {
                tracing::error!("Initial index import failed: {}", e);
            }
        }

        if let Err(e) = schedule_idx_downloads().await {
            tracing::error!("Scheduled index download failed: {}", e);
        }
    });

    let app = create_router();

    // Bind to the host address with proper error handling
    let listener = match tokio::net::TcpListener::bind(&config.host).await {
        Ok(l) => l,
        Err(e) => {
            return Err(color_eyre::eyre::eyre!(
                "Failed to bind to {}: {}",
                config.host,
                e
            ));
        }
    };

    // Log the actual bound address
    match listener.local_addr() {
        Ok(addr) => tracing::info!("Listening on: {}", addr),
        Err(e) => tracing::warn!("Could not determine local address: {}", e),
    }

    // Start the server with proper error handling
    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Server error: {}", e);
        return Err(color_eyre::eyre::eyre!("Server error: {}", e));
    }

    Ok(())
}
