use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, info};

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::{
    db::{AddShowError as DbAddShowError, Db, GetShowError},
    tv_maze::{self, TvMazeApiError, TvMazeShow, TvMazeShowId},
    types::{ShowId, TvEpisode, TvShow},
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

        for (show_id, indexer_id) in monitored_shows.iter() {
            let episodes = match tv_maze::episodes(indexer_id) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to get episodes for {}: {e}", show_id.0);
                    continue;
                }
            };

            ret.insert(*show_id, episodes);
        }

        self.inner.lock().expect("Poisoned lock").episodes = ret;
    }

    fn run(mut self) {
        loop {
            const HOUR_IN_SECONDS: u64 = 60 * 60;
            std::thread::sleep(Duration::from_secs(HOUR_IN_SECONDS));

            info!("Updating episode map");
            self.poll();
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
    episodes: HashMap<ShowId, Vec<TvEpisode>>,
}

type SharedInner = Arc<Mutex<Inner>>;

#[derive(Clone)]
pub struct App {
    inner: SharedInner,
}

impl App {
    pub fn new(db: Db) -> App {
        let inner = Inner {
            db,
            episodes: Default::default(),
        };

        let inner = Arc::new(Mutex::new(inner));

        let mut poller = IndexPoller {
            inner: Arc::clone(&inner),
        };

        info!("Initializing episode map");
        poller.poll();

        std::thread::spawn(move || {
            poller.run();
        });

        App { inner }
    }

    pub fn add_show(&self, indexer_show_id: &TvMazeShowId) -> Result<(), AddShowError> {
        let mut inner = self.inner.lock().expect("Poisoned lock");
        let existing_shows = inner.db.get_shows().map_err(AddShowError::LoadExisting)?;

        for (_, existing_indexer_id) in existing_shows {
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

        inner.episodes.insert(show_id, episodes);

        Ok(())
    }

    pub fn shows(&self) -> Result<HashMap<ShowId, TvShow>, GetShowError> {
        let inner = self.inner.lock().expect("Poisoned lock");

        let shows = inner.db.get_shows()?;

        let mut ret = HashMap::new();
        for k in shows.keys() {
            ret.insert(*k, inner.db.get_show(k)?);
        }

        Ok(ret)
    }

    pub fn episodes(&self) -> HashMap<ShowId, Vec<TvEpisode>> {
        self.inner.lock().expect("Poisoned lock").episodes.clone()
    }

    pub fn search(&self, query: &str) -> Result<Vec<TvMazeShow>, TvMazeApiError> {
        let results = tv_maze::search(query)?;
        Ok(results)
    }
}
