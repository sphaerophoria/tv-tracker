use chrono::NaiveDate;
use serde::Deserialize;
use tempfile::TempDir;
use thiserror::Error;
use tracing::{error, info};

use std::{fs::File, path::Path};

use crate::{
    app::App,
    tv_maze::TvMazeShowId,
    types::{
        EpisodeId, ImageId, MovieId, MovieUpdate, Rating, RatingId, ShowId, TvShowUpdate,
        WatchStatus,
    },
};

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

async fn get_shows(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let app = req.state();
    Ok(serde_json::to_value(app.shows()?)?)
}

#[derive(Debug, Deserialize)]
struct PutShowsRequest {
    remote_id: TvMazeShowId,
}

async fn put_shows(mut req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let params: PutShowsRequest = req.body_json().await?;
    let app = req.state();
    let show = app.add_show(&params.remote_id)?;
    Ok(serde_json::to_value(show)?)
}

#[derive(Debug, Error)]
#[error("no show with id")]
struct NoShowWithId;

async fn get_show(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let app = req.state();
    let shows = app.shows()?;
    let show = shows.get(&ShowId(id)).ok_or(NoShowWithId)?;
    Ok(serde_json::to_value(show)?)
}

#[derive(Debug, Error)]
#[error("id does not match URI")]
struct NonMatchingId;

async fn put_show(mut req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = ShowId(id);

    let params: TvShowUpdate = req.body_json().await?;
    if params.id != id {
        return Err(NonMatchingId.into());
    }

    let app = req.state();
    let show = app.update_show(&params)?;
    Ok(serde_json::to_value(show)?)
}

async fn delete_show(req: tide::Request<App>) -> tide::Result<tide::StatusCode> {
    let id: i64 = req.param("id")?.parse()?;
    let app = req.state();
    app.remove_show(&ShowId(id))?;
    Ok(tide::StatusCode::Ok)
}

async fn get_episodes_for_show(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = ShowId(id);

    let app = req.state();
    Ok(serde_json::to_value(app.episodes_for_show(&id)?)?)
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

#[derive(Deserialize)]
struct GetEpisodesQueryParams {
    start_date: NaiveDate,
    end_date: NaiveDate,
}

async fn get_episodes(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let params: GetEpisodesQueryParams = req.query()?;
    let app = req.state();
    let ret = app.get_episodes_aired_between(&params.start_date, &params.end_date)?;
    Ok(serde_json::to_value(ret)?)
}

async fn get_episode(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = EpisodeId(id);

    let app = req.state();
    let ret = app.get_episode(&id)?;
    Ok(serde_json::to_value(ret)?)
}

#[derive(Debug, Deserialize)]
struct PutEpisodeRequest {
    id: EpisodeId,
    watch_status: WatchStatus,
}

async fn put_episode(mut req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = EpisodeId(id);

    let params: PutEpisodeRequest = req.body_json().await?;
    if params.id != id {
        return Err(NonMatchingId.into());
    }

    let app = req.state();
    let response = app.set_watch_status(&id, &params.watch_status)?;
    Ok(serde_json::to_value(response)?)
}

async fn get_ratings(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let app = req.state();
    Ok(serde_json::to_value(app.get_ratings()?)?)
}

async fn get_rating(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id = req.param("id")?.parse()?;
    let id = RatingId(id);

    let app = req.state();
    Ok(serde_json::to_value(app.get_rating(&id)?)?)
}

async fn get_image(req: tide::Request<App>) -> tide::Result<tide::Body> {
    let id = req.param("id")?.parse()?;
    let id = ImageId(id);

    let app = req.state();
    let body = tide::Body::from_bytes(app.get_image(&id)?);
    Ok(body)
}

#[derive(Debug, Deserialize)]
struct SetRatingsRequest {
    name: String,
}

async fn put_ratings(mut req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let request: SetRatingsRequest = req.body_json().await?;
    let app = req.state();
    Ok(serde_json::to_value(app.add_rating(&request.name)?)?)
}

async fn put_rating(mut req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = RatingId(id);

    let params: Rating = req.body_json().await?;
    if params.id != id {
        return Err(NonMatchingId.into());
    }

    let app = req.state();
    Ok(serde_json::to_value(app.update_rating(&params)?)?)
}

async fn delete_rating(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = RatingId(id);
    let app = req.state();
    Ok(serde_json::to_value(app.delete_rating(&id)?)?)
}

#[derive(Debug, Deserialize)]
struct PutMoviesRequest {
    imdb_id: String,
}

async fn get_movies(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let app = req.state();
    let movies = app.get_movies()?;
    Ok(serde_json::to_value(movies)?)
}

async fn put_movies(mut req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let params: PutMoviesRequest = req.body_json().await?;
    let app = req.state();
    let movie = app.add_movie(&params.imdb_id)?;
    Ok(serde_json::to_value(movie)?)
}

async fn get_movie(req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = MovieId(id);

    let app = req.state();
    let movie = app.get_movie(&id)?;
    Ok(serde_json::to_value(movie)?)
}

async fn put_movie(mut req: tide::Request<App>) -> tide::Result<serde_json::Value> {
    let id: i64 = req.param("id")?.parse()?;
    let id = MovieId(id);

    let params: MovieUpdate = req.body_json().await?;
    if params.id != id {
        return Err(NonMatchingId.into());
    }

    let app = req.state();
    Ok(serde_json::to_value(app.update_movie(&params)?)?)
}

async fn delete_movie(req: tide::Request<App>) -> tide::Result<tide::StatusCode> {
    let id: i64 = req.param("id")?.parse()?;
    let id = MovieId(id);

    let app = req.state();
    app.delete_movie(&id)?;
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

        app.with(tide::utils::After(|res: tide::Response| async {
            if let Some(err) = res.error() {
                error!("{:?}", err);
            }
            Ok(res)
        }));
        app.at("/").get(tide::Redirect::new("/watch_list.html"));
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

        app.at("/shows").get(get_shows).put(put_shows);

        app.at("/shows/:id")
            .get(get_show)
            .put(put_show)
            .delete(delete_show);

        app.at("/shows/:id/episodes").get(get_episodes_for_show);
        app.at("/search").get(handle_search);
        app.at("/episodes").get(get_episodes);

        app.at("/episodes/:id").get(get_episode).put(put_episode);

        app.at("/ratings").get(get_ratings).put(put_ratings);

        app.at("/ratings/:id")
            .get(get_rating)
            .put(put_rating)
            .delete(delete_rating);

        app.at("/images/:id").get(get_image);

        app.at("/movies").get(get_movies).put(put_movies);

        app.at("/movies/:id")
            .get(get_movie)
            .put(put_movie)
            .delete(delete_movie);

        Ok(Server {
            app,
            _embedded_html_dir: embedded_html_dir,
        })
    }

    pub async fn serve(self, port: i16) -> std::io::Result<()> {
        self.app.listen(format!("0.0.0.0:{port}")).await
    }
}
