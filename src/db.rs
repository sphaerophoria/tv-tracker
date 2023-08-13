use crate::{
    tv_maze::TvMazeShowId,
    types::{
        EpisodeId, ImdbShowId, ShowId, TvEpisode, TvEpisodesList, TvShow, TvShowEpisode, TvdbShowId,
    },
};

use chrono::{Datelike, NaiveDate};
use rusqlite::{params, Connection, Row};
use thiserror::Error;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[derive(Debug, Error)]
pub enum DbCreationError {
    #[error("failed to open sqlite db")]
    OpenDb(#[source] rusqlite::Error),
    #[error("failed to get current version")]
    GetVersion(#[source] rusqlite::Error),
    #[error("failed to start transaction")]
    StartTransaction(#[source] rusqlite::Error),
    #[error("failed to commit transaction")]
    CommitTransaction(#[source] rusqlite::Error),
    #[error("failed to create show table")]
    CreateShowTable(#[source] rusqlite::Error),
    #[error("failed to upgrade episodes table to v2")]
    UpgradeEpisodesTalbeV2(#[source] rusqlite::Error),
    #[error("failed to create paused show table")]
    CreatePausedShows(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
#[error("failed to add show to db")]
pub struct AddShowError(#[source] rusqlite::Error);

#[derive(Debug, Error)]
pub enum RemoveShowError {
    #[error("failed to start transaction")]
    StartTransaction(#[source] rusqlite::Error),
    #[error("failed to remove pause status")]
    RemovePaused(#[source] rusqlite::Error),
    #[error("failed to remove watch status")]
    RemoveWatched(#[source] rusqlite::Error),
    #[error("failed to remove episodes")]
    RemoveEpisodes(#[source] rusqlite::Error),
    #[error("failed to remove show")]
    RemoveShow(#[source] rusqlite::Error),
    #[error("failed to verify foreign keys")]
    ForeignKeyCheck(#[source] rusqlite::Error),
    #[error("failed to commit transaction")]
    CommitTransaction(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum GetShowError {
    #[error("failed to prepare get show request")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute get show request")]
    Execute(#[source] rusqlite::Error),
    #[error("failed to get row from query response")]
    GetRow(#[source] rusqlite::Error),
    #[error("failed to get id")]
    GetId(#[source] rusqlite::Error),
    #[error("failed to get tv maze id")]
    GetTvMazeId(#[source] rusqlite::Error),
    #[error("failed to get name")]
    GetName(#[source] rusqlite::Error),
    #[error("failed to get year")]
    GetYear(#[source] rusqlite::Error),
    #[error("failed to get imdb id")]
    GetImdbId(#[source] rusqlite::Error),
    #[error("failed to get tvdb id")]
    GetTvdbId(#[source] rusqlite::Error),
    #[error("failed to get image url")]
    GetImageUrl(#[source] rusqlite::Error),
    #[error("failed to get tvmaze url")]
    GetTvMazeUrl(#[source] rusqlite::Error),
    #[error("missing id")]
    MissingId,
}

#[derive(Debug, Error)]
pub enum AddEpisodeError {
    #[error("failed to check if episode exists")]
    FindExisting(#[source] FindEpisodeError),
    #[error("failed to insert new episode")]
    InsertEpisode(#[source] rusqlite::Error),
    #[error("failed to insert new episode")]
    UpdateEpisode(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
#[error("failed to get episodes from db")]
pub enum GetEpisodeError {
    #[error("failed to prepare get show request")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute get show request")]
    Execute(#[source] rusqlite::Error),
    #[error("failed to get episode id")]
    GetId(#[source] rusqlite::Error),
    #[error("failed to get show id for episode")]
    GetShowId(#[source] rusqlite::Error),
    #[error("failed to get episode name")]
    GetName(#[source] rusqlite::Error),
    #[error("failed to get episode number")]
    GetSeason(#[source] rusqlite::Error),
    #[error("failed to get episode number")]
    GetEpisode(#[source] rusqlite::Error),
    #[error("failed to get airdate")]
    GetAirdate(#[source] rusqlite::Error),
    #[error("failed to parse airdate")]
    InvalidDate,
}

#[derive(Debug, Error)]
pub enum FindEpisodeError {
    #[error("failed to prepare find episode request")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute find episode request")]
    Execute(#[source] rusqlite::Error),
    #[error("failed to get first row from response")]
    InvalidRow(#[source] rusqlite::Error),
    #[error("failed to extract episode id")]
    InvalidEpisodeId(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
#[error("failed to set watch status")]
pub struct SetWatchStatusError(#[source] rusqlite::Error);

#[derive(Debug, Error)]
pub enum GetWatchStatusError {
    #[error("failed to prepare get watch status")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute get watch status")]
    Execute(#[source] rusqlite::Error),
    #[error("failed to get id of returned row")]
    GetId(#[source] rusqlite::Error),
    #[error("failed to get watch date")]
    GetDate(#[source] rusqlite::Error),
    #[error("invalid watch date")]
    InvalidDate,
}

#[derive(Debug, Error)]
#[error("failed to set pause status")]
pub struct SetPauseError(#[source] rusqlite::Error);

#[derive(Debug, Error)]
pub enum GetPausedShowError {
    #[error("failed to prepare pause statement")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute pause statement")]
    Execute(#[source] rusqlite::Error),
    #[error("failed to parse show id")]
    Parse(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum GetAiredEpisodesError {
    #[error("failed to get episodes")]
    GetEpisodes(#[from] GetEpisodeError),
    #[error("failed to get shows for episodes")]
    GetShows(#[from] GetShowError),
}

pub struct Db {
    connection: Connection,
}

impl Db {
    pub fn new(path: &Path) -> Result<Db, DbCreationError> {
        let mut connection = Connection::open(path).map_err(DbCreationError::OpenDb)?;

        initialize_connection(&mut connection)?;

        Ok(Db { connection })
    }

    #[cfg(test)]
    fn new_in_memory() -> Result<Db, DbCreationError> {
        let mut connection = Connection::open_in_memory().map_err(DbCreationError::OpenDb)?;

        initialize_connection(&mut connection)?;

        Ok(Db { connection })
    }

    pub fn add_show(
        &mut self,
        show: &TvShow,
        tvmaze_id: &TvMazeShowId,
    ) -> Result<ShowId, AddShowError> {
        self.connection
            .execute(
                "
            INSERT INTO shows(name, tvmaze_id, year, imdb_id, tvdb_id, image_url, tvmaze_url)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
                params![
                    show.name,
                    tvmaze_id.0,
                    show.year,
                    show.imdb_id.as_ref().map(|x| x.0.clone()),
                    show.tvdb_id.map(|x| x.0),
                    show.image,
                    show.url
                ],
            )
            .map_err(AddShowError)?;

        Ok(ShowId(self.connection.last_insert_rowid()))
    }

    pub fn remove_show(&mut self, show: &ShowId) -> Result<(), RemoveShowError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(RemoveShowError::StartTransaction)?;

        transaction
            .execute("DELETE FROM paused_shows WHERE show_id = ?1", [show.0])
            .map_err(RemoveShowError::RemovePaused)?;

        transaction
            .execute(
                "
                DELETE FROM watch_status WHERE episode_id IN (
                    SELECT id FROM episodes WHERE show_id = ?1
                )
                ",
                [show.0],
            )
            .map_err(RemoveShowError::RemoveWatched)?;

        transaction
            .execute("DELETE FROM episodes WHERE show_id = ?1", [show.0])
            .map_err(RemoveShowError::RemoveEpisodes)?;

        transaction
            .execute("DELETE FROM shows WHERE id = ?1", [show.0])
            .map_err(RemoveShowError::RemoveShow)?;

        transaction
            .execute("PRAGMA foreign_key_check", [])
            .map_err(RemoveShowError::ForeignKeyCheck)?;

        transaction
            .commit()
            .map_err(RemoveShowError::CommitTransaction)?;

        Ok(())
    }

    pub fn get_shows(&self) -> Result<Vec<(ShowId, TvMazeShowId, TvShow)>, GetShowError> {
        let mut statement = self
            .connection
            .prepare("SELECT * FROM shows")
            .map_err(GetShowError::Prepare)?;

        let mut rows = statement.query([]).map_err(GetShowError::Execute)?;
        let mut ret = Vec::new();

        loop {
            let row = rows.next().map_err(GetShowError::GetRow)?;

            let row = match row {
                Some(v) => v,
                None => break,
            };

            let id = row.get(0).map_err(GetShowError::GetId)?;
            let id = ShowId(id);

            let tvmaze_id: i64 = row.get(2).map_err(GetShowError::GetTvMazeId)?;
            let tvmaze_id = TvMazeShowId(tvmaze_id);

            let show = show_from_row_indices(
                row,
                ShowIndices {
                    name: 1,
                    image: 6,
                    year: 3,
                    url: 7,
                    imdb_id: 4,
                    tvdb_id: 5,
                },
            )?;

            ret.push((id, tvmaze_id, show));
        }

        Ok(ret)
    }

    fn find_episode(
        &mut self,
        show_id: &ShowId,
        episode: &TvEpisode,
    ) -> Result<Option<EpisodeId>, FindEpisodeError> {
        let mut statement = self
            .connection
            .prepare("SELECT id FROM episodes WHERE show_id = ?1 AND season = ?2 AND episode = ?3")
            .map_err(FindEpisodeError::Prepare)?;

        let mut rows = statement
            .query([show_id.0, episode.season, episode.episode])
            .map_err(FindEpisodeError::Execute)?;

        let row = rows.next().map_err(FindEpisodeError::InvalidRow)?;
        let row = match row {
            Some(v) => v,
            None => return Ok(None),
        };
        let episode_id = row.get(0).map_err(FindEpisodeError::InvalidEpisodeId)?;
        Ok(Some(EpisodeId(episode_id)))
    }

    pub fn add_episode(
        &mut self,
        show_id: &ShowId,
        episode: &TvEpisode,
    ) -> Result<EpisodeId, AddEpisodeError> {
        let episode_id = self
            .find_episode(show_id, episode)
            .map_err(AddEpisodeError::FindExisting)?;

        if let Some(episode_id) = episode_id {
            self.connection
                .execute(
                    "
                    UPDATE episodes
                    SET show_id = ?2, name = ?3, season = ?4, episode = ?5, airdate = ?6
                    WHERE id = ?1
                    ",
                    params![
                        episode_id.0,
                        show_id.0,
                        episode.name,
                        episode.season,
                        episode.episode,
                        episode.airdate.map(|v| v.num_days_from_ce())
                    ],
                )
                .map_err(AddEpisodeError::UpdateEpisode)?;

            Ok(episode_id)
        } else {
            self.connection
                .execute(
                    "
                INSERT INTO episodes(show_id, name, season, episode, airdate)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                    params![
                        show_id.0,
                        episode.name,
                        episode.season,
                        episode.episode,
                        episode.airdate.map(|v| v.num_days_from_ce())
                    ],
                )
                .map_err(AddEpisodeError::InsertEpisode)?;

            Ok(EpisodeId(self.connection.last_insert_rowid()))
        }
    }

    pub fn get_episodes(
        &self,
        show: &ShowId,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, GetEpisodeError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, season, episode, airdate FROM episodes WHERE show_id = ?1")
            .map_err(GetEpisodeError::Prepare)?;

        let mut rows = statement
            .query([show.0])
            .map_err(GetEpisodeError::Execute)?;

        let mut ret = HashMap::new();

        while let Ok(Some(row)) = rows.next() {
            let id = row.get(0).map_err(GetEpisodeError::GetId)?;
            let id = EpisodeId(id);

            let episode = episode_from_row_indices(
                row,
                EpisodeIndices {
                    name: 1,
                    season: 2,
                    episode: 3,
                    airdate: 4,
                },
            )?;

            ret.insert(id, episode);
        }

        Ok(ret)
    }

    pub fn set_episode_watch_status(
        &mut self,
        episode: &EpisodeId,
        watched: Option<NaiveDate>,
    ) -> Result<(), SetWatchStatusError> {
        if let Some(date) = watched {
            self.connection
                .execute(
                    "
                    INSERT OR IGNORE INTO watch_status(episode_id, watch_date)
                    VALUES (?1, ?2)
                    ",
                    params![episode.0, date.num_days_from_ce()],
                )
                .map_err(SetWatchStatusError)?;
        } else {
            self.connection
                .execute(
                    "
                    DELETE FROM watch_status
                    WHERE episode_id = ?1
                    ",
                    [episode.0],
                )
                .map_err(SetWatchStatusError)?;
        }

        Ok(())
    }

    pub fn get_show_watch_status(
        &self,
        show: &ShowId,
    ) -> Result<HashMap<EpisodeId, NaiveDate>, GetWatchStatusError> {
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT episode_id, watch_date
                FROM watch_status
                LEFT JOIN episodes ON watch_status.episode_id = episodes.id
                WHERE episodes.show_id = ?1
                ",
            )
            .map_err(GetWatchStatusError::Prepare)?;

        let mut rows = statement
            .query([show.0])
            .map_err(GetWatchStatusError::Execute)?;

        let mut ret = HashMap::new();

        while let Ok(Some(row)) = rows.next() {
            let id = row.get(0).map_err(GetWatchStatusError::GetId)?;
            let id = EpisodeId(id);

            let date = row.get(1).map_err(GetWatchStatusError::GetDate)?;
            let date = NaiveDate::from_num_days_from_ce_opt(date)
                .ok_or(GetWatchStatusError::InvalidDate)?;

            ret.insert(id, date);
        }

        Ok(ret)
    }

    pub fn set_pause_status(&self, show: &ShowId, paused: bool) -> Result<(), SetPauseError> {
        if paused {
            self.connection
                .execute(
                    "
                    INSERT OR IGNORE INTO paused_shows(show_id)
                    VALUES (?1)
                    ",
                    params![show.0],
                )
                .map_err(SetPauseError)?;
        } else {
            self.connection
                .execute(
                    "
                    DELETE FROM paused_shows WHERE show_id = ?1
                    ",
                    params![show.0],
                )
                .map_err(SetPauseError)?;
        }

        Ok(())
    }

    pub fn get_paused_shows(&self) -> Result<HashSet<ShowId>, GetPausedShowError> {
        let mut statement = self
            .connection
            .prepare("SELECT show_id FROM paused_shows")
            .map_err(GetPausedShowError::Prepare)?;

        let mut rows = statement.query([]).map_err(GetPausedShowError::Execute)?;

        let mut ret = HashSet::new();
        while let Ok(Some(row)) = rows.next() {
            ret.insert(ShowId(row.get(0).map_err(GetPausedShowError::Parse)?));
        }

        Ok(ret)
    }

    pub fn get_episodes_aired_between(
        &self,
        start_date: &NaiveDate,
        end_date: &NaiveDate,
    ) -> Result<TvEpisodesList, GetAiredEpisodesError> {
        let mut statement = self
            .connection
            .prepare(
                "
                SELECT show_id, name, season, episode, airdate FROM episodes
                WHERE airdate IS NOT NULL AND airdate >= ?1 AND airdate <= ?2
                ",
            )
            .map_err(GetEpisodeError::Prepare)?;

        let mut rows = statement
            .query([start_date.num_days_from_ce(), end_date.num_days_from_ce()])
            .map_err(GetEpisodeError::Execute)?;

        let mut show_ids = HashSet::new();
        let mut ret = TvEpisodesList {
            shows: Default::default(),
            episodes: Default::default(),
        };

        while let Ok(Some(row)) = rows.next() {
            let show_id: i64 = row.get(0).map_err(GetEpisodeError::GetShowId)?;
            let show_id = ShowId(show_id);
            let episode = episode_from_row_indices(
                row,
                EpisodeIndices {
                    name: 1,
                    season: 2,
                    episode: 3,
                    airdate: 4,
                },
            )?;

            show_ids.insert(show_id);

            ret.episodes.push(TvShowEpisode { show_id, episode });
        }

        // AFAICT, this is the easiest way to inject the show IDs retrieved from the previous step,
        // happy to be proven wrong
        let query_str = format!(
            "
            SELECT id, name, image_url, year, tvmaze_url, imdb_id, tvdb_id FROM shows
            WHERE id IN ({})
            ",
            show_ids_to_comma_separated(&show_ids)
        );
        let mut statement = self
            .connection
            .prepare(&query_str)
            .map_err(GetShowError::Prepare)?;

        let mut rows = statement.query([]).map_err(GetShowError::Execute)?;

        while let Ok(Some(row)) = rows.next() {
            let show_id: i64 = row.get(0).map_err(GetShowError::GetId)?;
            let show_id = ShowId(show_id);

            let show = show_from_row_indices(
                row,
                ShowIndices {
                    name: 1,
                    image: 2,
                    year: 3,
                    url: 4,
                    imdb_id: 5,
                    tvdb_id: 6,
                },
            )?;

            ret.shows.insert(show_id, show);
        }

        for show_id in show_ids {
            if !ret.shows.contains_key(&show_id) {
                return Err(GetShowError::MissingId.into());
            }
        }

        Ok(ret)
    }
}

fn show_ids_to_comma_separated(show_ids: &HashSet<ShowId>) -> String {
    let mut it = show_ids.iter();
    let first = it.next();

    let mut ret = match first {
        Some(v) => v.0.to_string(),
        None => {
            return String::new();
        }
    };

    for elem in it {
        ret.push(',');
        ret.push_str(&elem.0.to_string());
    }

    ret
}

struct EpisodeIndices {
    name: usize,
    season: usize,
    episode: usize,
    airdate: usize,
}

fn episode_from_row_indices(
    row: &Row,
    indices: EpisodeIndices,
) -> Result<TvEpisode, GetEpisodeError> {
    let name = row.get(indices.name).map_err(GetEpisodeError::GetName)?;

    let season = row
        .get(indices.season)
        .map_err(GetEpisodeError::GetSeason)?;
    let episode = row
        .get(indices.episode)
        .map_err(GetEpisodeError::GetEpisode)?;
    let airdate: Option<i32> = row
        .get(indices.airdate)
        .map_err(GetEpisodeError::GetAirdate)?;
    let airdate = match airdate {
        Some(v) => {
            Some(NaiveDate::from_num_days_from_ce_opt(v).ok_or(GetEpisodeError::InvalidDate)?)
        }
        None => None,
    };

    Ok(TvEpisode {
        name,
        season,
        episode,
        airdate,
    })
}

struct ShowIndices {
    name: usize,
    image: usize,
    year: usize,
    url: usize,
    imdb_id: usize,
    tvdb_id: usize,
}

fn show_from_row_indices(row: &Row, indices: ShowIndices) -> Result<TvShow, GetShowError> {
    let name = row.get(indices.name).map_err(GetShowError::GetName)?;

    let year = row.get(indices.year).map_err(GetShowError::GetYear)?;
    let imdb_id: Option<String> = row.get(indices.imdb_id).map_err(GetShowError::GetImdbId)?;
    let imdb_id = imdb_id.map(ImdbShowId);

    let tvdb_id: Option<i64> = row.get(indices.tvdb_id).map_err(GetShowError::GetTvdbId)?;
    let tvdb_id = tvdb_id.map(TvdbShowId);

    let image = row.get(indices.image).map_err(GetShowError::GetImageUrl)?;
    let url = row.get(indices.url).map_err(GetShowError::GetTvMazeUrl)?;

    Ok(TvShow {
        name,
        year,
        imdb_id,
        tvdb_id,
        image,
        url,
    })
}

fn initialize_v1_db(connection: &mut Connection) -> Result<(), DbCreationError> {
    let transaction = connection
        .transaction()
        .map_err(DbCreationError::StartTransaction)?;

    transaction
        .execute_batch(
            // NOTE: Presence of lots of nullable fields may initially indicate that we should
            // be splitting our show table into multiple tables. This does not make sense for
            // our use case. We are essentially always serializing and deserializing our TvShow
            // struct in one shot. In this scenario joining multiple tables just to avoid
            // nullables does not make sense, as we lookup all fields to convert them back to
            // Option::None anyways
            "
            CREATE TABLE IF NOT EXISTS shows(
                id INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                tvmaze_id INTEGER NOT NULL,
                year INTEGER,
                imdb_id TEXT,
                tvdb_id INTEGER,
                image_url TEXT,
                tvmaze_url TEXT
            );
            CREATE TABLE IF NOT EXISTS episodes(
                id INTEGER PRIMARY KEY NOT NULL,
                show_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                season INTEGER NOT NULL,
                episode INTEGER NOT NULL,
                airdate INTEGER NOT NULL,
                FOREIGN KEY(show_id) REFERENCES shows(id)

            );
            CREATE TABLE IF NOT EXISTS watch_status(
                episode_id INTEGER PRIMARY KEY NOT NULL,
                watch_date INTEGER NOT NULL,
                FOREIGN KEY(episode_id) REFERENCES episodes(id)
            );
            PRAGMA user_version = 1;
            ",
        )
        .map_err(DbCreationError::CreateShowTable)?;

    transaction
        .commit()
        .map_err(DbCreationError::CommitTransaction)?;

    Ok(())
}

fn upgrade_v1_v2(connection: &mut Connection) -> Result<(), DbCreationError> {
    let transaction = connection
        .transaction()
        .map_err(DbCreationError::StartTransaction)?;

    transaction
        .execute_batch(
            "
            CREATE TABLE new_episodes(
                id INTEGER PRIMARY KEY NOT NULL,
                show_id INTEGER NOT NULL,
                name TEXT NOT NULL,
                season INTEGER NOT NULL,
                episode INTEGER NOT NULL,
                airdate INTEGER,
                FOREIGN KEY(show_id) REFERENCES shows(id)
            );
            INSERT INTO new_episodes SELECT id, show_id, name, season, episode, airdate from episodes;
            DROP TABLE episodes;
            ALTER TABLE new_episodes RENAME TO episodes;
            PRAGMA user_version = 2;
            ",
        )
        .map_err(DbCreationError::UpgradeEpisodesTalbeV2)?;

    transaction
        .commit()
        .map_err(DbCreationError::CommitTransaction)?;

    Ok(())
}

fn upgrade_v2_v3(connection: &mut Connection) -> Result<(), DbCreationError> {
    let transaction = connection
        .transaction()
        .map_err(DbCreationError::StartTransaction)?;

    transaction
        .execute_batch(
            "
            CREATE TABLE paused_shows(
                show_id INTEGER PRIMARY KEY NOT NULL,
                FOREIGN KEY(show_id) REFERENCES shows(id)
            );
            PRAGMA user_version = 3;
            ",
        )
        .map_err(DbCreationError::CreatePausedShows)?;

    transaction
        .commit()
        .map_err(DbCreationError::CommitTransaction)?;

    Ok(())
}

fn initialize_connection(connection: &mut Connection) -> Result<(), DbCreationError> {
    let version: usize = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(DbCreationError::GetVersion)?;

    let upgrade_functions = [initialize_v1_db, upgrade_v1_v2, upgrade_v2_v3];

    for f in upgrade_functions.iter().skip(version) {
        f(connection)?;
    }

    let version: usize = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(DbCreationError::GetVersion)?;

    assert_eq!(version, 3);

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_full_show_in_out() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: Some("test_url".to_string()),
            year: Some(1234),
            url: Some("tvmaze_url".to_string()),
            imdb_id: Some(ImdbShowId("imdbid".to_string())),
            tvdb_id: Some(TvdbShowId(12)),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");
        let retrieved_shows = db.get_shows().expect("Failed to get show");

        assert_eq!(retrieved_shows.len(), 1);
        assert_eq!(retrieved_shows[0].0, id);
        assert_eq!(retrieved_shows[0].1, TvMazeShowId(0));
        assert_eq!(retrieved_shows[0].2, show);
    }

    #[test]
    fn test_empty_show_in_out() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");
        let retrieved_shows = db.get_shows().expect("Failed to get show");

        assert_eq!(retrieved_shows.len(), 1);
        assert_eq!(retrieved_shows[0].0, id);
        assert_eq!(retrieved_shows[0].1, TvMazeShowId(0));
        assert_eq!(retrieved_shows[0].2, show);
    }

    #[test]
    fn test_get_shows() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let show2 = TvShow {
            name: "Test Show 2".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");
        let id2 = db
            .add_show(&show2, &TvMazeShowId(1))
            .expect("Failed to add show");

        let shows = db.get_shows().expect("Failed to get shows");
        let shows = shows
            .into_iter()
            .map(|(id, tvmaze_id, show)| (id, (tvmaze_id, show)))
            .collect::<HashMap<_, _>>();
        assert_eq!(shows.len(), 2);
        assert_eq!(shows[&id].0, TvMazeShowId(0));
        assert_eq!(shows[&id].1, show);
        assert_eq!(shows[&id2].0, TvMazeShowId(1));
        assert_eq!(shows[&id2].1, show2);
    }

    #[test]
    fn test_episode_in_out() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let episode = TvEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");

        let id = db
            .add_episode(&show_id, &episode)
            .expect("Failed to add episode");

        let retrieved_episodes = db.get_episodes(&show_id).expect("Failed to get episodes");

        assert_eq!(retrieved_episodes.len(), 1);
        assert_eq!(retrieved_episodes[&id], episode);
    }

    #[test]
    fn test_update_episode() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let episode = TvEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let episode_update = TvEpisode {
            name: "Test Episode updated".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1024),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");

        let id = db
            .add_episode(&show_id, &episode)
            .expect("Failed to add episode");

        db.add_episode(&show_id, &episode_update)
            .expect("Failed to add episode");

        let retrieved_episodes = db.get_episodes(&show_id).expect("Failed to get episodes");

        assert_eq!(retrieved_episodes.len(), 1);
        assert_eq!(retrieved_episodes[&id], episode_update);
    }

    #[test]
    fn test_set_watch_status() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let episode = TvEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let episode2 = TvEpisode {
            name: "Test Episode 2".to_string(),
            season: 1,
            episode: 35,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");

        let episode_id = db
            .add_episode(&show_id, &episode)
            .expect("Failed to add episode");

        db.add_episode(&show_id, &episode2)
            .expect("Failed to add episode");

        let watch_date = NaiveDate::from_num_days_from_ce_opt(1024).expect("Invalid date");

        db.set_episode_watch_status(&episode_id, Some(watch_date))
            .expect("Failed to set watch status");

        let retrieved_episodes = db
            .get_show_watch_status(&show_id)
            .expect("Failed to get episodes");

        assert_eq!(retrieved_episodes.len(), 1);
        assert_eq!(retrieved_episodes[&episode_id], watch_date);

        db.set_episode_watch_status(&episode_id, None)
            .expect("Failed to set watch status");

        let retrieved_episodes = db
            .get_show_watch_status(&show_id)
            .expect("Failed to get episodes");

        assert_eq!(retrieved_episodes.len(), 0);
    }

    #[test]
    fn test_set_pause_status() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let show2 = TvShow {
            name: "Test Show 2".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");
        db.add_show(&show2, &TvMazeShowId(1))
            .expect("Failed to add show");

        db.set_pause_status(&id, false)
            .expect("Failed to set pause");
        let paused = db.get_paused_shows().expect("Failed to get shows");
        assert_eq!(paused.len(), 0);

        db.set_pause_status(&id, true).expect("Failed to set pause");
        let paused = db.get_paused_shows().expect("Failed to get shows");
        assert_eq!(paused.len(), 1);
        assert!(paused.contains(&id));

        db.set_pause_status(&id, false)
            .expect("Failed to set pause");
        let paused = db.get_paused_shows().expect("Failed to get shows");
        assert_eq!(paused.len(), 0);
    }

    #[test]
    fn test_remove_show() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let episode = TvEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");

        db.set_pause_status(&id, true).expect("Failed to set pause");

        let episode_id = db
            .add_episode(&id, &episode)
            .expect("Failed to add episode");

        let watch_date = NaiveDate::from_num_days_from_ce_opt(1024).expect("Invalid date");

        db.set_episode_watch_status(&episode_id, Some(watch_date))
            .expect("Failed to set watch status");

        let paused = db.get_paused_shows().expect("Failed to get shows");
        assert_eq!(paused.len(), 1);

        let episodes = db.get_episodes(&id).expect("Failed to get episodes");
        assert_eq!(episodes.len(), 1);

        let watched = db
            .get_show_watch_status(&id)
            .expect("Failed to get watch status");
        assert_eq!(watched.len(), 1);

        db.remove_show(&id).expect("Failed to remove show");

        let paused = db.get_paused_shows().expect("Failed to get shows");
        assert_eq!(paused.len(), 0);

        let episodes = db.get_episodes(&id).expect("Failed to get episodes");
        assert_eq!(episodes.len(), 0);

        let watched = db
            .get_show_watch_status(&id)
            .expect("Failed to get watch status");
        assert_eq!(watched.len(), 0);
    }

    #[test]
    fn test_shows_aired_between() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let show2 = TvShow {
            name: "Test Show 2".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let show3 = TvShow {
            name: "Test Show 2".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");
        let show1_id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show 1");
        let show2_id = db
            .add_show(&show2, &TvMazeShowId(1))
            .expect("Failed to add show 2");

        // Show 3 has no episodes
        db.add_show(&show3, &TvMazeShowId(2))
            .expect("Failed to add show 3");

        let show_ids = [show1_id, show2_id];
        for i in 0..100 {
            let show_id = show_ids[(i % 2) as usize];
            let episode = TvEpisode {
                name: "Test episode".to_string(),
                season: 1,
                episode: i,
                // 4 episodes a day, starting at 1000
                airdate: NaiveDate::from_num_days_from_ce_opt(((4000 + i) / 4) as i32),
            };

            db.add_episode(&show_id, &episode).unwrap();
        }

        let start_date =
            NaiveDate::from_num_days_from_ce_opt(1012).expect("Failed to set start date");
        let end_date = NaiveDate::from_num_days_from_ce_opt(1014).expect("Failed to set end date");

        let show_episodes = db
            .get_episodes_aired_between(&start_date, &end_date)
            .expect("Failed to get aired episodes");

        assert_eq!(show_episodes.shows.len(), 2);
        assert_eq!(show_episodes.shows.get(&show1_id), Some(&show));
        assert_eq!(show_episodes.shows.get(&show2_id), Some(&show2));

        // Airdates are inclusive, so we should expect 3 days of 4 episodes a day
        assert_eq!(show_episodes.episodes.len(), 12);
        assert_eq!(show_episodes.episodes[0].episode.airdate, Some(start_date));
        assert_eq!(show_episodes.episodes[11].episode.airdate, Some(end_date));
    }
}
