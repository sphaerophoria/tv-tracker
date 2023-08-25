#![deny(clippy::unwrap_used)]

use image_cache::ImageCache;
use omdb::OmdbIndexer;
use server::Server;

use thiserror::Error;

use std::path::PathBuf;

use app::App;
use db::Db;

mod app;
mod db;
mod image_cache;
mod omdb;
mod server;
mod tv_maze;
mod types;

#[derive(Error, Debug)]
enum ArgParseError {
    #[error("Unkonwn arg {0}")]
    UnknownArg(String),
    #[error("No port argument provided")]
    NoPort,
    #[error("No invalid port")]
    InvalidPort(#[source] std::num::ParseIntError),
    #[error("No db path provided")]
    NoDbPath,
    #[error("No cache path provided")]
    NoCachePath,
    #[error("No omdb api key provided")]
    NoOmdbApiKeyProvided,
}

struct Args {
    html_path: Option<PathBuf>,
    port: i16,
    db_path: PathBuf,
    cache_path: PathBuf,
    omdb_key_path: PathBuf,
    poll_indexers: bool,
}

impl Args {
    fn parse() -> Result<Args, ArgParseError> {
        let mut args = std::env::args();
        let _process_name = args.next();

        let mut html_path = None;
        let mut db_path = None;
        let mut port = None;
        let mut cache_path = None;
        let mut omdb_key_path = None;
        let mut poll_indexers = true;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" => {
                    println!("{}", Self::help());
                    std::process::exit(1);
                }
                "--cache-path" => {
                    cache_path = args.next().map(Into::into);
                }
                "--html-path" => {
                    html_path = args.next().map(Into::into);
                }
                "--db-path" => {
                    db_path = args.next().map(Into::into);
                }
                "--port" => {
                    port = args.next().map(|s| s.parse());
                }
                "--omdb-key-path" => {
                    omdb_key_path = args.next().map(Into::into);
                }
                "--no-poll" => {
                    poll_indexers = false;
                }
                _ => {
                    return Err(ArgParseError::UnknownArg(arg));
                }
            }
        }

        let port = port
            .ok_or(ArgParseError::NoPort)?
            .map_err(ArgParseError::InvalidPort)?;
        let db_path = db_path.ok_or(ArgParseError::NoDbPath)?;
        let cache_path = cache_path.ok_or(ArgParseError::NoCachePath)?;
        let omdb_key_path = omdb_key_path.ok_or(ArgParseError::NoOmdbApiKeyProvided)?;

        let ret = Args {
            html_path,
            port,
            db_path,
            cache_path,
            omdb_key_path,
            poll_indexers,
        };

        Ok(ret)
    }

    fn help() -> String {
        let process_name = std::env::args()
            .next()
            .unwrap_or_else(|| "tv-tracker".to_string());

        format!(
            "Track your tv watching\n\
                \n\
                Usage: {process_name} [ARGS]\n\
                \n\
                Args:\n\
                --help: Show this help\n\
                --cache-path: Where to cache assets retrieved from remote\n\
                --html-path: Optional path to filesystem to serve html files from. Useful for \
                debugging\n\
                --db-path: Where to store database\n\
                --port: Port to serve UI on\n\
                --omdb-key-path: Path to file containing omdb api key\n\
                --no-poll: Optional, when passed will not poll remote indexers for new data\n\
                "
        )
    }
}

fn main() {
    tracing_subscriber::fmt::init();

    let args = match Args::parse() {
        Ok(v) => v,
        Err(e) => {
            println!("{}", e);
            println!();
            println!("{}", Args::help());
            return;
        }
    };

    let omdb_key = std::fs::read_to_string(args.omdb_key_path).expect("Failed to read omdb key");
    let omdb_indexer = OmdbIndexer::new(omdb_key.trim().to_string());
    let db = Db::new(&args.db_path).expect("Failed to create db");
    let poster_cache = ImageCache::new(args.cache_path.join("posters"));
    let app = App::new(db, omdb_indexer, poster_cache, args.poll_indexers);
    let server = Server::new(args.html_path.as_deref(), app).expect("Failed to create server");
    futures::executor::block_on(server.serve(args.port)).expect("Failed to run server");
}
