use crate::types::RemoteMovie;

use chrono::NaiveDate;
use serde::{de::DeserializeOwned, Deserialize};
use thiserror::Error;
use tracing::debug;

use std::{io::Read, num::ParseIntError};

#[derive(Debug, Deserialize)]
struct OmdbSearchResult {
    #[serde(rename = "Search")]
    search: Vec<OmdbSearchItem>,
}

#[derive(Debug, Deserialize)]
struct OmdbSearchItem {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Year")]
    year: String,
    #[serde(rename = "imdbID")]
    imdb_id: String,
    #[serde(rename = "Poster")]
    poster: String,
}

#[derive(Debug, Deserialize)]
struct OmdbMovie {
    #[serde(rename = "Title")]
    title: String,
    #[serde(rename = "Year")]
    year: String,
    #[serde(rename = "imdbID")]
    imdb_id: String,
    #[serde(rename = "Poster")]
    poster: String,
    #[serde(rename = "Released")]
    theater_release_date: String,
    #[serde(rename = "DVD")]
    home_release_date: String,
}

#[derive(Error, Debug)]
pub enum ParseRemoteMovieError {
    #[error("failed to parse theater date")]
    TheaterDate(#[source] chrono::ParseError),
    #[error("failed to home release date")]
    HomeDate(#[source] chrono::ParseError),
    #[error("failed to parse year")]
    Year(#[source] ParseIntError),
}

impl TryFrom<OmdbMovie> for RemoteMovie {
    type Error = ParseRemoteMovieError;
    fn try_from(value: OmdbMovie) -> Result<Self, ParseRemoteMovieError> {
        use ParseRemoteMovieError::*;

        let theater_release_date = Some(
            NaiveDate::parse_from_str(&value.theater_release_date, "%d %b %Y")
                .map_err(TheaterDate)?,
        );
        let home_release_date = Some(
            NaiveDate::parse_from_str(&value.home_release_date, "%d %b %Y").map_err(HomeDate)?,
        );

        Ok(RemoteMovie {
            imdb_id: value.imdb_id,
            name: value.title,
            year: value.year.parse().map_err(Year)?,
            image: value.poster,
            theater_release_date,
            home_release_date,
        })
    }
}

impl TryFrom<OmdbSearchItem> for RemoteMovie {
    type Error = ParseRemoteMovieError;
    fn try_from(value: OmdbSearchItem) -> Result<Self, ParseRemoteMovieError> {
        Ok(RemoteMovie {
            imdb_id: value.imdb_id,
            name: value.title,
            year: value.year.parse().map_err(ParseRemoteMovieError::Year)?,
            image: value.poster,
            theater_release_date: None,
            home_release_date: None,
        })
    }
}

const OMDB_BASE_URI: &str = "https://www.omdbapi.com/";

#[derive(Debug, Error)]
pub enum OmdbError {
    #[error("failed to execute get")]
    Get(#[source] isahc::Error),
    #[error("failed to read get response")]
    Read(#[source] std::io::Error),
    #[error("failed to parse json data")]
    Parse(#[source] serde_json::Error),
    #[error("failed to convert to common type")]
    ConversionFailed(#[source] ParseRemoteMovieError),
}

pub struct OmdbIndexer {
    api_key: String,
}

impl OmdbIndexer {
    pub fn new(api_key: String) -> OmdbIndexer {
        OmdbIndexer { api_key }
    }

    pub fn search(&self, query: &str) -> Result<Vec<RemoteMovie>, OmdbError> {
        let api_response: OmdbSearchResult =
            self.request(&format!("s={}&type=movie", urlencoding::encode(query)))?;

        api_response
            .search
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<RemoteMovie>, _>>()
            .map_err(OmdbError::ConversionFailed)
    }

    pub fn get_by_id(&self, imdb_id: &str) -> Result<RemoteMovie, OmdbError> {
        let api_response: OmdbMovie = self.request(&format!("i={}", imdb_id))?;
        api_response.try_into().map_err(OmdbError::ConversionFailed)
    }

    fn request<T: DeserializeOwned>(&self, params: &str) -> Result<T, OmdbError> {
        use OmdbError::*;

        let mut url = format!("{}?{}&apikey=", OMDB_BASE_URI, params);
        debug!("Sending requsest to {url}xxxxx");
        url.push_str(&self.api_key);

        let mut response = isahc::get(url).map_err(Get)?;
        let body = response.body_mut();

        let mut body_s = String::new();
        body.read_to_string(&mut body_s).map_err(Read)?;

        debug!("Returned content {}", body_s);
        serde_json::from_str(&body_s).map_err(Parse)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_search_result_parsing() {
        let test_files: &[&[u8]] = &[
            include_bytes!("../res/omdb/f9_search_result.json"),
            include_bytes!("../res/omdb/matrix_search_result.json"),
        ];
        for file in test_files {
            let res = serde_json::from_slice::<OmdbSearchResult>(file)
                .expect("Failed to parse search results");
            for movie in res.search {
                let _: RemoteMovie = movie.try_into().expect("Failed to convert to remote movie");
            }
        }
    }

    #[test]
    fn test_id_result_parsing() {
        let test_files: &[&[u8]] = &[
            include_bytes!("../res/omdb/f9_id_result.json"),
            include_bytes!("../res/omdb/matrix_id_result.json"),
        ];
        for file in test_files {
            let res = serde_json::from_slice::<OmdbMovie>(file).expect("Failed to parse id result");
            let _: RemoteMovie = res.try_into().expect("Failed to convert to remote movie");
        }
    }
}
