use serde::Deserialize;
use tempfile::TempDir;
use thiserror::Error;
use tracing::info;

use std::{
    fs::File,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{app::App, indexer::Indexer};

#[derive(Error, Debug)]
pub enum ClientExtractionError {
    #[error("failed to create temp dir")]
    CreateDir(#[source] std::io::Error),
    #[error("failed to open tarball")]
    Open(#[source] std::io::Error),
    #[error("failed to unpack tarball")]
    Unpack(#[source] std::io::Error),
}

fn extract_client() -> Result<TempDir, ClientExtractionError> {
    use ClientExtractionError::*;
    let d = TempDir::new().map_err(CreateDir)?;
    let tarball_path = Path::new(env!("OUT_DIR")).join("client.tar");
    let tarball_reader = File::open(tarball_path).map_err(Open)?;
    let mut tarball = tar::Archive::new(tarball_reader);
    tarball.unpack(&d).map_err(Unpack)?;
    Ok(d)
}

async fn handle_episodes<T: Indexer>(
    req: tide::Request<Arc<Mutex<App<T>>>>,
) -> tide::Result<serde_json::Value> {
    let mut app = req.state().lock().expect("Invalid lock");
    Ok(serde_json::to_value(app.episodes())?)
}

#[derive(Deserialize)]
struct SearchQueryParams {
    query: String,
}

async fn handle_search<T: Indexer>(
    req: tide::Request<Arc<Mutex<App<T>>>>,
) -> tide::Result<serde_json::Value> {
    let mut app = req.state().lock().expect("Invalid lock");
    let query: SearchQueryParams = req.query()?;
    let results = app.search(&query.query)?;
    Ok(serde_json::to_value(results)?)
}

#[derive(Error, Debug)]
pub enum ServerCreationError {
    #[error("failed to extract client")]
    ExtractClient(#[from] ClientExtractionError),
    #[error("failed to serve directory")]
    ServeDir(#[source] std::io::Error),
}

pub struct Server<T: Indexer> {
    app: tide::Server<Arc<Mutex<App<T>>>>,
    _embedded_html_dir: TempDir,
}

impl<T: Indexer> Server<T> {
    pub fn new(data_path: Option<&Path>, app: App<T>) -> Result<Server<T>, ServerCreationError> {
        let mut app = tide::with_state(Arc::new(Mutex::new(app)));
        let embedded_html_dir = extract_client()?;

        app.at("/").get(tide::Redirect::new("/episodes.html"));
        if let Some(data_path) = data_path {
            info!("Overriding embedded html with {}", data_path.display());
            app.at("/")
                .serve_dir(data_path)
                .map_err(ServerCreationError::ServeDir)?;
        } else {
            app.at("/")
                .serve_dir(&embedded_html_dir)
                .map_err(ServerCreationError::ServeDir)?;
        }

        app.at("/episodes").get(handle_episodes);
        app.at("/search").get(handle_search);

        Ok(Server {
            app,
            _embedded_html_dir: embedded_html_dir,
        })
    }

    pub async fn serve(self, port: i16) -> std::io::Result<()> {
        self.app.listen(format!("127.0.0.1:{port}")).await
    }
}
