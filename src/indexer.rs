use serde::{Deserialize, Serialize};

use std::fmt::{Debug, Display};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TvShow<Id> {
    pub id: Id,
    pub name: String,
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
    type Err: Debug + Display;
    fn search(&mut self, query: &str) -> Result<TvShows<Self::ShowId>, Self::Err>;
    fn episodes(&mut self, show: &Self::ShowId) -> Result<TvEpisodes<Self::EpisodeId>, Self::Err>;
}
