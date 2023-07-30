use server::Server;
use thiserror::Error;

use std::path::PathBuf;

use app::App;

mod app;
mod indexer;
mod server;
mod tv_maze;

#[derive(Error, Debug)]
enum ArgParseError {
    #[error("Unkonwn arg {0}")]
    UnknownArg(String),
    #[error("No port argument provided")]
    NoPort,
    #[error("No invalid port")]
    InvalidPort(#[source] std::num::ParseIntError),
    #[error("No show list argument provided")]
    NoShowList,
}

struct Args {
    html_path: Option<PathBuf>,
    port: i16,
    show_list: PathBuf,
}

impl Args {
    fn parse() -> Result<Args, ArgParseError> {
        let mut args = std::env::args();
        let _process_name = args.next();

        let mut html_path = None;
        let mut port = None;
        let mut show_list = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" => {
                    println!("{}", Self::help());
                    std::process::exit(1);
                }
                "--html-path" => {
                    html_path = args.next().map(Into::into);
                }
                "--port" => {
                    port = args.next().map(|s| s.parse());
                }
                "--show-list" => show_list = args.next().map(Into::into),
                _ => {
                    return Err(ArgParseError::UnknownArg(arg));
                }
            }
        }

        let port = port
            .ok_or(ArgParseError::NoPort)?
            .map_err(ArgParseError::InvalidPort)?;

        let show_list = show_list.ok_or(ArgParseError::NoShowList)?;

        let ret = Args {
            html_path,
            port,
            show_list,
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
                --show-list: List of shows to monitor, newline separated titles\n\
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

    let indexer = tv_maze::TvMazeIndexer::new();
    let app = App::new(args.show_list, indexer);
    let server = Server::new(args.html_path.as_deref(), app).unwrap();
    futures::executor::block_on(server.serve(args.port)).unwrap();
}
