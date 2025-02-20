use serde::{Deserialize, Serialize};

use crate::tv_maze::TvMazeShowId;

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ImdbShowId(pub String);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub struct TvdbShowId(pub i64);

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
    pub image: Option<ImageId>,
    pub year: Option<i32>,
    pub url: Option<String>,
    pub imdb_id: Option<ImdbShowId>,
    pub tvdb_id: Option<TvdbShowId>,
    pub pause_status: bool,
    pub episodes_watched: Box<[i64]>,
    pub episodes_skipped: Box<[i64]>,
    pub episodes_aired: i64,
    pub rating_id: Option<RatingId>,
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TvShowUpdate {
    pub id: ShowId,
    pub pause_status: Option<bool>,
    pub rating_id: Option<RatingId>,
    pub notes: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct MovieUpdate {
    pub id: MovieId,
    pub watched: bool,
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
pub enum WatchStatus {
    Watched(chrono::NaiveDate),
    Skipped,
    Unwatched,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TvEpisode {
    pub id: EpisodeId,
    pub show_id: ShowId,
    pub name: String,
    pub season: i64,
    pub episode: i64,
    pub airdate: Option<chrono::NaiveDate>,
    pub watch_status: Box<[WatchStatus]>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Rating {
    pub id: RatingId,
    pub name: String,
    pub priority: usize,
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct RemoteMovie {
    pub imdb_id: String,
    pub name: String,
    pub year: i32,
    pub image: String,
    pub theater_release_date: Option<chrono::NaiveDate>,
    pub home_release_date: Option<chrono::NaiveDate>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct Movie {
    pub id: MovieId,
    pub imdb_id: String,
    pub name: String,
    pub image: ImageId,
    pub year: i32,
    pub watched: bool,
    pub rating_id: Option<RatingId>,
    pub theater_release_date: Option<chrono::NaiveDate>,
    pub home_release_date: Option<chrono::NaiveDate>,
}

macro_rules! impl_id {
    ($name:ident) => {
        #[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Hash)]
        pub struct $name(pub i64);
    };
}

impl_id!(ShowId);
impl_id!(EpisodeId);
impl_id!(RatingId);
impl_id!(ImageId);
impl_id!(MovieId);
