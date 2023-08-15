use chrono::{Datelike, NaiveDate};
use isahc::error::Error as IsahcError;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

use std::io::Read;

use crate::types::{ImdbShowId, RemoteEpisode, RemoteTvShow, TvdbShowId};

fn tvmaze_api_url(url: &str) -> String {
    const API_ROOT: &str = "https://api.tvmaze.com";
    format!("{API_ROOT}{url}")
}

#[derive(Clone, Serialize, Deserialize, Debug, Eq, PartialEq)]
pub struct TvMazeShowId(pub i64);

#[derive(Serialize, Deserialize, Debug)]
struct ApiImage {
    medium: String,
    original: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ApiExternals {
    thetvdb: Option<TvdbShowId>,
    imdb: Option<ImdbShowId>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ApiShow {
    id: TvMazeShowId,
    name: String,
    premiered: Option<chrono::NaiveDate>,
    image: Option<ApiImage>,
    url: Option<String>,
    externals: Option<ApiExternals>,
}

impl From<ApiShow> for RemoteTvShow<TvMazeShowId> {
    fn from(value: ApiShow) -> Self {
        let (imdb_id, tvdb_id) = match value.externals {
            Some(v) => (v.imdb, v.thetvdb),
            None => (None, None),
        };

        RemoteTvShow {
            id: value.id,
            name: value.name,
            year: value.premiered.map(|d| d.year()),
            url: value.url,
            image: value.image.map(|i| i.medium),
            imdb_id,
            tvdb_id,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
struct ApiSearchItem {
    score: f32,
    show: ApiShow,
}

impl From<ApiSearchItem> for RemoteTvShow<TvMazeShowId> {
    fn from(value: ApiSearchItem) -> Self {
        value.show.into()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TvMazeEpisodeId(i64);

#[derive(Serialize, Deserialize, Debug)]
struct ApiEpisode {
    id: TvMazeEpisodeId,
    name: String,
    season: i64,
    number: i64,
    airdate: String,
}

impl From<ApiEpisode> for RemoteEpisode {
    fn from(value: ApiEpisode) -> Self {
        let airdate = NaiveDate::parse_from_str(&value.airdate, "%Y-%m-%d").ok();
        RemoteEpisode {
            name: value.name,
            season: value.season,
            episode: value.number,
            airdate,
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

fn send_request<T: DeserializeOwned>(url: &str) -> Result<T, TvMazeApiError> {
    let url = tvmaze_api_url(url);
    debug!("Sending requsest to {url}");
    let mut response = isahc::get(url)?;
    let body = response.body_mut();

    let mut body_s = String::new();
    body.read_to_string(&mut body_s)?;

    debug!("Returned content {}", body_s);
    Ok(serde_json::from_str(&body_s)?)
}

pub fn show(id: &TvMazeShowId) -> Result<RemoteTvShow<TvMazeShowId>, TvMazeApiError> {
    let show: ApiShow = send_request(&format!("/shows/{}", id.0))?;
    Ok(show.into())
}

pub fn search(query: &str) -> Result<Vec<RemoteTvShow<TvMazeShowId>>, TvMazeApiError> {
    let query: &str = &urlencoding::encode(query);
    let shows: Vec<ApiSearchItem> = send_request(&format!("/search/shows?q={query}"))?;
    Ok(shows.into_iter().map(Into::into).collect())
}

pub fn episodes(show: &TvMazeShowId) -> Result<Vec<RemoteEpisode>, TvMazeApiError> {
    let episodes: Vec<ApiEpisode> = send_request(&format!("/shows/{}/episodes", show.0))?;
    Ok(episodes.into_iter().map(Into::into).collect())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_search_deserialization() {
        let body = include_bytes!("../res/tv_maze/search_result_banshee.json");
        serde_json::from_slice::<Vec<ApiSearchItem>>(body).expect("Failed to deserialize");

        let body = include_bytes!("../res/tv_maze/search_result_arcane.json");
        serde_json::from_slice::<Vec<ApiSearchItem>>(body).expect("Failed to deserialize");
    }

    #[test]
    fn test_episodes_deserialization() {
        let body = include_bytes!("../res/tv_maze/episodes_result.json");
        serde_json::from_slice::<Vec<ApiEpisode>>(body).expect("Failed to deserialize");

        let body = include_bytes!("../res/tv_maze/episodes_resident_alien.json");
        serde_json::from_slice::<Vec<ApiEpisode>>(body).expect("Failed to deserialize");
    }
}
