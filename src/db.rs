use crate::{
    tv_maze::TvMazeShowId,
    types::{
        EpisodeId, ImageId, ImdbShowId, Movie, MovieId, Rating, RatingId, RemoteEpisode,
        RemoteMovie, RemoteTvShow, ShowId, TvEpisode, TvShow, TvdbShowId,
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
    #[error("failed to create ratings tables")]
    CreateRatings(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum AddShowError {
    #[error("failed to insert show")]
    Insert(#[source] rusqlite::Error),
    #[error("failed to start transaction")]
    StartTransaction(#[source] rusqlite::Error),
    #[error("failed to get inserted show")]
    GetShow(#[from] GetShowError),
    #[error("failed to commit transaction")]
    CommitTransaction(#[source] rusqlite::Error),
}

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
    #[error("failed to remove image")]
    RemoveImage(#[source] rusqlite::Error),
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
    #[error("failed to get remote id")]
    GetRemoteId(#[source] rusqlite::Error),
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
    #[error("failed to get pause status")]
    GetPauseStatus(#[source] rusqlite::Error),
    #[error("failed to get watch count")]
    GetWatchCount(#[source] rusqlite::Error),
    #[error("incorrect number of elements returned")]
    IncorrectLen,
    #[error("failed to get rating id")]
    GetRatingId(#[source] rusqlite::Error),
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
    #[error("failed to get row from query")]
    GetRow(#[source] rusqlite::Error),
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
    #[error("failed to get watch date")]
    GetWatchdate(#[source] rusqlite::Error),
    #[error("failed to parse airdate")]
    InvalidDate,
    #[error("query returned wrong number of results")]
    IncorrectLen,
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
#[error("failed to set pause status")]
pub struct SetPauseError(#[source] rusqlite::Error);

const GET_SHOWS_QUERY: &str =
    "
    SELECT shows.id, shows.tvmaze_id, shows.name, shows.image_id, shows.year, shows.tvmaze_url, shows.imdb_id, shows.tvdb_id, watch_count.count, epi_count.count, paused_shows.show_id, show_ratings.rating_id FROM shows
    LEFT JOIN
        (
            SELECT show_id, COUNT(*) as count FROM episodes
            WHERE episodes.airdate <= ?1
            GROUP BY show_id

        ) as epi_count
        ON shows.id = epi_count.show_id
    LEFT JOIN
        (
            SELECT show_id, COUNT(*) as count FROM episode_watch_status
            LEFT JOIN episodes WHERE episodes.id = episode_watch_status.episode_id
            GROUP BY show_id
        ) as watch_count
        ON shows.id = watch_count.show_id
    LEFT JOIN paused_shows ON shows.id = paused_shows.show_id
    LEFT JOIN show_ratings ON shows.id = show_ratings.show_id
    ";

#[derive(Debug, Error)]
pub enum GetRatingsError {
    #[error("failed to prepare statement")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute query")]
    Query(#[source] rusqlite::Error),
    #[error("failed to get row")]
    GetRow(#[source] rusqlite::Error),
    #[error("failed to get id")]
    GetId(#[source] rusqlite::Error),
    #[error("failed to get name")]
    GetName(#[source] rusqlite::Error),
    #[error("failed to get priority")]
    GetPriority(#[source] rusqlite::Error),
    #[error("failed to find rating")]
    Missing,
}

#[derive(Debug, Error)]
pub enum AddRatingError {
    #[error("failed to start transaction")]
    Transaction(#[source] rusqlite::Error),
    #[error("failed to prepare get priority statement")]
    PrepareGetPriority(#[source] rusqlite::Error),
    #[error("failed to query for largest priority")]
    QueryGetPriority(#[source] rusqlite::Error),
    #[error("failed to extract priority from sqlite response")]
    GetPriorityRow(#[source] rusqlite::Error),
    #[error("failed to insert new rating")]
    Insert(#[source] rusqlite::Error),
    #[error("failed to commit transaction")]
    CommitTransaction(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum DeleteRatingError {
    #[error("failed to start transaction")]
    Transaction(#[source] rusqlite::Error),
    #[error("failed to commit transaction")]
    CommitTransaction(#[source] rusqlite::Error),
    #[error("failed to delete rating")]
    DeleteRating(#[source] rusqlite::Error),
    #[error("failed to delete show ratings")]
    DeleteShowRatings(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
#[error("failed to set show rating")]
pub struct SetShowRatingError(#[source] rusqlite::Error);

#[derive(Debug, Error)]
pub enum GetImageUrlError {
    #[error("failed to prepare get image statement")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to query images")]
    Query(#[source] rusqlite::Error),
    #[error("failed to get row")]
    GetRow(#[source] rusqlite::Error),
    #[error("query returned no results")]
    MissingRow,
    #[error("failed to get url from query")]
    GetImageUrl(#[source] rusqlite::Error),
}

#[derive(Error, Debug)]
pub enum AddMovieError {
    #[error("failed to start transaction")]
    StartTransaction(#[source] rusqlite::Error),
    #[error("failed to check if movie is in db")]
    FindRemote(#[source] rusqlite::Error),
    #[error("failed to insert movie into db")]
    Insert(#[source] rusqlite::Error),
    #[error("failed to commit transaction")]
    Commit(#[source] rusqlite::Error),
}

#[derive(Error, Debug)]
pub enum GetMovieError {
    #[error("failed to prepare statement")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute query")]
    Execute(#[source] rusqlite::Error),
    #[error("failed to get row")]
    GetRow(#[source] rusqlite::Error),
    #[error("failed to get id from row")]
    GetId(#[source] rusqlite::Error),
    #[error("failed to get imdb id from row")]
    GetImdbId(#[source] rusqlite::Error),
    #[error("failed to get name from row")]
    GetName(#[source] rusqlite::Error),
    #[error("failed to get year from row")]
    GetYear(#[source] rusqlite::Error),
    #[error("failed to get image from row")]
    GetImage(#[source] rusqlite::Error),
    #[error("failed to get theater release date from row")]
    GetTheaterReleaseDate(#[source] rusqlite::Error),
    #[error("failed to get home release date from row")]
    GetHomeReleaseDate(#[source] rusqlite::Error),
    #[error("failed to get watch date from row")]
    GetWatchDate(#[source] rusqlite::Error),
    #[error("failed to get rating id from row")]
    GetRatingId(#[source] rusqlite::Error),
    #[error("not enough rows in query response")]
    NotEnoughRows,
    #[error("too many rows in query response")]
    TooManyRows,
}

#[derive(Error, Debug)]
pub enum DeleteMovieError {
    #[error("failed to start transaction")]
    StartTransaction(#[source] rusqlite::Error),
    #[error("failed to remove ratings")]
    RemoveRatings(#[source] rusqlite::Error),
    #[error("failed to remove watch status")]
    RemoveWatchStatus(#[source] rusqlite::Error),
    #[error("failed to remove image")]
    RemoveImage(#[source] rusqlite::Error),
    #[error("failed to remove movie")]
    RemoveMovie(#[source] rusqlite::Error),
    #[error("failed to commit transaction")]
    Commit(#[source] rusqlite::Error),
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

    pub fn add_show(&mut self, show: &RemoteTvShow<TvMazeShowId>) -> Result<ShowId, AddShowError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(AddShowError::StartTransaction)?;

        let image_id = insert_image_into_images_table(&transaction, show.image.as_deref())
            .map_err(AddShowError::Insert)?;

        transaction
            .execute(
                "
            INSERT INTO shows(name, tvmaze_id, year, imdb_id, tvdb_id, image_id, tvmaze_url)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
                params![
                    show.name,
                    show.id.0,
                    show.year,
                    show.imdb_id.as_ref().map(|x| x.0.clone()),
                    show.tvdb_id.map(|x| x.0),
                    image_id,
                    show.url
                ],
            )
            .map_err(AddShowError::Insert)?;

        let last_show_id = ShowId(transaction.last_insert_rowid());
        transaction
            .commit()
            .map_err(AddShowError::CommitTransaction)?;
        Ok(last_show_id)
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
                DELETE FROM episode_watch_status WHERE episode_id IN (
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
            .execute(
                "DELETE FROM images WHERE id = (SELECT image_id from shows where id = ?1)",
                [show.0],
            )
            .map_err(RemoveShowError::RemoveImage)?;

        transaction
            .execute("DELETE FROM shows WHERE id = ?1", [show.0])
            .map_err(RemoveShowError::RemoveShow)?;

        transaction
            .execute_batch("PRAGMA foreign_key_check")
            .map_err(RemoveShowError::ForeignKeyCheck)?;

        transaction
            .commit()
            .map_err(RemoveShowError::CommitTransaction)?;

        Ok(())
    }

    pub fn get_show(&self, id: &ShowId, today: &NaiveDate) -> Result<TvShow, GetShowError> {
        get_show_with_connection(&self.connection, id, today)
    }

    pub fn get_shows(&self, today: &NaiveDate) -> Result<HashMap<ShowId, TvShow>, GetShowError> {
        let mut statement = self
            .connection
            .prepare(GET_SHOWS_QUERY)
            .map_err(GetShowError::Prepare)?;

        let mut rows = statement
            .query([today.num_days_from_ce()])
            .map_err(GetShowError::Execute)?;
        let mut ret = HashMap::new();

        loop {
            let row = rows.next().map_err(GetShowError::GetRow)?;

            let row = match row {
                Some(v) => v,
                None => break,
            };

            let show = show_from_row_indices(
                row,
                ShowIndices {
                    id: 0,
                    remote_id: 1,
                    name: 2,
                    image: 3,
                    year: 4,
                    url: 5,
                    imdb_id: 6,
                    tvdb_id: 7,
                    episodes_watched: 8,
                    num_episodes: 9,
                    pause_status: 10,
                    rating_id: 11,
                },
            )?;

            ret.insert(show.id, show);
        }

        Ok(ret)
    }

    fn find_episode(
        &mut self,
        show_id: &ShowId,
        episode: &RemoteEpisode,
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
        episode: &RemoteEpisode,
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

    pub fn get_episode(&self, episode_id: &EpisodeId) -> Result<TvEpisode, GetEpisodeError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, show_id, name, season, episode, airdate, watch_date FROM episodes LEFT JOIN episode_watch_status ON episodes.id = episode_watch_status.episode_id WHERE id = ?1")
            .map_err(GetEpisodeError::Prepare)?;

        let mut rows = statement
            .query([episode_id.0])
            .map_err(GetEpisodeError::Execute)?;

        let row = match rows.next().map_err(GetEpisodeError::GetRow)? {
            Some(v) => v,
            None => return Err(GetEpisodeError::IncorrectLen),
        };

        let episode = episode_from_row_indices(
            row,
            EpisodeIndices {
                id: 0,
                show_id: 1,
                name: 2,
                season: 3,
                episode: 4,
                airdate: 5,
                watch_date: 6,
            },
        )?;

        Ok(episode)
    }

    pub fn get_episodes_for_show(
        &self,
        show: &ShowId,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, GetEpisodeError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, show_id, name, season, episode, airdate, watch_date FROM episodes LEFT JOIN episode_watch_status ON episodes.id = episode_watch_status.episode_id WHERE show_id = ?1")
            .map_err(GetEpisodeError::Prepare)?;

        let mut rows = statement
            .query([show.0])
            .map_err(GetEpisodeError::Execute)?;

        let mut ret = HashMap::new();

        while let Ok(Some(row)) = rows.next() {
            let episode = episode_from_row_indices(
                row,
                EpisodeIndices {
                    id: 0,
                    show_id: 1,
                    name: 2,
                    season: 3,
                    episode: 4,
                    airdate: 5,
                    watch_date: 6,
                },
            )?;

            ret.insert(episode.id, episode);
        }

        Ok(ret)
    }

    pub fn set_episode_watch_status(
        &mut self,
        episode: &EpisodeId,
        watched: &Option<NaiveDate>,
    ) -> Result<(), SetWatchStatusError> {
        if let Some(date) = watched {
            self.connection
                .execute(
                    "
                    INSERT OR IGNORE INTO episode_watch_status(episode_id, watch_date)
                    VALUES (?1, ?2)
                    ",
                    params![episode.0, date.num_days_from_ce()],
                )
                .map_err(SetWatchStatusError)?;
        } else {
            self.connection
                .execute(
                    "
                    DELETE FROM episode_watch_status
                    WHERE episode_id = ?1
                    ",
                    [episode.0],
                )
                .map_err(SetWatchStatusError)?;
        }

        Ok(())
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

    pub fn get_episodes_aired_between(
        &mut self,
        start_date: &NaiveDate,
        end_date: &NaiveDate,
    ) -> Result<HashMap<EpisodeId, TvEpisode>, GetEpisodeError> {
        let mut ret = HashMap::new();

        let mut statement = self
            .connection
            .prepare(
                "
                SELECT id, show_id, name, season, episode, airdate, watch_date FROM episodes
                LEFT JOIN episode_watch_status on episodes.id = episode_watch_status.episode_id
                WHERE airdate IS NOT NULL AND airdate >= ?1 AND airdate <= ?2
                ",
            )
            .map_err(GetEpisodeError::Prepare)?;

        let mut rows = statement
            .query([start_date.num_days_from_ce(), end_date.num_days_from_ce()])
            .map_err(GetEpisodeError::Execute)?;

        while let Ok(Some(row)) = rows.next() {
            let episode = episode_from_row_indices(
                row,
                EpisodeIndices {
                    id: 0,
                    show_id: 1,
                    name: 2,
                    season: 3,
                    episode: 4,
                    airdate: 5,
                    watch_date: 6,
                },
            )?;

            ret.insert(episode.id, episode);
        }

        Ok(ret)
    }

    pub fn get_ratings(&mut self) -> Result<HashMap<RatingId, Rating>, GetRatingsError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, priority FROM ratings")
            .map_err(GetRatingsError::Prepare)?;

        let mut ret = HashMap::new();

        let mut rows = statement.query([]).map_err(GetRatingsError::Query)?;

        while let Some(row) = rows.next().map_err(GetRatingsError::GetRow)? {
            let id = row.get(0).map_err(GetRatingsError::GetId)?;
            let id = RatingId(id);

            let name = row.get(1).map_err(GetRatingsError::GetName)?;
            let priority = row.get(2).map_err(GetRatingsError::GetPriority)?;
            let rating = Rating { id, name, priority };
            ret.insert(id, rating);
        }

        Ok(ret)
    }

    pub fn get_rating(&mut self, id: &RatingId) -> Result<Rating, GetRatingsError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, priority FROM ratings WHERE id = ?1")
            .map_err(GetRatingsError::Prepare)?;

        let mut rows = statement.query([id.0]).map_err(GetRatingsError::Query)?;

        match rows.next().map_err(GetRatingsError::GetRow)? {
            Some(row) => {
                let id = row.get(0).map_err(GetRatingsError::GetId)?;
                let id = RatingId(id);

                let name = row.get(1).map_err(GetRatingsError::GetName)?;
                let priority = row.get(2).map_err(GetRatingsError::GetPriority)?;
                Ok(Rating { id, name, priority })
            }
            None => Err(GetRatingsError::Missing),
        }
    }

    pub fn add_rating(&mut self, name: &str) -> Result<RatingId, AddRatingError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(AddRatingError::Transaction)?;

        let priority = {
            let mut statement = transaction
                .prepare("SELECT MAX(priority) FROM ratings")
                .map_err(AddRatingError::PrepareGetPriority)?;

            let mut rows = statement
                .query([])
                .map_err(AddRatingError::QueryGetPriority)?;

            let priority = match rows.next() {
                Ok(Some(v)) => v
                    .get::<usize, Option<i64>>(0)
                    .map_err(AddRatingError::GetPriorityRow)?
                    .unwrap_or(0),
                Ok(None) => 0,
                Err(e) => return Err(AddRatingError::GetPriorityRow(e)),
            };

            match rows.next() {
                Ok(None) => (),
                _ => panic!("Unexpected extra row"),
            };
            priority
        };

        transaction
            .execute(
                "
                INSERT INTO ratings(name, priority)
                VALUES (?1, ?2)
                ",
                params![name, priority + 1],
            )
            .map_err(AddRatingError::Insert)?;

        let last_row_id = RatingId(transaction.last_insert_rowid());

        transaction
            .commit()
            .map_err(AddRatingError::CommitTransaction)?;

        Ok(last_row_id)
    }

    pub fn update_rating(&self, rating: &Rating) -> Result<(), rusqlite::Error> {
        self.connection.execute(
            "
                UPDATE ratings SET name = ?2, priority = ?3
                WHERE id = ?1
                ",
            params![rating.id.0, rating.name, rating.priority],
        )?;
        Ok(())
    }

    pub fn delete_rating(&mut self, rating: &RatingId) -> Result<(), DeleteRatingError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(DeleteRatingError::Transaction)?;

        transaction
            .execute(
                "
            DELETE FROM show_ratings WHERE rating_id = ?1
            ",
                [rating.0],
            )
            .map_err(DeleteRatingError::DeleteShowRatings)?;

        transaction
            .execute(
                "
            DELETE FROM ratings WHERE id = ?1
            ",
                [rating.0],
            )
            .map_err(DeleteRatingError::DeleteRating)?;

        transaction
            .commit()
            .map_err(DeleteRatingError::CommitTransaction)?;

        Ok(())
    }

    pub fn set_show_rating(
        &self,
        show_id: &ShowId,
        rating_id: &Option<RatingId>,
    ) -> Result<(), SetShowRatingError> {
        if let Some(rating_id) = rating_id {
            self.connection
                .execute(
                    "
                    INSERT INTO show_ratings(show_id, rating_id)
                    VALUES (?1, ?2)
                    ON CONFLICT(show_id) DO UPDATE SET rating_id = ?2
                    ",
                    [show_id.0, rating_id.0],
                )
                .map_err(SetShowRatingError)?;
        } else {
            self.connection
                .execute(
                    "
                    DELETE FROM show_ratings WHERE show_id = ?1
                    ",
                    [show_id.0],
                )
                .map_err(SetShowRatingError)?;
        }

        Ok(())
    }

    pub fn get_image_url(&self, image_id: &ImageId) -> Result<String, GetImageUrlError> {
        let mut statement = self
            .connection
            .prepare("SELECT url FROM images WHERE id = ?1")
            .map_err(GetImageUrlError::Prepare)?;

        let mut rows = statement
            .query([image_id.0])
            .map_err(GetImageUrlError::Query)?;

        let row = match rows.next().map_err(GetImageUrlError::GetRow)? {
            Some(v) => v,
            None => {
                return Err(GetImageUrlError::MissingRow);
            }
        };

        let url = row.get(0).map_err(GetImageUrlError::GetImageUrl)?;
        Ok(url)
    }

    pub fn add_movie(&mut self, movie: &RemoteMovie) -> Result<MovieId, AddMovieError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(AddMovieError::StartTransaction)?;

        let id = if let Some(id) =
            find_remote_movie_in_db(&transaction, movie).map_err(AddMovieError::FindRemote)?
        {
            // If the movie was already added, we were likely happy with the name/year/imdb id etc.
            // The only thing we really want to update is the release dates, as those could change
            // with time
            transaction
                .execute(
                    "
                    UPDATE movies
                    SET theater_release_date = ?2, home_release_date = ?3
                    WHERE id = ?1
                    ",
                    params![
                        id.0,
                        movie
                            .theater_release_date
                            .map(|date| date.num_days_from_ce()),
                        movie.home_release_date.map(|date| date.num_days_from_ce()),
                    ],
                )
                .map_err(AddMovieError::Insert)?;
            id
        } else {
            let image_id = insert_image_into_images_table(&transaction, Some(&movie.image))
                .expect("movie should always have image");

            transaction
                .execute(
                    "
                    INSERT INTO
                    movies(imdb_id, name, year, image, theater_release_date, home_release_date)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ",
                    params![
                        movie.imdb_id,
                        movie.name,
                        movie.year,
                        image_id,
                        movie
                            .theater_release_date
                            .map(|date| date.num_days_from_ce()),
                        movie.home_release_date.map(|date| date.num_days_from_ce()),
                    ],
                )
                .map_err(AddMovieError::Insert)?;
            MovieId(transaction.last_insert_rowid())
        };

        transaction.commit().map_err(AddMovieError::Commit)?;

        Ok(id)
    }

    pub fn get_movies(&self) -> Result<Vec<Movie>, GetMovieError> {
        get_movies(&self.connection, None)
    }

    pub fn get_movie(&self, id: &MovieId) -> Result<Movie, GetMovieError> {
        let mut movies = get_movies(&self.connection, Some(*id))?;

        if movies.len() > 1 {
            return Err(GetMovieError::TooManyRows);
        }

        movies.pop().ok_or(GetMovieError::NotEnoughRows)
    }

    pub fn delete_movie(&mut self, id: &MovieId) -> Result<(), DeleteMovieError> {
        let transaction = self
            .connection
            .transaction()
            .map_err(DeleteMovieError::StartTransaction)?;

        transaction
            .execute(
                "
            DELETE FROM movie_ratings WHERE movie_id = ?1
            ",
                [id.0],
            )
            .map_err(DeleteMovieError::RemoveRatings)?;

        transaction
            .execute(
                "
            DELETE FROM movie_watch_status WHERE movie_id = ?1
            ",
                [id.0],
            )
            .map_err(DeleteMovieError::RemoveWatchStatus)?;

        transaction
            .execute(
                "DELETE FROM images WHERE id = (SELECT image FROM movies WHERE id = ?1)",
                [id.0],
            )
            .map_err(DeleteMovieError::RemoveImage)?;

        transaction
            .execute(
                "
            DELETE FROM movies WHERE id = ?1
            ",
                [id.0],
            )
            .map_err(DeleteMovieError::RemoveMovie)?;

        transaction.commit().map_err(DeleteMovieError::Commit)?;

        Ok(())
    }

    pub fn set_movie_rating(
        &self,
        movie_id: &MovieId,
        rating_id: &Option<RatingId>,
    ) -> Result<(), SetShowRatingError> {
        if let Some(rating_id) = rating_id {
            self.connection
                .execute(
                    "
                    INSERT INTO movie_ratings(movie_id, rating_id)
                    VALUES (?1, ?2)
                    ON CONFLICT(movie_id) DO UPDATE SET rating_id = ?2
                    ",
                    [movie_id.0, rating_id.0],
                )
                .map_err(SetShowRatingError)?;
        } else {
            self.connection
                .execute(
                    "
                    DELETE FROM movie_ratings WHERE movie_id = ?1
                    ",
                    [movie_id.0],
                )
                .map_err(SetShowRatingError)?;
        }

        Ok(())
    }

    pub fn set_movie_watch_status(
        &self,
        id: &MovieId,
        watched: &Option<NaiveDate>,
    ) -> Result<(), SetWatchStatusError> {
        if let Some(date) = watched {
            self.connection
                .execute(
                    "
                    INSERT OR IGNORE INTO movie_watch_status(movie_id, watch_date)
                    VALUES (?1, ?2)
                    ",
                    params![id.0, date.num_days_from_ce()],
                )
                .map_err(SetWatchStatusError)?;
        } else {
            self.connection
                .execute(
                    "
                    DELETE FROM movie_watch_status
                    WHERE movie_id = ?1
                    ",
                    [id.0],
                )
                .map_err(SetWatchStatusError)?;
        }

        Ok(())
    }
}

fn show_ids_to_comma_separated<'a, I: Iterator<Item = &'a ShowId>>(mut it: I) -> String {
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

fn get_show_with_connection(
    connection: &Connection,
    id: &ShowId,
    today: &NaiveDate,
) -> Result<TvShow, GetShowError> {
    let mut filter_ids = HashSet::new();
    filter_ids.insert(*id);
    let mut ret = get_shows_with_filter(connection, today, Some(filter_ids))?;

    if ret.len() != 1 {
        return Err(GetShowError::IncorrectLen);
    }

    Ok(ret.pop().expect("Inserted show does not exist"))
}

fn get_movies(
    connection: &Connection,
    movie_id: Option<MovieId>,
) -> Result<Vec<Movie>, GetMovieError> {
    use GetMovieError::*;

    let mut query_str =
        "
        SELECT id, imdb_id, name, year, image, theater_release_date, home_release_date, movie_watch_status.watch_date, movie_ratings.rating_id
        FROM movies
        LEFT JOIN movie_watch_status ON movies.id = movie_watch_status.movie_id
        LEFT JOIN movie_ratings ON movies.id = movie_ratings.movie_id
        ".to_string();

    if let Some(movie_id) = movie_id {
        query_str.push_str(&format!("WHERE id = {}", movie_id.0));
    }

    let mut statement = connection.prepare(&query_str).map_err(Prepare)?;

    let mut rows = statement.query([]).map_err(Execute)?;

    let mut ret = Vec::new();
    while let Some(row) = rows.next().map_err(GetMovieError::GetRow)? {
        let id = row.get(0).map_err(GetId)?;
        let id = MovieId(id);

        let imdb_id = row.get(1).map_err(GetImdbId)?;
        let name = row.get(2).map_err(GetName)?;
        let year = row.get(3).map_err(GetYear)?;
        let image: i64 = row.get(4).map_err(GetImage)?;
        let image = ImageId(image);

        let theater_release_date: Option<i32> = row.get(5).map_err(GetTheaterReleaseDate)?;
        let theater_release_date =
            theater_release_date.and_then(NaiveDate::from_num_days_from_ce_opt);

        let home_release_date: Option<i32> = row.get(6).map_err(GetHomeReleaseDate)?;
        let home_release_date = home_release_date.and_then(NaiveDate::from_num_days_from_ce_opt);

        let watch_date: Option<i64> = row.get(7).map_err(GetWatchDate)?;
        let watched = watch_date.is_some();
        let rating_id: Option<i64> = row.get(8).map_err(GetRatingId)?;
        let rating_id = rating_id.map(RatingId);

        ret.push(Movie {
            id,
            imdb_id,
            name,
            year,
            image,
            watched,
            rating_id,
            theater_release_date,
            home_release_date,
        });
    }

    Ok(ret)
}

fn get_shows_with_filter(
    connection: &Connection,
    today: &NaiveDate,
    show_ids: Option<HashSet<ShowId>>,
) -> Result<Vec<TvShow>, GetShowError> {
    let mut query_str = GET_SHOWS_QUERY.to_string();

    // AFAICT, this is the easiest way to inject the show IDs retrieved from the previous step,
    // happy to be proven wrong
    if let Some(show_ids) = show_ids {
        query_str.push_str(&format!(
            "WHERE shows.id IN ({})",
            show_ids_to_comma_separated(show_ids.iter())
        ));
    }

    let mut statement = connection
        .prepare(&query_str)
        .map_err(GetShowError::Prepare)?;

    let mut rows = statement
        .query([today.num_days_from_ce()])
        .map_err(GetShowError::Execute)?;
    let mut ret = Vec::new();

    while let Some(row) = rows.next().map_err(GetShowError::GetRow)? {
        let show = show_from_row_indices(
            row,
            ShowIndices {
                id: 0,
                remote_id: 1,
                name: 2,
                image: 3,
                year: 4,
                url: 5,
                imdb_id: 6,
                tvdb_id: 7,
                episodes_watched: 8,
                num_episodes: 9,
                pause_status: 10,
                rating_id: 11,
            },
        )?;

        ret.push(show);
    }

    Ok(ret)
}

struct EpisodeIndices {
    id: usize,
    show_id: usize,
    name: usize,
    season: usize,
    episode: usize,
    airdate: usize,
    watch_date: usize,
}

fn episode_from_row_indices(
    row: &Row,
    indices: EpisodeIndices,
) -> Result<TvEpisode, GetEpisodeError> {
    let id = row.get(indices.id).map_err(GetEpisodeError::GetId)?;
    let id = EpisodeId(id);

    let show_id = row
        .get(indices.show_id)
        .map_err(GetEpisodeError::GetShowId)?;
    let show_id = ShowId(show_id);

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

    let watch_date: Option<i32> = row
        .get(indices.watch_date)
        .map_err(GetEpisodeError::GetWatchdate)?;
    let watch_date = match watch_date {
        Some(v) => {
            Some(NaiveDate::from_num_days_from_ce_opt(v).ok_or(GetEpisodeError::InvalidDate)?)
        }
        None => None,
    };

    Ok(TvEpisode {
        id,
        show_id,
        name,
        season,
        episode,
        airdate,
        watch_date,
    })
}

struct ShowIndices {
    id: usize,
    remote_id: usize,
    name: usize,
    image: usize,
    year: usize,
    url: usize,
    imdb_id: usize,
    tvdb_id: usize,
    episodes_watched: usize,
    num_episodes: usize,
    pause_status: usize,
    rating_id: usize,
}

fn show_from_row_indices(row: &Row, indices: ShowIndices) -> Result<TvShow, GetShowError> {
    let id = row.get(indices.id).map_err(GetShowError::GetId)?;
    let id = ShowId(id);

    let remote_id = row
        .get(indices.remote_id)
        .map_err(GetShowError::GetRemoteId)?;
    let remote_id = TvMazeShowId(remote_id);

    let rating_id: Option<i64> = row
        .get(indices.rating_id)
        .map_err(GetShowError::GetRatingId)?;
    let rating_id = rating_id.map(RatingId);

    let name = row.get(indices.name).map_err(GetShowError::GetName)?;

    let year = row.get(indices.year).map_err(GetShowError::GetYear)?;
    let imdb_id: Option<String> = row.get(indices.imdb_id).map_err(GetShowError::GetImdbId)?;
    let imdb_id = imdb_id.map(ImdbShowId);

    let tvdb_id: Option<i64> = row.get(indices.tvdb_id).map_err(GetShowError::GetTvdbId)?;
    let tvdb_id = tvdb_id.map(TvdbShowId);

    let image: Option<i64> = row.get(indices.image).map_err(GetShowError::GetImageUrl)?;
    let image = image.map(ImageId);

    let url = row.get(indices.url).map_err(GetShowError::GetTvMazeUrl)?;

    let episodes_watched: Option<i64> = row
        .get(indices.episodes_watched)
        .map_err(GetShowError::GetWatchCount)?;
    let episodes_watched = episodes_watched.unwrap_or(0);

    let episodes_aired: Option<i64> = row
        .get(indices.num_episodes)
        .map_err(GetShowError::GetWatchCount)?;
    let episodes_aired = episodes_aired.unwrap_or(0);

    let pause_status: Option<i64> = row
        .get(indices.pause_status)
        .map_err(GetShowError::GetPauseStatus)?;
    let pause_status = pause_status.is_some();

    Ok(TvShow {
        id,
        remote_id,
        name,
        year,
        imdb_id,
        tvdb_id,
        image,
        url,
        pause_status,
        episodes_watched,
        episodes_aired,
        rating_id,
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

fn upgrade_v3_v4(connection: &mut Connection) -> Result<(), DbCreationError> {
    let transaction = connection
        .transaction()
        .map_err(DbCreationError::StartTransaction)?;

    transaction
        .execute_batch(
            "
            CREATE TABLE ratings(
                id INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                priority INTEGER NOT NULL
            );
            CREATE TABLE show_ratings(
                show_id INTEGER PRIMARY KEY NOT NULL,
                rating_id INTEGER NOT NULL,
                FOREIGN KEY (show_id) references shows(id),
                FOREIGN KEY (rating_id) references ratings(id)
            );
            PRAGMA user_version = 4;
            ",
        )
        .map_err(DbCreationError::CreateRatings)?;

    transaction
        .commit()
        .map_err(DbCreationError::CommitTransaction)?;

    Ok(())
}

fn upgrade_v4_v5(connection: &mut Connection) -> Result<(), DbCreationError> {
    let transaction = connection
        .transaction()
        .map_err(DbCreationError::StartTransaction)?;

    transaction
        .execute_batch(
            "
            CREATE TABLE images(
                id INTEGER PRIMARY KEY NOT NULL,
                url TEXT NOT NULL
            );
            CREATE TABLE new_shows(
                id INTEGER PRIMARY KEY NOT NULL,
                name TEXT NOT NULL,
                tvmaze_id INTEGER NOT NULL,
                year INTEGER,
                imdb_id TEXT,
                tvdb_id INTEGER,
                image_id INTEGER,
                tvmaze_url TEXT,
                FOREIGN KEY(image_id) REFERENCES images(id)
            );
            INSERT INTO images SELECT null, image_url FROM shows;
            INSERT INTO new_shows SELECT * FROM (SELECT shows.id, name, tvmaze_id, year, imdb_id, tvdb_id, images.id, tvmaze_url FROM shows LEFT JOIN images ON images.url = shows.image_url);
            DROP TABLE shows;
            ALTER TABLE new_shows RENAME TO shows;
            PRAGMA user_version = 5;
            ",
        )
        .map_err(DbCreationError::CreateRatings)?;

    transaction
        .commit()
        .map_err(DbCreationError::CommitTransaction)?;

    Ok(())
}

fn upgrade_v5_v6(connection: &mut Connection) -> Result<(), DbCreationError> {
    let transaction = connection
        .transaction()
        .map_err(DbCreationError::StartTransaction)?;

    transaction
        .execute_batch(
            "
            CREATE TABLE movies(
                id INTEGER PRIMARY KEY NOT NULL,
                imdb_id TEXT NOT NULL,
                name TEXT NOT NULL,
                year INTEGER,
                image INTEGER,
                theater_release_date INTEGER,
                home_release_date INTEGER,
                FOREIGN KEY(image) REFERENCES images(id)
            );
            ALTER TABLE watch_status RENAME TO episode_watch_status;
            CREATE TABLE movie_watch_status(
                movie_id INTEGER PRIMARY KEY NOT NULL,
                watch_date INTEGER NOT NULL,
                FOREIGN KEY(movie_id) REFERENCES movies(id)
            );
            CREATE TABLE movie_ratings(
                movie_id INTEGER PRIMARY KEY NOT NULL,
                rating_id INTEGER NOT NULL,
                FOREIGN KEY(movie_id) REFERENCES movies(id),
                FOREIGN KEY(rating_id) REFERENCES ratings(id)
            );
            PRAGMA user_version = 6;
            ",
        )
        .map_err(DbCreationError::CreateRatings)?;

    transaction
        .commit()
        .map_err(DbCreationError::CommitTransaction)?;

    Ok(())
}

fn initialize_connection(connection: &mut Connection) -> Result<(), DbCreationError> {
    let version: usize = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(DbCreationError::GetVersion)?;

    let upgrade_functions = [
        initialize_v1_db,
        upgrade_v1_v2,
        upgrade_v2_v3,
        upgrade_v3_v4,
        upgrade_v4_v5,
        upgrade_v5_v6,
    ];

    for f in upgrade_functions.iter().skip(version) {
        f(connection)?;
    }

    let version: usize = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(DbCreationError::GetVersion)?;

    assert_eq!(version, 6);

    Ok(())
}

fn insert_image_into_images_table(
    connection: &Connection,
    url: Option<&str>,
) -> Result<Option<i64>, rusqlite::Error> {
    match url {
        Some(image_url) => {
            connection.execute(" INSERT INTO images(url) VALUES (?1)", [image_url])?;
            Ok(Some(connection.last_insert_rowid()))
        }
        None => Ok(None),
    }
}

fn find_remote_movie_in_db(
    connection: &Connection,
    movie: &RemoteMovie,
) -> Result<Option<MovieId>, rusqlite::Error> {
    let mut check_exists_statement = connection.prepare(
        "
        SELECT id FROM movies WHERE imdb_id = ?1
        ",
    )?;

    let mut rows = check_exists_statement.query([&movie.imdb_id])?;

    if let Some(row) = rows.next()? {
        let id: Option<i64> = row.get(0)?;
        Ok(id.map(MovieId))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn generate_empty_show(name: &str, id: i64) -> RemoteTvShow<TvMazeShowId> {
        RemoteTvShow {
            id: TvMazeShowId(id),
            name: name.to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        }
    }

    fn remote_from_tv_show(show: &TvShow, db: &Db) -> RemoteTvShow<TvMazeShowId> {
        let image_url = show
            .image
            .map(|id| db.get_image_url(&id).expect("Failed to get image"));

        RemoteTvShow {
            id: show.remote_id.clone(),
            name: show.name.clone(),
            year: show.year.clone(),
            url: show.url.clone(),
            image: image_url,
            imdb_id: show.imdb_id.clone(),
            tvdb_id: show.tvdb_id.clone(),
        }
    }

    fn remote_from_movie(movie: &Movie, db: &Db) -> RemoteMovie {
        let image_url = db.get_image_url(&movie.image).expect("Failed to get image");

        RemoteMovie {
            imdb_id: movie.imdb_id.clone(),
            name: movie.name.clone(),
            year: movie.year,
            image: image_url.clone(),
            theater_release_date: movie.theater_release_date,
            home_release_date: movie.home_release_date,
        }
    }

    fn remote_from_tv_episode(show: &TvEpisode) -> RemoteEpisode {
        RemoteEpisode {
            name: show.name.clone(),
            season: show.season.clone(),
            episode: show.episode.clone(),
            airdate: show.airdate.clone(),
        }
    }

    fn gen_date(num_days_since_ce: i32) -> NaiveDate {
        NaiveDate::from_num_days_from_ce_opt(num_days_since_ce).expect("Failed to generate date")
    }

    fn find_show(id: ShowId, shows: &HashMap<ShowId, TvShow>) -> TvShow {
        shows[&id].clone()
    }

    #[test]
    fn test_full_show_in_out() {
        let show = RemoteTvShow {
            id: TvMazeShowId(0),
            name: "Test Show".to_string(),
            image: Some("test_url".to_string()),
            year: Some(1234),
            url: Some("tvmaze_url".to_string()),
            imdb_id: Some(ImdbShowId("imdbid".to_string())),
            tvdb_id: Some(TvdbShowId(12)),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db.add_show(&show).expect("Failed to add show");

        let inserted_show = db
            .get_show(&show_id, &gen_date(1234))
            .expect("Failed to get show");
        assert_eq!(remote_from_tv_show(&inserted_show, &db), show);

        let retrieved_shows = db.get_shows(&gen_date(1234)).expect("Failed to get show");

        assert_eq!(retrieved_shows.len(), 1);
        assert_eq!(find_show(show_id, &retrieved_shows), inserted_show);
    }

    #[test]
    fn test_empty_show_in_out() {
        let show = generate_empty_show("Test Show", 0);

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db.add_show(&show).expect("Failed to add show");

        let inserted_show = db
            .get_show(&show_id, &gen_date(1234))
            .expect("Failed to get show");
        assert_eq!(remote_from_tv_show(&inserted_show, &db), show);

        let retrieved_shows = db.get_shows(&gen_date(1234)).expect("Failed to get show");

        assert_eq!(retrieved_shows.len(), 1);
        assert_eq!(find_show(show_id, &retrieved_shows), inserted_show);
    }

    #[test]
    fn test_get_shows() {
        let show = generate_empty_show("Test Show", 0);
        let show2 = generate_empty_show("Test show 2", 1);

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id1 = db.add_show(&show).expect("Failed to add show");
        let show_id2 = db.add_show(&show2).expect("Failed to add show");

        let shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");

        assert_eq!(shows.len(), 2);
        assert_eq!(find_show(show_id1, &shows).remote_id, TvMazeShowId(0));
        assert_eq!(remote_from_tv_show(&find_show(show_id1, &shows), &db), show);
        assert_eq!(find_show(show_id2, &shows).remote_id, TvMazeShowId(1));
        assert_eq!(
            remote_from_tv_show(&find_show(show_id2, &shows), &db),
            show2
        );
    }

    #[test]
    fn test_episode_in_out() {
        let show = generate_empty_show("Test Show", 0);

        let episode = RemoteEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db.add_show(&show).expect("Failed to add show");

        let id = db
            .add_episode(&show_id, &episode)
            .expect("Failed to add episode");

        let retrieved_episodes = db
            .get_episodes_for_show(&show_id)
            .expect("Failed to get episodes");

        assert_eq!(retrieved_episodes.len(), 1);
        assert_eq!(remote_from_tv_episode(&retrieved_episodes[&id]), episode);
        assert_eq!(retrieved_episodes[&id].show_id, show_id);
    }

    #[test]
    fn test_update_episode() {
        let show = generate_empty_show("Test Show", 0);

        let episode = RemoteEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let episode_update = RemoteEpisode {
            name: "Test Episode updated".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1024),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db.add_show(&show).expect("Failed to add show");

        let id = db
            .add_episode(&show_id, &episode)
            .expect("Failed to add episode");

        db.add_episode(&show_id, &episode_update)
            .expect("Failed to add episode");

        let retrieved_episodes = db
            .get_episodes_for_show(&show_id)
            .expect("Failed to get episodes");

        assert_eq!(retrieved_episodes.len(), 1);
        assert_eq!(
            remote_from_tv_episode(&retrieved_episodes[&id]),
            episode_update
        );
    }

    #[test]
    fn test_set_watch_status() {
        let show = generate_empty_show("Test Show", 0);

        let episode = RemoteEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let episode2 = RemoteEpisode {
            name: "Test Episode 2".to_string(),
            season: 1,
            episode: 35,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db.add_show(&show).expect("Failed to add show");

        let episode_id = db
            .add_episode(&show_id, &episode)
            .expect("Failed to add episode");

        db.add_episode(&show_id, &episode2)
            .expect("Failed to add episode");

        let watch_date = NaiveDate::from_num_days_from_ce_opt(1024).expect("Invalid date");

        db.set_episode_watch_status(&episode_id, &Some(watch_date))
            .expect("Failed to set watch status");

        let retrieved_episode = db.get_episode(&episode_id).expect("Failed to get episodes");

        assert_eq!(retrieved_episode.watch_date, Some(watch_date));

        db.set_episode_watch_status(&episode_id, &None)
            .expect("Failed to set watch status");

        let retrieved_episode = db.get_episode(&episode_id).expect("Failed to get episodes");

        assert_eq!(retrieved_episode.watch_date, None);
    }

    #[test]
    fn test_set_pause_status() {
        let show = generate_empty_show("Test Show", 0);
        let show2 = generate_empty_show("Test Show 2", 1);

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id1 = db.add_show(&show).expect("Failed to add show");
        let show_id2 = db.add_show(&show2).expect("Failed to add show");

        let shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");
        assert_eq!(find_show(show_id1, &shows).pause_status, false);
        assert_eq!(find_show(show_id2, &shows).pause_status, false);

        db.set_pause_status(&show_id1, false)
            .expect("Failed to set pause");
        let shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");
        assert_eq!(find_show(show_id1, &shows).pause_status, false);
        assert_eq!(find_show(show_id2, &shows).pause_status, false);

        db.set_pause_status(&show_id1, true)
            .expect("Failed to set pause");
        let shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");
        assert_eq!(find_show(show_id1, &shows).pause_status, true);
        assert_eq!(find_show(show_id2, &shows).pause_status, false);

        db.set_pause_status(&show_id1, false)
            .expect("Failed to set pause");
        let shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");
        assert_eq!(find_show(show_id1, &shows).pause_status, false);
        assert_eq!(find_show(show_id2, &shows).pause_status, false);
    }

    #[test]
    fn test_remove_show() {
        let mut show = generate_empty_show("Test Show", 0);
        show.image = Some("test_image".to_string());

        let episode = RemoteEpisode {
            name: "Test Episode".to_string(),
            season: 1,
            episode: 34,
            airdate: NaiveDate::from_num_days_from_ce_opt(1023),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let show_id = db.add_show(&show).expect("Failed to add show");
        let inserted_show = db
            .get_show(&show_id, &gen_date(1234))
            .expect("failed to get show");
        let image_id = inserted_show.image.expect("invalid image");
        db.get_image_url(&image_id)
            .expect("Failed to get image url");

        db.set_pause_status(&show_id, true)
            .expect("Failed to set pause");

        let episode_id = db
            .add_episode(&show_id, &episode)
            .expect("Failed to add episode");

        let watch_date = NaiveDate::from_num_days_from_ce_opt(1024).expect("Invalid date");

        db.set_episode_watch_status(&episode_id, &Some(watch_date))
            .expect("Failed to set watch status");

        let episodes = db
            .get_episodes_for_show(&show_id)
            .expect("Failed to get episodes");
        assert_eq!(episodes.len(), 1);

        db.remove_show(&show_id).expect("Failed to remove show");

        assert!(db.get_image_url(&image_id).is_err());

        let episodes = db
            .get_episodes_for_show(&show_id)
            .expect("Failed to get episodes");
        assert_eq!(episodes.len(), 0);
    }

    #[test]
    fn test_shows_aired_between() {
        let show = generate_empty_show("Test Show", 0);
        let show2 = generate_empty_show("Test Show 2", 0);
        let show3 = generate_empty_show("Test Show 3", 0);

        let mut db = Db::new_in_memory().expect("Failed to create db");
        let show_id1 = db.add_show(&show).expect("Failed to add show 1");
        let show_id2 = db.add_show(&show2).expect("Failed to add show 2");

        // Show 3 has no episodes
        db.add_show(&show3).expect("Failed to add show 3");

        let show_ids = [show_id1, show_id2];
        for i in 0..100 {
            let show_id = show_ids[(i % 2) as usize];
            let episode = RemoteEpisode {
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

        let episodes = db
            .get_episodes_aired_between(&start_date, &end_date)
            .expect("Failed to get aired episodes");

        // Airdates are inclusive, so we should expect 3 days of 4 episodes a day
        assert_eq!(episodes.len(), 12);
        let min_date = episodes
            .values()
            .min_by(|a, b| a.airdate.cmp(&b.airdate))
            .and_then(|x| x.airdate);
        let max_date = episodes
            .values()
            .max_by(|a, b| a.airdate.cmp(&b.airdate))
            .and_then(|x| x.airdate);
        assert_eq!(min_date, Some(start_date));
        assert_eq!(max_date, Some(end_date));
    }

    #[test]
    fn test_ratings() {
        let mut db = Db::new_in_memory().expect("Failed to create db");
        let shows = [
            generate_empty_show("Show 1", 0),
            generate_empty_show("Show 2", 1),
            generate_empty_show("Show 3", 2),
        ];
        let ids: Vec<_> = shows
            .iter()
            .map(|show| db.add_show(show).expect("Failed to add show"))
            .collect();

        let ratings = ["Super good", "bad", "good", "unbearable"];

        let rating_ids: Vec<_> = ratings
            .iter()
            .map(|rating| db.add_rating(rating).expect("Failed to add rating"))
            .collect();

        let mut retrieved_ratings = db.get_ratings().expect("Failed to get ratings");
        assert_eq!(retrieved_ratings.len(), 4);
        assert_eq!(retrieved_ratings[&rating_ids[0]].priority, 1);
        assert_eq!(&retrieved_ratings[&rating_ids[0]].name, "Super good");
        assert_eq!(retrieved_ratings[&rating_ids[1]].priority, 2);
        assert_eq!(&retrieved_ratings[&rating_ids[1]].name, "bad");
        assert_eq!(retrieved_ratings[&rating_ids[2]].priority, 3);
        assert_eq!(&retrieved_ratings[&rating_ids[2]].name, "good");
        assert_eq!(retrieved_ratings[&rating_ids[3]].priority, 4);
        assert_eq!(&retrieved_ratings[&rating_ids[3]].name, "unbearable");

        retrieved_ratings
            .get_mut(&rating_ids[1])
            .expect("Failed to get rating")
            .priority = 3;
        retrieved_ratings
            .get_mut(&rating_ids[2])
            .expect("Failed to get rating")
            .priority = 2;

        db.update_rating(&retrieved_ratings[&rating_ids[1]])
            .expect("Failed to set rating order");
        db.update_rating(&retrieved_ratings[&rating_ids[2]])
            .expect("Failed to set rating order");

        let retrieved_ratings = db.get_ratings().expect("Failed to get ratings");
        assert_eq!(retrieved_ratings.len(), 4);
        assert_eq!(retrieved_ratings[&rating_ids[0]].priority, 1);
        assert_eq!(&retrieved_ratings[&rating_ids[0]].name, "Super good");
        assert_eq!(retrieved_ratings[&rating_ids[1]].priority, 3);
        assert_eq!(&retrieved_ratings[&rating_ids[1]].name, "bad");
        assert_eq!(retrieved_ratings[&rating_ids[2]].priority, 2);
        assert_eq!(&retrieved_ratings[&rating_ids[2]].name, "good");
        assert_eq!(retrieved_ratings[&rating_ids[3]].priority, 4);
        assert_eq!(&retrieved_ratings[&rating_ids[3]].name, "unbearable");

        db.set_show_rating(&ids[0], &Some(rating_ids[0]))
            .expect("Failed to set rating");
        db.set_show_rating(&ids[2], &Some(rating_ids[3]))
            .expect("Failed to set rating");

        let retrieved_shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");
        assert_eq!(retrieved_shows.len(), 3);
        assert_eq!(retrieved_shows[&ids[0]].rating_id, Some(rating_ids[0]));
        assert_eq!(retrieved_shows[&ids[1]].rating_id, None);
        assert_eq!(retrieved_shows[&ids[2]].rating_id, Some(rating_ids[3]));

        db.delete_rating(&rating_ids[0])
            .expect("Failed to delete rating");

        let retrieved_ratings = db.get_ratings().expect("Failed to get ratings");
        assert_eq!(retrieved_ratings.len(), 3);
        assert_eq!(retrieved_ratings[&rating_ids[1]].priority, 3);
        assert_eq!(&retrieved_ratings[&rating_ids[1]].name, "bad");
        assert_eq!(retrieved_ratings[&rating_ids[2]].priority, 2);
        assert_eq!(&retrieved_ratings[&rating_ids[2]].name, "good");
        assert_eq!(retrieved_ratings[&rating_ids[3]].priority, 4);
        assert_eq!(&retrieved_ratings[&rating_ids[3]].name, "unbearable");

        let retrieved_shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");
        assert_eq!(retrieved_shows.len(), 3);
        assert_eq!(retrieved_shows[&ids[0]].rating_id, None);
        assert_eq!(retrieved_shows[&ids[1]].rating_id, None);
        assert_eq!(retrieved_shows[&ids[2]].rating_id, Some(rating_ids[3]));

        db.set_show_rating(&ids[2], &None)
            .expect("Failed to set rating");

        let retrieved_shows = db.get_shows(&gen_date(1234)).expect("Failed to get shows");
        assert_eq!(retrieved_shows.len(), 3);
        assert_eq!(retrieved_shows[&ids[0]].rating_id, None);
        assert_eq!(retrieved_shows[&ids[1]].rating_id, None);
        assert_eq!(retrieved_shows[&ids[2]].rating_id, None);
    }

    #[test]
    fn test_full_movie_in_out() {
        let movie = RemoteMovie {
            imdb_id: "test".to_string(),
            name: "movie".to_string(),
            year: 1234,
            image: "http://image".to_string(),
            theater_release_date: Some(gen_date(1234)),
            home_release_date: Some(gen_date(1244)),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let movie_id = db.add_movie(&movie).expect("Failed to add movie");

        let inserted_movie = db.get_movie(&movie_id).expect("Failed to get show");

        assert_eq!(remote_from_movie(&inserted_movie, &db), movie);

        let movies = db.get_movies().expect("Failed to get movie");
        assert_eq!(movies.len(), 1);
        assert_eq!(inserted_movie, movies[0]);
    }

    #[test]
    fn test_add_duplicate_movie() {
        let movie = RemoteMovie {
            imdb_id: "test".to_string(),
            name: "movie".to_string(),
            year: 1234,
            image: "http://image".to_string(),
            theater_release_date: Some(gen_date(1234)),
            home_release_date: Some(gen_date(1244)),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");

        let movie_id = db.add_movie(&movie).expect("Failed to add movie");
        let movie_id2 = db.add_movie(&movie).expect("Failed to add movie");
        assert_eq!(movie_id, movie_id2);
    }

    #[test]
    fn test_watch_movie() {
        let movie = RemoteMovie {
            imdb_id: "test".to_string(),
            name: "movie".to_string(),
            year: 1234,
            image: "http://image".to_string(),
            theater_release_date: Some(gen_date(1234)),
            home_release_date: Some(gen_date(1244)),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");
        let movie_id = db.add_movie(&movie).expect("Failed to add movie");
        let movie = db.get_movie(&movie_id).expect("Failed to get movie");
        assert_eq!(movie.watched, false);

        db.set_movie_watch_status(&movie_id, &Some(gen_date(1234)))
            .expect("Failed to set watch status");
        let movie = db.get_movie(&movie_id).expect("Failed to get movie");
        assert_eq!(movie.watched, true);

        db.set_movie_watch_status(&movie_id, &None)
            .expect("Failed to set watch status");
        let movie = db.get_movie(&movie_id).expect("Failed to get movie");
        assert_eq!(movie.watched, false);
    }

    #[test]
    fn test_rate_movie() {
        let movie = RemoteMovie {
            imdb_id: "test".to_string(),
            name: "movie".to_string(),
            year: 1234,
            image: "http://image".to_string(),
            theater_release_date: Some(gen_date(1234)),
            home_release_date: Some(gen_date(1244)),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");
        let movie_id = db.add_movie(&movie).expect("Failed to add movie");
        let movie = db.get_movie(&movie_id).expect("Failed to get movie");
        assert_eq!(movie.rating_id, None);

        let rating_id = db.add_rating("test").expect("Failed to add rating");

        db.set_movie_rating(&movie_id, &Some(rating_id))
            .expect("Failed to add rating");
        let movie = db.get_movie(&movie_id).expect("Failed to get movie");
        assert_eq!(movie.rating_id, Some(rating_id));

        db.set_movie_rating(&movie_id, &None)
            .expect("Failed to add rating");
        let movie = db.get_movie(&movie_id).expect("Failed to get movie");
        assert_eq!(movie.rating_id, None);
    }

    #[test]
    fn test_delete_movie() {
        let movie = RemoteMovie {
            imdb_id: "test".to_string(),
            name: "movie".to_string(),
            year: 1234,
            image: "http://image".to_string(),
            theater_release_date: Some(gen_date(1234)),
            home_release_date: Some(gen_date(1244)),
        };

        let mut db = Db::new_in_memory().expect("Failed to create db");
        let movie_id = db.add_movie(&movie).expect("Failed to add movie");
        let inserted_movie = db.get_movie(&movie_id).expect("Failed to get movie");
        db.get_image_url(&inserted_movie.image)
            .expect("Failed to get image url");

        db.delete_movie(&movie_id).expect("failed to delete movie");
        assert!(db.get_movie(&movie_id).is_err());
        assert!(db.get_image_url(&inserted_movie.image).is_err());
        assert_eq!(db.get_movies().expect("failed to get movies").len(), 0);
    }
}
