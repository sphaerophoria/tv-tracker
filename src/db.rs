use crate::{
    tv_maze::TvMazeShowId,
    types::{ImdbShowId, ShowId, TvShow, TvdbShowId},
};

use rusqlite::{params, Connection};
use thiserror::Error;

use std::{collections::HashMap, path::Path};

#[derive(Debug, Error)]
pub enum DbCreationError {
    #[error("failed to open sqlite db")]
    OpenDb(#[source] rusqlite::Error),
    #[error("failed to start transaction")]
    StartTransaction(#[source] rusqlite::Error),
    #[error("failed to commit transaction")]
    CommitTransaction(#[source] rusqlite::Error),
    #[error("failed to create show table")]
    CreateShowTable(#[source] rusqlite::Error),
}

#[derive(Debug, Error)]
#[error("failed to add show to db")]
pub struct AddShowError(#[source] rusqlite::Error);

#[derive(Debug, Error)]
pub enum GetShowError {
    #[error("failed to prepare get show request")]
    Prepare(#[source] rusqlite::Error),
    #[error("failed to execute get show request")]
    Execute(#[source] rusqlite::Error),
    #[error("too many results for query")]
    TooManyRows,
    #[error("failed to get row from query response")]
    GetRow(#[source] rusqlite::Error),
    #[error("show not found")]
    ShowNotFound,
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
        &self,
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

        Ok(ShowId::new(self.connection.last_insert_rowid()))
    }

    pub fn get_shows(&self) -> Result<HashMap<ShowId, TvMazeShowId>, GetShowError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, tvmaze_id FROM shows")
            .map_err(GetShowError::Prepare)?;

        let mut rows = statement.query(params![]).map_err(GetShowError::Execute)?;

        let mut ret = HashMap::new();
        while let Ok(Some(row)) = rows.next() {
            let id = ShowId::new(row.get(0).map_err(GetShowError::GetId)?);
            let tvmaze_id = TvMazeShowId(row.get(1).map_err(GetShowError::GetTvMazeId)?);
            ret.insert(id, tvmaze_id);
        }

        Ok(ret)
    }

    pub fn get_show(&self, show: &ShowId) -> Result<TvShow, GetShowError> {
        let mut statement = self.connection.prepare(
                "SELECT name, year, imdb_id, tvdb_id, image_url, tvmaze_url FROM shows WHERE id = ?1 ")
            .map_err(GetShowError::Prepare)?;

        let mut rows = statement.query([show.0]).map_err(GetShowError::Execute)?;

        let row = match rows.next().map_err(GetShowError::GetRow)? {
            Some(v) => v,
            None => return Err(GetShowError::ShowNotFound),
        };

        let name = row.get(0).map_err(GetShowError::GetName)?;
        let year = row.get(1).map_err(GetShowError::GetYear)?;
        let imdb_id: Option<String> = row.get(2).map_err(GetShowError::GetImdbId)?;
        let imdb_id = imdb_id.map(ImdbShowId);

        let tvdb_id: Option<i64> = row.get(3).map_err(GetShowError::GetTvdbId)?;
        let tvdb_id = tvdb_id.map(TvdbShowId);

        let image = row.get(4).map_err(GetShowError::GetImageUrl)?;
        let url = row.get(5).map_err(GetShowError::GetTvMazeUrl)?;

        if rows.next().map_err(GetShowError::GetRow)?.is_some() {
            return Err(GetShowError::TooManyRows);
        }

        Ok(TvShow {
            name,
            year,
            imdb_id,
            tvdb_id,
            image,
            url,
        })
    }
}

fn initialize_connection(connection: &mut Connection) -> Result<(), DbCreationError> {
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
            ",
        )
        .map_err(DbCreationError::CreateShowTable)?;

    transaction
        .commit()
        .map_err(DbCreationError::CommitTransaction)?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_full_in_out() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: Some("test_url".to_string()),
            year: Some(1234),
            url: Some("tvmaze_url".to_string()),
            imdb_id: Some(ImdbShowId("imdbid".to_string())),
            tvdb_id: Some(TvdbShowId(12)),
        };

        let db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");
        let retrieved_show = db.get_show(&id).expect("Failed to get show");

        assert_eq!(show, retrieved_show);
    }

    #[test]
    fn test_empty_in_out() {
        let show = TvShow {
            name: "Test Show".to_string(),
            image: None,
            year: None,
            url: None,
            imdb_id: None,
            tvdb_id: None,
        };

        let db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");
        let retrieved_show = db.get_show(&id).expect("Failed to get show");

        assert_eq!(show, retrieved_show);
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

        let db = Db::new_in_memory().expect("Failed to create db");

        let id = db
            .add_show(&show, &TvMazeShowId(0))
            .expect("Failed to add show");
        let id2 = db
            .add_show(&show2, &TvMazeShowId(1))
            .expect("Failed to add show");

        let shows = db.get_shows().expect("Failed to get shows");
        assert_eq!(shows.len(), 2);
        assert_eq!(shows[&id], TvMazeShowId(0));
        assert_eq!(shows[&id2], TvMazeShowId(1));
    }
}
