use serde::Deserialize;
use tempfile::TempDir;
use thiserror::Error;
use tracing::info;

use std::{fs::File, path::Path};

use crate::{app::App, tv_maze::TvMazeShowId};

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

async fn handle_shows(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let app = req.state();
    Ok(serde_json::to_value(app.shows()?)?)
}

async fn handle_episodes(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let app = req.state();
    Ok(serde_json::to_value(app.episodes())?)
}

#[derive(Deserialize)]
struct SearchQueryParams {
    query: String,
}

async fn handle_search(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let app = req.state();
    let query: SearchQueryParams = req.query()?;
    let results = app.search(&query.query)?;
    Ok(serde_json::to_value(results)?)
}

#[derive(Debug, Deserialize)]
struct AddRequest {
    id: TvMazeShowId,
}

async fn handle_add(mut req: tide::Request<App>) -> tide::Result<tide::StatusCode> {
    let request: AddRequest = req.body_json().await?;
    let app = req.state();
    app.add_show(&request.id)?;
    Ok(tide::StatusCode::Ok)
}

#[derive(Error, Debug)]
pub enum ServerCreationError {
    #[error("failed to extract client")]
    ExtractClient(#[from] ClientExtractionError),
    #[error("failed to serve directory")]
    ServeDir(#[source] std::io::Error),
}

pub struct Server {
    app: tide::Server<App>,
    _embedded_html_dir: TempDir,
}

impl Server {
    pub fn new(data_path: Option<&Path>, app: App) -> Result<Server, ServerCreationError> {
        let mut app = tide::with_state(app);
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

        app.at("/shows").get(handle_shows);
        app.at("/episodes").get(handle_episodes);
        app.at("/search").get(handle_search);
        app.at("/add_show").put(handle_add);

        Ok(Server {
            app,
            _embedded_html_dir: embedded_html_dir,
        })
    }

    pub async fn serve(self, port: i16) -> std::io::Result<()> {
        self.app.listen(format!("127.0.0.1:{port}")).await
    }
}
