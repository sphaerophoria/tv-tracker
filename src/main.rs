#![deny(clippy::unwrap_used)]

use server::Server;
use thiserror::Error;

use std::path::PathBuf;

use app::App;
use db::Db;

mod app;
mod db;
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
}

struct Args {
    html_path: Option<PathBuf>,
    port: i16,
    db_path: PathBuf,
}

impl Args {
    fn parse() -> Result<Args, ArgParseError> {
        let mut args = std::env::args();
        let _process_name = args.next();

        let mut html_path = None;
        let mut db_path = None;
        let mut port = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" => {
                    println!("{}", Self::help());
                    std::process::exit(1);
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
                _ => {
                    return Err(ArgParseError::UnknownArg(arg));
                }
            }
        }

        let port = port
            .ok_or(ArgParseError::NoPort)?
            .map_err(ArgParseError::InvalidPort)?;

        let db_path = db_path.ok_or(ArgParseError::NoDbPath)?;

        let ret = Args {
            html_path,
            port,
            db_path,
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
                --html-path: Optional path to filesystem to serve html files from. Useful for \
                debugging\n\
                --db-path: Where to store database\n\
                --port: Port to serve UI on\n\
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

    let db = Db::new(&args.db_path).expect("Failed to create db");
    let app = App::new(db);
    let server = Server::new(args.html_path.as_deref(), app).expect("Failed to create server");
    futures::executor::block_on(server.serve(args.port)).expect("Failed to run server");
}
