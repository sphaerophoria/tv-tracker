use serde::{Deserialize, Serialize};

use crate::tv_maze::TvMazeShowId;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ImdbShowId(pub String);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub struct TvdbShowId(pub i64);

#[derive(Serialize, Eq, PartialEq, Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShowWatchStatus {
    Finished,
    Unstarted,
    InProgress,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct RemoteTvShow<RemoteId> {
    pub id: RemoteId,
    pub name: String,
    pub image: Option<String>,
    pub year: Option<i32>,
    pub url: Option<String>,
    pub imdb_id: Option<ImdbShowId>,
    pub tvdb_id: Option<TvdbShowId>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TvShow {
    pub id: ShowId,
    pub remote_id: TvMazeShowId,
    pub name: String,
    pub image: Option<String>,
    pub year: Option<i32>,
    pub url: Option<String>,
    pub imdb_id: Option<ImdbShowId>,
    pub tvdb_id: Option<TvdbShowId>,
    pub watch_status: ShowWatchStatus,
    pub pause_status: bool,
    pub rating_id: Option<RatingId>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TvShowUpdate {
    pub id: ShowId,
    pub pause_status: Option<bool>,
    pub rating_id: Option<RatingId>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct RemoteEpisode {
    pub name: String,
    pub season: i64,
    pub episode: i64,
    pub airdate: Option<chrono::NaiveDate>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TvEpisode {
    pub id: EpisodeId,
    pub show_id: ShowId,
    pub name: String,
    pub season: i64,
    pub episode: i64,
    pub airdate: Option<chrono::NaiveDate>,
    pub watch_date: Option<chrono::NaiveDate>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Rating {
    pub id: RatingId,
    pub name: String,
    pub priority: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ShowId(pub i64);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct EpisodeId(pub i64);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct RatingId(pub i64);
