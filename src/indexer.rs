use serde::{Deserialize, Serialize};

use std::{
    error::Error,
    fmt::{Debug, Display},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TvShow<Id> {
    pub id: Id,
    pub name: String,
    pub image: Option<String>,
    pub year: Option<i32>,
    pub url: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TvEpisode<Id> {
    pub id: Id,
    pub name: String,
    pub season: i64,
    pub episode: i64,
    pub airdate: chrono::NaiveDate,
}

pub type TvShows<T> = Vec<TvShow<T>>;
pub type TvEpisodes<T> = Vec<TvEpisode<T>>;

pub trait Indexer: Send + Sync + 'static {
    type ShowId: Serialize;
    type EpisodeId: Serialize + Send + Clone;
    type Err: Debug + Display + Error + Send + Sync;

    fn search(&self, query: &str) -> Result<TvShows<Self::ShowId>, Self::Err>;
    fn episodes(&self, show: &Self::ShowId) -> Result<TvEpisodes<Self::EpisodeId>, Self::Err>;
}
