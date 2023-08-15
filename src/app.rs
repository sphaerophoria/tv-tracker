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
    tv_maze::{self, TvMazeApiError, TvMazeShowId},
    types::{EpisodeId, Rating, RatingId, RemoteTvShow, ShowId, TvEpisode, TvShow, TvShowUpdate},
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

struct IndexPoller {
    inner: SharedInner,
}

impl IndexPoller {
    fn poll(&mut self) {
        let mut ret = HashMap::new();
        let monitored_shows = self
            .inner
            .lock()
            .expect("Poisoned lock")
            .db
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

        let mut inner = self.inner.lock().expect("Poisoned lock");
        for (show_id, episodes) in ret {
            for episode in episodes {
                if let Err(e) = inner.db.add_episode(&show_id, &episode) {
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

#[derive(Serialize, Deserialize)]
pub struct SearchResults {
    id: TvMazeShowId,
    shows: Vec<TvShow>,
}

pub struct Inner {
    db: Db,
}

type SharedInner = Arc<Mutex<Inner>>;

#[derive(Clone)]
pub struct App {
    inner: SharedInner,
}

impl App {
    pub fn new(db: Db) -> App {
        let inner = Inner { db };

        let inner = Arc::new(Mutex::new(inner));

        let poller = IndexPoller {
            inner: Arc::clone(&inner),
        };

        std::thread::spawn(move || {
            poller.run();
        });

        App { inner }
    }

    pub fn add_show(&self, indexer_show_id: &TvMazeShowId) -> Result<TvShow, AddShowError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        let existing_shows = inner
            .db
            .get_shows(&today())
            .map_err(AddShowError::LoadExisting)?;

        for show in existing_shows.values() {
            if show.remote_id == *indexer_show_id {
                return Err(AddShowError::ShowExists);
            }
        }

        let show = tv_maze::show(indexer_show_id).map_err(AddShowError::LookupShow)?;
        let episodes = tv_maze::episodes(indexer_show_id);
        let episodes = episodes.map_err(AddShowError::LookupEpisodes)?;

        let show_id = inner
            .db
            .add_show(&show)
            .map_err(AddShowError::AddShowToDb)?;

        for episode in episodes {
            if let Err(e) = inner.db.add_episode(&show_id, &episode) {
                error!("Failed to insert episode into db: {e}");
            }
        }

        inner
            .db
            .get_show(&show_id, &today())
            .map_err(AddShowError::GetShow)
    }

    pub fn remove_show(&self, show_id: &ShowId) -> Result<(), db::RemoveShowError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        inner.db.remove_show(show_id)
    }

    pub fn shows(&self) -> Result<HashMap<ShowId, TvShow>, GetShowError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        let shows = inner.db.get_shows(&today())?;
        Ok(shows)
    }

    pub fn get_episode(&self, episode_id: &EpisodeId) -> Result<TvEpisode, db::GetEpisodeError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        inner.db.get_episode(episode_id)
    }

    pub fn episodes_for_show(
        &self,
        show_id: &ShowId,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, db::GetEpisodeError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        inner.db.get_episodes_for_show(show_id)
    }

    pub fn search(&self, query: &str) -> Result<Vec<RemoteTvShow<TvMazeShowId>>, TvMazeApiError> {
        let results = tv_maze::search(query)?;
        Ok(results)
    }

    pub fn set_watch_status(
        &self,
        episode: &EpisodeId,
        status: &Option<NaiveDate>,
    ) -> Result<TvEpisode, SetWatchStatusError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        inner.db.set_episode_watch_status(episode, status)?;
        Ok(inner.db.get_episode(episode)?)
    }

    pub fn update_show(&self, show: &TvShowUpdate) -> Result<TvShow, UpdateShowError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        if let Some(pause_status) = &show.pause_status {
            inner.db.set_pause_status(&show.id, *pause_status)?;
        }

        inner.db.set_show_rating(&show.id, &show.rating_id)?;

        Ok(inner.db.get_show(&show.id, &today())?)
    }

    pub fn get_episodes_aired_between(
        &self,
        start_date: &NaiveDate,
        end_date: &NaiveDate,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, db::GetEpisodeError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        inner.db.get_episodes_aired_between(start_date, end_date)
    }

    pub fn add_rating(&self, name: &str) -> Result<Rating, AddRatingError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        let id = inner.db.add_rating(name)?;
        Ok(inner.db.get_rating(&id)?)
    }

    pub fn get_ratings(&self) -> Result<HashMap<RatingId, Rating>, db::GetRatingsError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        inner.db.get_ratings()
    }

    pub fn get_rating(&self, id: &RatingId) -> Result<Rating, GetRatingError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        Ok(inner.db.get_rating(id)?)
    }

    pub fn delete_rating(&self, id: &RatingId) -> Result<(), db::DeleteRatingError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        inner.db.delete_rating(id)?;
        Ok(())
    }

    pub fn update_rating(&self, rating: &Rating) -> Result<Rating, UpdateRatingError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        inner.db.update_rating(rating)?;
        Ok(inner.db.get_rating(&rating.id)?)
    }
}

fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}
