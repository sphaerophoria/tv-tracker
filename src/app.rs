use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    db::{self, AddShowError as DbAddShowError, Db, GetShowError},
    image_cache::{self, ImageCache},
    omdb::{OmdbError, OmdbIndexer},
    tv_maze::{self, TvMazeApiError, TvMazeShowId},
    types::{
        EpisodeId, ImageId, Movie, MovieId, MovieUpdate, Rating, RatingId, RemoteMovie,
        RemoteTvShow, ShowId, TvEpisode, TvShow, TvShowUpdate,
    },
};

#[derive(Debug, Error)]
pub enum AddShowError {
    #[error("failed to load existing show list")]
    LoadExisting(#[source] GetShowError),
    #[error("show already monitored")]
    ShowExists,
    #[error("failed to lookup show")]
    LookupShow(#[source] TvMazeApiError),
    #[error("failed to lookup episodes")]
    LookupEpisodes(#[source] TvMazeApiError),
    #[error("failed to add show to db")]
    AddShowToDb(#[source] DbAddShowError),
    #[error("failed to get show after add")]
    GetShow(#[source] db::GetShowError),
}

#[derive(Debug, Error)]
pub enum UpdateShowError {
    #[error("failed to set pause status in db")]
    SetPause(#[from] db::SetPauseError),
    #[error("failed to set rating in db")]
    SetRating(#[from] db::SetShowRatingError),
    #[error("failed to get shows after modification")]
    GetShows(#[from] db::GetShowError),
}

#[derive(Debug, Error)]
pub enum UpdateMovieError {
    #[error("failed to set watch status in db")]
    SetWatchStatus(#[from] db::SetWatchStatusError),
    #[error("failed to set rating in db")]
    SetRating(#[from] db::SetShowRatingError),
    #[error("failed to get movie after modification")]
    GetMovie(#[from] db::GetMovieError),
}

#[derive(Debug, Error)]
pub enum SetWatchStatusError {
    #[error("failed to set watch status in db")]
    Db(#[from] db::SetWatchStatusError),
    #[error("failed to get episode after modification")]
    GetEpisode(#[from] db::GetEpisodeError),
}

#[derive(Debug, Error)]
pub enum GetRatingError {
    #[error("failed to get ratings from db")]
    Db(#[from] db::GetRatingsError),
}

#[derive(Debug, Error)]
pub enum AddRatingError {
    #[error("failed to add rating to db")]
    Add(#[from] db::AddRatingError),
    #[error("failed to get ratings from db")]
    Retrieve(#[from] db::GetRatingsError),
}

#[derive(Debug, Error)]
pub enum UpdateRatingError {
    #[error("failed to update rating in db")]
    UpdateRating(#[from] rusqlite::Error),
    #[error("failed to get ratings from db")]
    GetRatings(#[from] db::GetRatingsError),
}

#[derive(Debug, Error)]
pub enum GetImageError {
    #[error("failed to get image url from db")]
    GetImageUrl(#[from] db::GetImageUrlError),
    #[error("failed to get image")]
    GetImage(#[from] image_cache::GetImageError),
}

#[derive(Debug, Error)]
pub enum AddMovieError {
    #[error("failed to get movie info from remote")]
    Remote(#[source] OmdbError),
    #[error("failed add movie to databse")]
    AddToDb(#[source] db::AddMovieError),
    #[error("failed to get inserted movie from database")]
    GetInserted(#[source] db::GetMovieError),
}

struct IndexPoller {
    inner: SharedInner,
}

impl IndexPoller {
    fn poll(&mut self) {
        let mut ret = HashMap::new();
        let monitored_shows = self
            .inner
            .db
            .lock()
            .expect("Poisoned lock")
            .get_shows(&today());

        let monitored_shows = match monitored_shows {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to get monitored show list: {e}");
                return;
            }
        };

        for show in monitored_shows.values() {
            let episodes = match tv_maze::episodes(&show.remote_id) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to get episodes for {}: {e}", show.id.0);
                    continue;
                }
            };

            ret.insert(show.id, episodes);
        }

        let mut db = self.inner.db.lock().expect("Poisoned lock");
        for (show_id, episodes) in ret {
            for episode in episodes {
                if let Err(e) = db.add_episode(&show_id, &episode) {
                    error!("Failed to add episode: {e}");
                }
            }
        }
    }

    fn run(mut self) {
        loop {
            info!("Updating episode map");
            self.poll();

            const DAY_IN_SECONDS: u64 = 24 * 60 * 60;
            std::thread::sleep(Duration::from_secs(DAY_IN_SECONDS));
        }
    }
}

fn db_contains_show_id(
    db: &mut Db,
    indexer_show_id: &TvMazeShowId,
) -> Result<bool, db::GetShowError> {
    let existing_shows = db.get_shows(&today())?;

    for show in existing_shows.values() {
        if show.remote_id == *indexer_show_id {
            return Ok(true);
        }
    }

    Ok(false)
}

#[derive(Serialize, Deserialize)]
pub struct SearchResults {
    movies: Vec<RemoteMovie>,
    shows: Vec<RemoteTvShow<TvMazeShowId>>,
}

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("failed to search tv shows")]
    Tv(#[from] TvMazeApiError),
    #[error("failed to search movies")]
    Movie(#[from] OmdbError),
}

pub struct Inner {
    db: Mutex<Db>,
    image_cache: ImageCache,
    omdb_indexer: OmdbIndexer,
}

type SharedInner = Arc<Inner>;

#[derive(Clone)]
pub struct App {
    inner: SharedInner,
}

impl App {
    pub fn new(
        db: Db,
        omdb_indexer: OmdbIndexer,
        image_cache: ImageCache,
        poll_indexers: bool,
    ) -> App {
        let inner = Inner {
            db: Mutex::new(db),
            omdb_indexer,
            image_cache,
        };

        let inner = Arc::new(inner);

        if poll_indexers {
            let poller = IndexPoller {
                inner: Arc::clone(&inner),
            };

            std::thread::spawn(move || {
                poller.run();
            });
        }

        App { inner }
    }

    pub fn add_show(&self, indexer_show_id: &TvMazeShowId) -> Result<TvShow, AddShowError> {
        let res = db_contains_show_id(
            &mut self.inner.db.lock().expect("Poisoned lock"),
            indexer_show_id,
        );

        if res.map_err(AddShowError::LoadExisting)? {
            return Err(AddShowError::ShowExists);
        }

        let show = tv_maze::show(indexer_show_id).map_err(AddShowError::LookupShow)?;
        let episodes = tv_maze::episodes(indexer_show_id);
        let episodes = episodes.map_err(AddShowError::LookupEpisodes)?;

        let mut db = self.inner.db.lock().expect("Poisoned lock");
        let show_id = db.add_show(&show).map_err(AddShowError::AddShowToDb)?;

        for episode in episodes {
            if let Err(e) = db.add_episode(&show_id, &episode) {
                error!("Failed to insert episode into db: {e}");
            }
        }

        db.get_show(&show_id, &today())
            .map_err(AddShowError::GetShow)
    }

    pub fn remove_show(&self, show_id: &ShowId) -> Result<(), db::RemoveShowError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        db.remove_show(show_id)
    }

    pub fn shows(&self) -> Result<HashMap<ShowId, TvShow>, GetShowError> {
        let db = self.inner.db.lock().expect("Poisoned lock");
        let shows = db.get_shows(&today())?;
        Ok(shows)
    }

    pub fn get_episode(&self, episode_id: &EpisodeId) -> Result<TvEpisode, db::GetEpisodeError> {
        let db = self.inner.db.lock().expect("Poisoned lock");
        db.get_episode(episode_id)
    }

    pub fn episodes_for_show(
        &self,
        show_id: &ShowId,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, db::GetEpisodeError> {
        let db = self.inner.db.lock().expect("Poisoned lock");
        db.get_episodes_for_show(show_id)
    }

    pub fn search(&self, query: &str) -> Result<SearchResults, SearchError> {
        let shows = tv_maze::search(query)?;
        let movies = self.inner.omdb_indexer.search(query)?;
        Ok(SearchResults { movies, shows })
    }

    pub fn set_watch_status(
        &self,
        episode: &EpisodeId,
        status: &Option<NaiveDate>,
    ) -> Result<TvEpisode, SetWatchStatusError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        db.set_episode_watch_status(episode, status)?;
        Ok(db.get_episode(episode)?)
    }

    pub fn update_show(&self, show: &TvShowUpdate) -> Result<TvShow, UpdateShowError> {
        let db = self.inner.db.lock().expect("Poisoned lock");
        if let Some(pause_status) = &show.pause_status {
            db.set_pause_status(&show.id, *pause_status)?;
        }

        db.set_show_rating(&show.id, &show.rating_id)?;

        Ok(db.get_show(&show.id, &today())?)
    }

    pub fn get_episodes_aired_between(
        &self,
        start_date: &NaiveDate,
        end_date: &NaiveDate,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, db::GetEpisodeError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        db.get_episodes_aired_between(start_date, end_date)
    }

    pub fn add_rating(&self, name: &str) -> Result<Rating, AddRatingError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        let id = db.add_rating(name)?;
        Ok(db.get_rating(&id)?)
    }

    pub fn get_ratings(&self) -> Result<HashMap<RatingId, Rating>, db::GetRatingsError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        db.get_ratings()
    }

    pub fn get_rating(&self, id: &RatingId) -> Result<Rating, GetRatingError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        Ok(db.get_rating(id)?)
    }

    pub fn delete_rating(&self, id: &RatingId) -> Result<(), db::DeleteRatingError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        db.delete_rating(id)?;
        Ok(())
    }

    pub fn update_rating(&self, rating: &Rating) -> Result<Rating, UpdateRatingError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        db.update_rating(rating)?;
        Ok(db.get_rating(&rating.id)?)
    }

    pub fn get_image(&self, id: &ImageId) -> Result<Vec<u8>, GetImageError> {
        let db = self.inner.db.lock().expect("Poisoned lock");
        let url = db.get_image_url(id)?;
        Ok(self.inner.image_cache.get(&url)?)
    }

    pub fn add_movie(&self, imdb_id: &str) -> Result<Movie, AddMovieError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        let movie = self
            .inner
            .omdb_indexer
            .get_by_id(imdb_id)
            .map_err(AddMovieError::Remote)?;

        let movie_id = db.add_movie(&movie).map_err(AddMovieError::AddToDb)?;

        let ret = db
            .get_movie(&movie_id)
            .map_err(AddMovieError::GetInserted)?;

        Ok(ret)
    }

    pub fn get_movie(&self, id: &MovieId) -> Result<Movie, db::GetMovieError> {
        let db = self.inner.db.lock().expect("Poisoned lock");

        let ret = db.get_movie(id)?;

        Ok(ret)
    }

    pub fn get_movies(&self) -> Result<Vec<Movie>, db::GetMovieError> {
        let db = self.inner.db.lock().expect("Poisoned lock");
        let ret = db.get_movies()?;

        Ok(ret)
    }

    pub fn update_movie(&self, movie: &MovieUpdate) -> Result<Movie, UpdateMovieError> {
        let db = self.inner.db.lock().expect("Poisoned lock");

        let watch_date = match movie.watched {
            true => Some(today()),
            false => None,
        };
        db.set_movie_watch_status(&movie.id, &watch_date)?;
        db.set_movie_rating(&movie.id, &movie.rating_id)?;

        Ok(db.get_movie(&movie.id)?)
    }

    pub fn delete_movie(&self, id: &MovieId) -> Result<(), db::DeleteMovieError> {
        let mut db = self.inner.db.lock().expect("Poisoned lock");
        db.delete_movie(id)?;
        Ok(())
    }
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
