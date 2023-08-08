use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    db::{self, AddShowError as DbAddShowError, Db, GetShowError},
    tv_maze::{self, TvMazeApiError, TvMazeShow, TvMazeShowId},
    types::{EpisodeId, ShowId, TvEpisode, TvShow},
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
}

#[derive(Debug, Error)]
pub enum GetShowWatchStatusError {
    #[error("failed to get shows")]
    Shows(#[source] db::GetShowError),
    #[error("failed to get episodes")]
    Episodes(#[source] db::GetEpisodeError),
    #[error("failed to get watch_statuses")]
    WatchStatus(#[source] db::GetWatchStatusError),
}

struct IndexPoller {
    inner: SharedInner,
}

impl IndexPoller {
    fn poll(&mut self) {
        let mut ret = HashMap::new();
        let monitored_shows = self.inner.lock().expect("Poisoned lock").db.get_shows();

        let monitored_shows = match monitored_shows {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to get monitored show list: {e}");
                return;
            }
        };

        for (show_id, indexer_id, _) in monitored_shows.iter() {
            let episodes = match tv_maze::episodes(indexer_id) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to get episodes for {}: {e}", show_id.0);
                    continue;
                }
            };

            ret.insert(*show_id, episodes);
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

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowWatchStatus {
    Finished,
    Unstarted,
    InProgress,
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

    pub fn add_show(&self, indexer_show_id: &TvMazeShowId) -> Result<(), AddShowError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        let existing_shows = inner.db.get_shows().map_err(AddShowError::LoadExisting)?;

        for (_, existing_indexer_id, _) in existing_shows {
            if existing_indexer_id == *indexer_show_id {
                return Err(AddShowError::ShowExists);
            }
        }

        let show = tv_maze::show(indexer_show_id).map_err(AddShowError::LookupShow)?;
        let episodes = tv_maze::episodes(indexer_show_id);
        let episodes = episodes.map_err(AddShowError::LookupEpisodes)?;

        let show_id = inner
            .db
            .add_show(&show, indexer_show_id)
            .map_err(AddShowError::AddShowToDb)?;

        for episode in episodes {
            if let Err(e) = inner.db.add_episode(&show_id, &episode) {
                error!("Failed to insert episode into db: {e}");
            }
        }

        Ok(())
    }

    pub fn shows(&self) -> Result<HashMap<ShowId, TvShow>, GetShowError> {
        let inner = self.inner.lock().expect("Poisoned lock");

        let shows = inner.db.get_shows()?;

        Ok(shows
            .into_iter()
            .map(|(show_id, _indexer_id, show)| (show_id, show))
            .collect())
    }

    pub fn episodes(
        &self,
        show_id: &ShowId,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, db::GetEpisodeError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        inner.db.get_episodes(show_id)
    }

    pub fn search(&self, query: &str) -> Result<Vec<TvMazeShow>, TvMazeApiError> {
        let results = tv_maze::search(query)?;
        Ok(results)
    }

    pub fn set_watch_status(
        &self,
        episode: &EpisodeId,
        status: Option<NaiveDate>,
    ) -> Result<(), db::SetWatchStatusError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        inner.db.set_episode_watch_status(episode, status)
    }

    pub fn get_watch_status(
        &self,
        show_id: &ShowId,
    ) -> Result<HashMap<EpisodeId, NaiveDate>, db::GetWatchStatusError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        inner.db.get_show_watch_status(show_id)
    }

    pub fn get_shows_by_watch_status(
        &self,
    ) -> Result<HashMap<ShowId, ShowWatchStatus>, GetShowWatchStatusError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        let mut ret = HashMap::new();
        let today = chrono::Local::now().date_naive();

        for (show_id, _, _) in inner
            .db
            .get_shows()
            .map_err(GetShowWatchStatusError::Shows)?
        {
            let watch_status = inner
                .db
                .get_show_watch_status(&show_id)
                .map_err(GetShowWatchStatusError::WatchStatus)?;

            if watch_status.is_empty() {
                ret.insert(show_id, ShowWatchStatus::Unstarted);
                continue;
            }

            let episodes = inner
                .db
                .get_episodes(&show_id)
                .map_err(GetShowWatchStatusError::Episodes)?;

            let aired_episodes: Vec<_> = episodes
                .into_iter()
                .filter(|(_id, epi)| {
                    let airdate = match epi.airdate {
                        Some(v) => v,
                        None => return false,
                    };
                    airdate <= today
                })
                .collect();

            if watch_status.len() >= aired_episodes.len() {
                ret.insert(show_id, ShowWatchStatus::Finished);
                continue;
            }

            ret.insert(show_id, ShowWatchStatus::InProgress);
        }

        Ok(ret)
    }

    pub fn set_show_pause_status(
        &self,
        show: &ShowId,
        pause: bool,
    ) -> Result<(), db::SetPauseError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        inner.db.set_pause_status(show, pause)
    }

    pub fn get_paused_shows(&self) -> Result<HashSet<ShowId>, db::GetPausedShowError> {
        let inner = self.inner.lock().expect("Poisoned lock");
        inner.db.get_paused_shows()
    }
}
