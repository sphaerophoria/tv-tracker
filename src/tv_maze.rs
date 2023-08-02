use chrono::Datelike;
use isahc::error::Error as IsahcError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

use std::io::Read;

use crate::indexer::{Indexer, TvEpisode, TvShow};

fn tvmaze_api_url(url: &str) -> String {
    const API_ROOT: &str = "https://api.tvmaze.com";
    format!("{API_ROOT}{url}")
}

type TvMazeSearchResult = Vec<TvMazeSearchItem>;

#[derive(Serialize, Deserialize, Debug)]
pub struct TvMazeShowId(i64);

#[derive(Serialize, Deserialize, Debug)]
struct TvMazeImage {
    medium: String,
    original: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct TvMazeShow {
    id: TvMazeShowId,
    name: String,
    premiered: Option<chrono::NaiveDate>,
    image: Option<TvMazeImage>,
    url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct TvMazeSearchItem {
    score: f32,
    show: TvMazeShow,
}

impl From<TvMazeSearchItem> for TvShow<TvMazeShowId> {
    fn from(value: TvMazeSearchItem) -> Self {
        TvShow {
            id: value.show.id,
            name: value.show.name,
            year: value.show.premiered.map(|d| d.year()),
            url: value.show.url,
            image: value.show.image.map(|i| i.medium),
        }
    }
}

type TvMazeEpisodes = Vec<TvMazeEpisode>;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TvMazeEpisodeId(i64);

#[derive(Serialize, Deserialize, Debug)]
struct TvMazeEpisode {
    id: TvMazeEpisodeId,
    name: String,
    season: i64,
    number: i64,
    airdate: chrono::NaiveDate,
}

impl From<TvMazeEpisode> for TvEpisode<TvMazeEpisodeId> {
    fn from(value: TvMazeEpisode) -> Self {
        TvEpisode {
            id: value.id,
            name: value.name,
            season: value.season,
            episode: value.number,
            airdate: value.airdate,
        }
    }
}

#[derive(Debug, Error)]
pub enum TvMazeApiError {
    #[error("failed to parse response")]
    Get(#[from] IsahcError),
    #[error("failed to read body")]
    Read(#[from] std::io::Error),
    #[error("failed to parse response")]
    Parse(#[from] serde_json::Error),
}

#[derive(Debug)]
pub struct TvMazeIndexer {}

impl TvMazeIndexer {
    pub fn new() -> TvMazeIndexer {
        TvMazeIndexer {}
    }

    pub fn send_request<T: DeserializeOwned>(&self, url: &str) -> Result<T, TvMazeApiError> {
        let url = tvmaze_api_url(url);
        debug!("Sending requsest to {url}");
        let mut response = isahc::get(url)?;
        let body = response.body_mut();

        let mut body_s = String::new();
        body.read_to_string(&mut body_s)?;

        debug!("Returned content {}", body_s);
        Ok(serde_json::from_str(&body_s)?)
    }
}

impl Indexer for TvMazeIndexer {
    type ShowId = TvMazeShowId;
    type EpisodeId = TvMazeEpisodeId;
    type Err = TvMazeApiError;

    fn search(&self, query: &str) -> Result<Vec<TvShow<Self::ShowId>>, Self::Err> {
        let query: &str = &urlencoding::encode(query);
        let shows: TvMazeSearchResult = self.send_request(&format!("/search/shows?q={query}"))?;
        Ok(shows.into_iter().map(Into::into).collect())
    }

    fn episodes(&self, show: &TvMazeShowId) -> Result<Vec<TvEpisode<Self::EpisodeId>>, Self::Err> {
        let episodes: TvMazeEpisodes = self.send_request(&format!("/shows/{}/episodes", show.0))?;
        Ok(episodes.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_search_deserialization() {
        let body = include_bytes!("../res/tv_maze/search_result_banshee.json");
        serde_json::from_slice::<TvMazeSearchResult>(body).expect("Failed to deserialize");

        let body = include_bytes!("../res/tv_maze/search_result_arcane.json");
        serde_json::from_slice::<TvMazeSearchResult>(body).expect("Failed to deserialize");
    }

    #[test]
    fn test_episodes_deserialization() {
        let body = include_bytes!("../res/tv_maze/episodes_result.json");
        serde_json::from_slice::<TvMazeEpisodes>(body).expect("Failed to deserialize");
    }
}
