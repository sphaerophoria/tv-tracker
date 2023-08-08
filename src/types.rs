use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct ImdbShowId(pub String);

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq)]
pub struct TvdbShowId(pub i64);

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TvShow {
    pub name: String,
    pub image: Option<String>,
    pub year: Option<i32>,
    pub url: Option<String>,
    pub imdb_id: Option<ImdbShowId>,
    pub tvdb_id: Option<TvdbShowId>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct TvEpisode {
    pub name: String,
    pub season: i64,
    pub episode: i64,
    pub airdate: Option<chrono::NaiveDate>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub struct ShowId(pub i64);

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, Hash)]
pub struct EpisodeId(pub i64);
