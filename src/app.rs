use tracing::{error, info};

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use crate::indexer::{Indexer, TvEpisodes};

type TvEpisodeMap<T> = HashMap<String, TvEpisodes<T>>;
type SharedTvEpisodeMap<T> = Arc<Mutex<TvEpisodeMap<T>>>;

struct IndexPoller<I: Indexer> {
    show_list: PathBuf,
    episode_map: SharedTvEpisodeMap<I::EpisodeId>,
    indexer: I,
}

impl<I> IndexPoller<I>
where
    I: Indexer,
{
    fn poll(&mut self) {
        let mut ret = HashMap::new();

        let monitored_shows = match parse_show_list(&self.show_list) {
            Ok(v) => v,
            Err(e) => {
                error!("Failed to parse show list: {e}");
                return;
            }
        };

        for show_name in &monitored_shows {
            let possible_shows = match self.indexer.search(show_name) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to search for {show_name}: {e}");
                    continue;
                }
            };

            if possible_shows.is_empty() {
                error!("Failed to find {show_name}");
                continue;
            }

            let selected_show = &possible_shows[0];

            let episodes = match self.indexer.episodes(&selected_show.id) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to get episodes for {show_name}: {e}");
                    continue;
                }
            };

            ret.insert(selected_show.name.clone(), episodes);
        }

        *self.episode_map.lock().expect("Poisoned lock") = ret;
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

pub struct App<I: Indexer> {
    episode_map: SharedTvEpisodeMap<I::EpisodeId>,
}

impl<I: Indexer> App<I> {
    pub fn new(show_list: PathBuf, indexer: I) -> App<I> {
        let episode_map = Arc::new(Mutex::new(Default::default()));

        let mut poller = IndexPoller {
            show_list,
            episode_map: Arc::clone(&episode_map),
            indexer,
        };

        info!("Initializing episode map");
        poller.poll();

        std::thread::spawn(move || {
            poller.run();
        });

        App { episode_map }
    }

    pub fn episodes(&mut self) -> TvEpisodeMap<I::EpisodeId> {
        (*self.episode_map.lock().expect("Poisoned lock")).clone()
    }
}

fn parse_show_list(show_list: &Path) -> std::io::Result<Vec<String>> {
    let reader = BufReader::new(File::open(show_list)?);
    reader.lines().collect()
}
