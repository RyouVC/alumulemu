mod db;
mod index;
mod nsp;
mod router;
mod titledb;

use db::init_database;
use router::create_router;
use surrealdb::Surreal;
use surrealdb::engine::local::RocksDb;
use titledb::TitleDBImport;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    tracing_subscriber::fmt::init();
    color_eyre::install().unwrap();

    // create games directory
    if !std::path::Path::new("games/").exists() {
        std::fs::create_dir("games/").unwrap();
        println!("Directory 'games/' does not exist, creating...");
    } else {
        println!("Directory 'games/' already exists, skipping...");
    }

    // initialize database
    //
    // todo: support any backend but use rocksdb://database by default
    // let db = Surreal::new::<RocksDb>("database").await?;
    // db.use_ns("tinfoil").use_db("games").await?;

    // get the web thing working
    init_database().await?;
    tokio::spawn(async {
        tracing::info!("Importing TitleDB...");
        let us_titledb_file = std::fs::File::open("US.en.json").unwrap();
        // let us_titledb_file = std::fs::File::open("src/zeld.json").unwrap();
        // match TitleDBImport::from_json_reader(us_titledb_file) {
        //     Ok(us_titledb) => {
        //         if let Err(e) = us_titledb.import_to_db("US-en").await {
        //             eprintln!("Error importing to DB: {}", e);
        //         }
        //     },
        //     Err(e) => eprintln!("Error reading titledb: {}", e),
        // },
        let a = TitleDBImport::from_json_reader_streaming(us_titledb_file, "US-en").await;
    });

    let app = create_router();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
