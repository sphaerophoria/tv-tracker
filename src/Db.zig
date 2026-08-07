const std = @import("std");
const types = @import("types.zig");
const sphtud = @import("sphtud");
const Shows = types.Shows;
const Show = types.Show;
const Rating = types.Rating;

const c = @import("sqlite");

sqlite: *c.sqlite3,

const Db = @This();

pub fn init(path: [:0]const u8) !Db {
    var db: ?*c.sqlite3 = null;
    try retCheck(c.sqlite3_open_v2(
        path,
        &db,
        c.SQLITE_OPEN_READWRITE | c.SQLITE_OPEN_CREATE,
        null,
    ));

    var ret: Db = .{
        .sqlite = db orelse return error.SqliteInit,
    };

    try ret.exec("PRAGMA foreign_keys = ON", &.{});

    try ret.initializeConnection();

    return ret;
}

pub fn deinit(self: *Db) void {
    if (c.sqlite3_close(self.sqlite) != c.SQLITE_OK) {
        std.log.err("Failed to close sqlite database\n", .{});
    }
}

fn initializeConnection(self: *Db) !void {
    const upgrade_functions = [_]*const fn (*Db) anyerror!void{
        initializeV1,
        upgradeV1V2,
        upgradeV2V3,
        upgradeV3V4,
        upgradeV4V5,
        upgradeV5V6,
        upgradeV6V7,
        upgradeV7V8,
        upgradeV8V9,
        upgradeV9V10,
        upgradeV10V11,
    };

    const version = try self.userVersion();
    if (version < 0 or version > upgrade_functions.len) return error.InvalidDbVersion;

    for (upgrade_functions[@intCast(version)..]) |f| {
        try f(self);
    }

    if (try self.userVersion() != upgrade_functions.len) return error.InitializeFailed;
}

fn userVersion(self: *Db) !i64 {
    const stmt = try self.prepare("PRAGMA user_version", &.{});
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => return c.sqlite3_column_int64(stmt, 0),
        else => return error.Sqlite,
    }
}

fn upgradeBatch(self: *Db, sql: [:0]const u8) !void {
    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};
    try self.exec(sql, &.{});
    try self.exec("COMMIT", &.{});
}

fn initializeV1(self: *Db) !void {
    // NOTE: Presence of lots of nullable fields may initially indicate that we
    // should be splitting our show table into multiple tables. This does not
    // make sense for our use case. We are essentially always serializing and
    // deserializing our TvShow struct in one shot. In this scenario joining
    // multiple tables just to avoid nullables does not make sense, as we lookup
    // all fields to convert them back to Option::None anyways
    try self.upgradeBatch(
        \\CREATE TABLE IF NOT EXISTS shows(
        \\    id INTEGER PRIMARY KEY NOT NULL,
        \\    name TEXT NOT NULL,
        \\    tvmaze_id INTEGER NOT NULL,
        \\    year INTEGER,
        \\    imdb_id TEXT,
        \\    tvdb_id INTEGER,
        \\    image_url TEXT,
        \\    tvmaze_url TEXT
        \\);
        \\CREATE TABLE IF NOT EXISTS episodes(
        \\    id INTEGER PRIMARY KEY NOT NULL,
        \\    show_id INTEGER NOT NULL,
        \\    name TEXT NOT NULL,
        \\    season INTEGER NOT NULL,
        \\    episode INTEGER NOT NULL,
        \\    airdate INTEGER NOT NULL,
        \\    FOREIGN KEY(show_id) REFERENCES shows(id)
        \\);
        \\CREATE TABLE IF NOT EXISTS watch_status(
        \\    episode_id INTEGER PRIMARY KEY NOT NULL,
        \\    watch_date INTEGER NOT NULL,
        \\    FOREIGN KEY(episode_id) REFERENCES episodes(id)
        \\);
        \\PRAGMA user_version = 1;
    );
}

fn upgradeV1V2(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE new_episodes(
        \\    id INTEGER PRIMARY KEY NOT NULL,
        \\    show_id INTEGER NOT NULL,
        \\    name TEXT NOT NULL,
        \\    season INTEGER NOT NULL,
        \\    episode INTEGER NOT NULL,
        \\    airdate INTEGER,
        \\    FOREIGN KEY(show_id) REFERENCES shows(id)
        \\);
        \\INSERT INTO new_episodes SELECT id, show_id, name, season, episode, airdate from episodes;
        \\DROP TABLE episodes;
        \\ALTER TABLE new_episodes RENAME TO episodes;
        \\PRAGMA user_version = 2;
    );
}

fn upgradeV2V3(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE paused_shows(
        \\    show_id INTEGER PRIMARY KEY NOT NULL,
        \\    FOREIGN KEY(show_id) REFERENCES shows(id)
        \\);
        \\PRAGMA user_version = 3;
    );
}

fn upgradeV3V4(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE ratings(
        \\    id INTEGER PRIMARY KEY NOT NULL,
        \\    name TEXT NOT NULL,
        \\    priority INTEGER NOT NULL
        \\);
        \\CREATE TABLE show_ratings(
        \\    show_id INTEGER PRIMARY KEY NOT NULL,
        \\    rating_id INTEGER NOT NULL,
        \\    FOREIGN KEY (show_id) references shows(id),
        \\    FOREIGN KEY (rating_id) references ratings(id)
        \\);
        \\PRAGMA user_version = 4;
    );
}

fn upgradeV4V5(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE images(
        \\    id INTEGER PRIMARY KEY NOT NULL,
        \\    url TEXT NOT NULL
        \\);
        \\CREATE TABLE new_shows(
        \\    id INTEGER PRIMARY KEY NOT NULL,
        \\    name TEXT NOT NULL,
        \\    tvmaze_id INTEGER NOT NULL,
        \\    year INTEGER,
        \\    imdb_id TEXT,
        \\    tvdb_id INTEGER,
        \\    image_id INTEGER,
        \\    tvmaze_url TEXT,
        \\    FOREIGN KEY(image_id) REFERENCES images(id)
        \\);
        \\INSERT INTO images SELECT null, image_url FROM shows;
        \\INSERT INTO new_shows SELECT * FROM (SELECT shows.id, name, tvmaze_id, year, imdb_id, tvdb_id, images.id, tvmaze_url FROM shows LEFT JOIN images ON images.url = shows.image_url);
        \\DROP TABLE shows;
        \\ALTER TABLE new_shows RENAME TO shows;
        \\PRAGMA user_version = 5;
    );
}

fn upgradeV5V6(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE movies(
        \\    id INTEGER PRIMARY KEY NOT NULL,
        \\    imdb_id TEXT NOT NULL,
        \\    name TEXT NOT NULL,
        \\    year INTEGER,
        \\    image INTEGER,
        \\    theater_release_date INTEGER,
        \\    home_release_date INTEGER,
        \\    FOREIGN KEY(image) REFERENCES images(id)
        \\);
        \\ALTER TABLE watch_status RENAME TO episode_watch_status;
        \\CREATE TABLE movie_watch_status(
        \\    movie_id INTEGER PRIMARY KEY NOT NULL,
        \\    watch_date INTEGER NOT NULL,
        \\    FOREIGN KEY(movie_id) REFERENCES movies(id)
        \\);
        \\CREATE TABLE movie_ratings(
        \\    movie_id INTEGER PRIMARY KEY NOT NULL,
        \\    rating_id INTEGER NOT NULL,
        \\    FOREIGN KEY(movie_id) REFERENCES movies(id),
        \\    FOREIGN KEY(rating_id) REFERENCES ratings(id)
        \\);
        \\PRAGMA user_version = 6;
    );
}

fn upgradeV6V7(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE skipped_episodes(
        \\    episode_id INTEGER NOT NULL,
        \\    FOREIGN KEY(episode_id) REFERENCES episodes(id),
        \\    UNIQUE(episode_id)
        \\);
        \\PRAGMA user_version = 7;
    );
}

fn upgradeV7V8(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE episode_watch_status_new(
        \\    episode_id INTEGER NOT NULL,
        \\    watch_date INTEGER NOT NULL,
        \\    playthrough_id INTEGER NOT NULL,
        \\    FOREIGN KEY(episode_id) REFERENCES episodes(id)
        \\);
        \\INSERT INTO episode_watch_status_new(episode_id, watch_date, playthrough_id)
        \\SELECT episode_id, watch_date, 0 FROM episode_watch_status;
        \\DROP TABLE episode_watch_status;
        \\ALTER TABLE episode_watch_status_new RENAME TO episode_watch_status;
        \\
        \\CREATE TABLE skipped_episodes_new(
        \\    episode_id INTEGER NOT NULL,
        \\    playthrough_id INTEGER NOT NULL,
        \\    FOREIGN KEY(episode_id) REFERENCES episodes(id)
        \\);
        \\INSERT INTO skipped_episodes_new(episode_id, playthrough_id)
        \\SELECT episode_id, 0 FROM skipped_episodes;
        \\DROP TABLE skipped_episodes;
        \\ALTER TABLE skipped_episodes_new RENAME TO skipped_episodes;
        \\
        \\PRAGMA user_version = 8;
    );
}

fn upgradeV8V9(self: *Db) !void {
    try self.upgradeBatch(
        \\CREATE TABLE notes(
        \\    show_id INTEGER PRIMARY KEY NOT NULL,
        \\    content TEXT NOT NULL,
        \\    FOREIGN KEY(show_id) REFERENCES shows(id)
        \\);
        \\
        \\PRAGMA user_version = 9;
    );
}

fn upgradeV9V10(self: *Db) !void {
    try self.upgradeBatch(
        \\ALTER TABLE movies ADD COLUMN last_update_time INTEGER;
        \\PRAGMA user_version = 10;
    );
}

fn upgradeV10V11(self: *Db) !void {
    try self.upgradeBatch(
        \\ALTER TABLE shows ADD COLUMN notes TEXT;
        \\UPDATE shows SET notes = (SELECT content FROM notes WHERE notes.show_id = shows.id);
        \\DROP TABLE notes;
        \\ALTER TABLE movies ADD COLUMN notes TEXT;
        \\PRAGMA user_version = 11;
    );
}

test "fresh db migrates to latest version" {
    var db = try Db.init(":memory:");
    defer _ = c.sqlite3_close(db.sqlite);

    try std.testing.expectEqual(@as(i64, 11), try db.userVersion());

    // Re-running initialization on an already-current DB is a no-op and must
    // still land on the latest version (exercises the skip-already-applied path).
    try db.initializeConnection();
    try std.testing.expectEqual(@as(i64, 11), try db.userVersion());
}

const get_shows_query =
    \\SELECT shows.id, shows.tvmaze_id, shows.name, shows.image_id, shows.year, shows.tvmaze_url, shows.imdb_id, shows.tvdb_id, epi_count.count, paused_shows.show_id, show_ratings.rating_id, shows.notes FROM shows
    \\LEFT JOIN
    \\    (
    \\        SELECT show_id, COUNT(*) as count FROM episodes
    \\        WHERE episodes.airdate <= ?1
    \\        GROUP BY show_id
    \\    ) as epi_count
    \\    ON shows.id = epi_count.show_id
    \\LEFT JOIN paused_shows ON shows.id = paused_shows.show_id
    \\LEFT JOIN show_ratings ON shows.id = show_ratings.show_id
;

pub fn getShows(self: *Db, gpa: std.mem.Allocator, now: std.Io.Timestamp) !Shows {
    const stmt = try self.prepare(get_shows_query, &.{.{ .i64 = toCeDay(now) }});
    defer _ = c.sqlite3_finalize(stmt);

    var ret: std.ArrayList(Show) = .empty;
    while (true) {
        switch (c.sqlite3_step(stmt)) {
            c.SQLITE_ROW => {
                const show = try self.showFromRowIndices(
                    gpa,
                    stmt,
                    .{
                        .id = 0,
                        .remote_id = 1,
                        .name = 2,
                        .image = 3,
                        .year = 4,
                        .url = 5,
                        .imdb_id = 6,
                        .tvdb_id = 7,
                        .num_episodes = 8,
                        .pause_status = 9,
                        .rating_id = 10,
                        .notes = 11,
                    },
                );

                try ret.append(gpa, try self.appendShowWatchStatus(gpa, show));
            },
            c.SQLITE_DONE => break,
            else => return error.Sqlite,
        }
    }

    return ret.items;
}

const get_show_query = get_shows_query ++
    \\
    \\WHERE shows.id = ?2
;

pub fn getShow(self: *Db, gpa: std.mem.Allocator, id: types.ShowId, now: std.Io.Timestamp) !?Show {
    const stmt = try self.prepare(get_show_query, &.{
        .{ .i64 = toCeDay(now) },
        .{ .i64 = id.inner },
    });
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => {
            const show = try self.showFromRowIndices(
                gpa,
                stmt,
                .{
                    .id = 0,
                    .remote_id = 1,
                    .name = 2,
                    .image = 3,
                    .year = 4,
                    .url = 5,
                    .imdb_id = 6,
                    .tvdb_id = 7,
                    .num_episodes = 8,
                    .pause_status = 9,
                    .rating_id = 10,
                    .notes = 11,
                },
            );
            return try self.appendShowWatchStatus(gpa, show);
        },
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

const get_episodes_aired_between_query =
    \\SELECT id, show_id, name, season, episode, airdate FROM episodes
    \\WHERE airdate IS NOT NULL AND airdate >= ?1 AND airdate <= ?2
;

pub fn getEpisodesAiredBetween(
    self: *Db,
    gpa: std.mem.Allocator,
    start_date: sphtud.datetime.Date,
    end_date: sphtud.datetime.Date,
) ![]const types.Episode {
    const stmt = try self.prepare(get_episodes_aired_between_query, &.{
        .{ .i64 = start_date.toCeDay() },
        .{ .i64 = end_date.toCeDay() },
    });
    defer _ = c.sqlite3_finalize(stmt);

    var ret: std.ArrayList(types.Episode) = .empty;
    while (true) {
        switch (c.sqlite3_step(stmt)) {
            c.SQLITE_ROW => {
                const episode = try self.episodeFromRowIndices(
                    gpa,
                    stmt,
                    .{
                        .id = 0,
                        .show_id = 1,
                        .name = 2,
                        .season = 3,
                        .episode = 4,
                        .airdate = 5,
                    },
                );

                const num_playthroughs = try self.numPlaythroughsForShow(episode.show_id);
                try ret.append(gpa, try self.appendEpisodeWatchStatus(gpa, episode, num_playthroughs));
            },
            c.SQLITE_DONE => break,
            else => return error.Sqlite,
        }
    }

    return ret.items;
}

const get_episodes_for_show_query =
    \\SELECT id, show_id, name, season, episode, airdate FROM episodes
    \\WHERE show_id = ?1
;

pub fn getEpisodesForShow(
    self: *Db,
    gpa: std.mem.Allocator,
    show_id: types.ShowId,
) ![]const types.Episode {
    const stmt = try self.prepare(get_episodes_for_show_query, &.{
        .{ .i64 = show_id.inner },
    });
    defer _ = c.sqlite3_finalize(stmt);

    const num_playthroughs = try self.numPlaythroughsForShow(show_id);

    var ret: std.ArrayList(types.Episode) = .empty;
    while (true) {
        switch (c.sqlite3_step(stmt)) {
            c.SQLITE_ROW => {
                const episode = try self.episodeFromRowIndices(
                    gpa,
                    stmt,
                    .{
                        .id = 0,
                        .show_id = 1,
                        .name = 2,
                        .season = 3,
                        .episode = 4,
                        .airdate = 5,
                    },
                );

                try ret.append(gpa, try self.appendEpisodeWatchStatus(gpa, episode, num_playthroughs));
            },
            c.SQLITE_DONE => break,
            else => return error.Sqlite,
        }
    }

    return ret.items;
}

const get_episode_query =
    \\SELECT id, show_id, name, season, episode, airdate FROM episodes WHERE id = ?1
;

pub fn getEpisode(self: *Db, gpa: std.mem.Allocator, id: types.EpisodeId) !?types.Episode {
    const stmt = try self.prepare(get_episode_query, &.{
        .{ .i64 = id.inner },
    });
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => {
            const episode = try self.episodeFromRowIndices(
                gpa,
                stmt,
                .{
                    .id = 0,
                    .show_id = 1,
                    .name = 2,
                    .season = 3,
                    .episode = 4,
                    .airdate = 5,
                },
            );

            const num_playthroughs = try self.numPlaythroughsForShow(episode.show_id);
            return try self.appendEpisodeWatchStatus(gpa, episode, num_playthroughs);
        },
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

pub fn setEpisodeWatchStatus(
    self: *Db,
    episode: types.EpisodeId,
    statuses: []const types.WatchStatus,
) !void {
    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};

    for (statuses, 0..) |status, i| {
        const playthrough_id: i64 = @intCast(i);

        try self.exec(
            \\DELETE FROM episode_watch_status
            \\    WHERE episode_id = ?1 AND playthrough_id = ?2;
            \\DELETE FROM skipped_episodes
            \\    WHERE episode_id = ?1 AND playthrough_id = ?2;
        , &.{
            .{ .i64 = episode.inner },
            .{ .i64 = playthrough_id },
        });

        switch (status) {
            .watched => |date| {
                try self.exec(
                    \\INSERT OR IGNORE INTO episode_watch_status(episode_id, watch_date, playthrough_id)
                    \\VALUES (?1, ?2, ?3)
                , &.{
                    .{ .i64 = episode.inner },
                    .{ .i64 = date.inner.toCeDay() },
                    .{ .i64 = playthrough_id },
                });
            },
            .skipped => {
                try self.exec(
                    \\INSERT OR IGNORE INTO skipped_episodes(episode_id, playthrough_id)
                    \\VALUES (?1, ?2)
                , &.{
                    .{ .i64 = episode.inner },
                    .{ .i64 = playthrough_id },
                });
            },
            .unwatched => {},
        }
    }

    try self.exec("COMMIT", &.{});
}

const get_ratings_query =
    \\SELECT id, name, priority FROM ratings
;

pub fn getRatings(self: *Db, gpa: std.mem.Allocator) ![]const Rating {
    const stmt = try self.prepare(get_ratings_query, &.{});
    defer _ = c.sqlite3_finalize(stmt);

    var ret: std.ArrayList(Rating) = .empty;
    while (true) {
        switch (c.sqlite3_step(stmt)) {
            c.SQLITE_ROW => {
                const id = try self.rowi64(stmt, 0) orelse return error.InvalidData;
                const name = try self.rowText(gpa, stmt, 1) orelse return error.InvalidData;
                const priority = try self.rowi64(stmt, 2) orelse return error.InvalidData;

                try ret.append(gpa, .{
                    .id = .{ .inner = id },
                    .name = name,
                    .priority = priority,
                });
            },
            c.SQLITE_DONE => break,
            else => return error.Sqlite,
        }
    }

    return ret.items;
}

pub fn getRating(self: *Db, gpa: std.mem.Allocator, id: types.RatingId) !?Rating {
    const stmt = try self.prepare("SELECT id, name, priority FROM ratings WHERE id = ?1", &.{
        .{ .i64 = id.inner },
    });
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => {
            const row_id = try self.rowi64(stmt, 0) orelse return error.InvalidData;
            const name = try self.rowText(gpa, stmt, 1) orelse return error.InvalidData;
            const priority = try self.rowi64(stmt, 2) orelse return error.InvalidData;
            return .{
                .id = .{ .inner = row_id },
                .name = name,
                .priority = priority,
            };
        },
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

pub fn addRating(self: *Db, name: []const u8) !types.RatingId {
    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};

    const priority = blk: {
        const stmt = try self.prepare("SELECT MAX(priority) FROM ratings", &.{});
        defer _ = c.sqlite3_finalize(stmt);

        switch (c.sqlite3_step(stmt)) {
            c.SQLITE_ROW => break :blk (try self.rowi64(stmt, 0)) orelse 0,
            c.SQLITE_DONE => break :blk 0,
            else => return error.Sqlite,
        }
    };

    try self.exec(
        \\INSERT INTO ratings(name, priority)
        \\VALUES (?1, ?2)
    , &.{
        .{ .text = name },
        .{ .i64 = priority + 1 },
    });

    const last_row_id = c.sqlite3_last_insert_rowid(self.sqlite);

    try self.exec("COMMIT", &.{});

    return .{ .inner = last_row_id };
}

pub fn updateRating(self: *Db, rating: Rating) !void {
    try self.exec(
        \\UPDATE ratings SET name = ?2, priority = ?3
        \\WHERE id = ?1
    , &.{
        .{ .i64 = rating.id.inner },
        .{ .text = rating.name },
        .{ .i64 = rating.priority },
    });
}

pub fn deleteRating(self: *Db, id: types.RatingId) !void {
    const id_param: []const Binding = &.{.{ .i64 = id.inner }};

    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};
    try self.exec(
        \\DELETE FROM show_ratings WHERE rating_id = ?1;
        \\DELETE FROM ratings WHERE id = ?1;
    , id_param);
    try self.exec("COMMIT", &.{});
}

const get_image_url_query =
    \\SELECT url FROM images WHERE id = ?1
;

pub fn getImageUrl(self: *Db, gpa: std.mem.Allocator, id: types.ImageId) !?[]const u8 {
    const stmt = try self.prepare(get_image_url_query, &.{
        .{ .i64 = id.inner },
    });
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => return try self.rowText(gpa, stmt, 0) orelse return error.InvalidData,
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

pub fn addMovie(self: *Db, movie: types.RemoteMovie, now: std.Io.Timestamp) !types.MovieId {
    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};

    const theater: Binding = if (movie.theater_release_date) |d| .{ .i64 = d.inner.toCeDay() } else .null;
    const home: Binding = if (movie.home_release_date) |d| .{ .i64 = d.inner.toCeDay() } else .null;

    const id: types.MovieId = if (try self.findRemoteMovie(movie.imdb_id)) |existing| blk: {
        try self.exec(
            \\UPDATE images
            \\SET url = ?2
            \\WHERE id = (SELECT image FROM movies WHERE id = ?1)
        , &.{
            .{ .i64 = existing.inner },
            .{ .text = movie.image },
        });
        try self.exec(
            \\UPDATE movies
            \\SET theater_release_date = ?2, home_release_date = ?3, last_update_time = ?4
            \\WHERE id = ?1
        , &.{
            .{ .i64 = existing.inner },
            theater,
            home,
            .{ .i64 = now.toSeconds() },
        });
        break :blk existing;
    } else blk: {
        try self.exec("INSERT INTO images(url) VALUES (?1)", &.{.{ .text = movie.image }});
        const image_id = c.sqlite3_last_insert_rowid(self.sqlite);

        try self.exec(
            \\INSERT INTO movies(imdb_id, name, year, image, theater_release_date, home_release_date, last_update_time)
            \\VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        , &.{
            .{ .text = movie.imdb_id },
            .{ .text = movie.name },
            .{ .i64 = movie.year },
            .{ .i64 = image_id },
            theater,
            home,
            .{ .i64 = now.toSeconds() },
        });
        break :blk .{ .inner = c.sqlite3_last_insert_rowid(self.sqlite) };
    };

    try self.exec("COMMIT", &.{});

    return id;
}

fn findRemoteMovie(self: *Db, imdb_id: []const u8) !?types.MovieId {
    const stmt = try self.prepare("SELECT id FROM movies WHERE imdb_id = ?1", &.{.{ .text = imdb_id }});
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => return .{ .inner = (try self.rowi64(stmt, 0)) orelse return error.InvalidData },
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

const get_movies_query =
    \\SELECT id, imdb_id, name, year, image, theater_release_date, home_release_date, movie_watch_status.watch_date, movie_ratings.rating_id, last_update_time, movies.notes
    \\FROM movies
    \\LEFT JOIN movie_watch_status ON movies.id = movie_watch_status.movie_id
    \\LEFT JOIN movie_ratings ON movies.id = movie_ratings.movie_id
;

pub fn getMovies(self: *Db, gpa: std.mem.Allocator) ![]types.Movie {
    const stmt = try self.prepare(get_movies_query, &.{});
    defer _ = c.sqlite3_finalize(stmt);

    var ret: std.ArrayList(types.Movie) = .empty;
    while (true) {
        switch (c.sqlite3_step(stmt)) {
            c.SQLITE_ROW => try ret.append(gpa, try self.movieFromRow(gpa, stmt)),
            c.SQLITE_DONE => break,
            else => return error.Sqlite,
        }
    }

    return ret.items;
}

const get_movie_query = get_movies_query ++
    \\
    \\WHERE id = ?1
;

pub fn getMovie(self: *Db, gpa: std.mem.Allocator, id: types.MovieId) !?types.Movie {
    const stmt = try self.prepare(get_movie_query, &.{
        .{ .i64 = id.inner },
    });
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => return try self.movieFromRow(gpa, stmt),
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

fn movieFromRow(self: *Db, alloc: std.mem.Allocator, stmt: *c.sqlite3_stmt) !types.Movie {
    const id = try self.rowi64(stmt, 0) orelse return error.InvalidData;
    const imdb_id = try self.rowText(alloc, stmt, 1) orelse return error.InvalidData;
    const name = try self.rowText(alloc, stmt, 2) orelse return error.InvalidData;
    const year = try self.rowi32(stmt, 3) orelse return error.InvalidData;
    const image = try self.rowi64(stmt, 4) orelse return error.InvalidData;
    const theater_release_date = try self.rowi64(stmt, 5);
    const home_release_date = try self.rowi64(stmt, 6);
    const watch_date = try self.rowi64(stmt, 7);
    const rating_id = try self.rowi64(stmt, 8);
    const last_update_timestamp: ?std.Io.Timestamp = if (try self.rowi64(stmt, 9)) |v| .fromNanoseconds(v * std.time.ns_per_s) else null;
    const notes = try self.rowText(alloc, stmt, 10);

    return .{
        .id = .{ .inner = id },
        .imdb_id = imdb_id,
        .name = name,
        .image = .{ .inner = image },
        .year = year,
        .watched = watch_date != null,
        .rating_id = if (rating_id) |v| .{ .inner = v } else null,
        .theater_release_date = if (theater_release_date) |v| .{ .inner = sphtud.datetime.Date.fromCeDay(v) } else null,
        .home_release_date = if (home_release_date) |v| .{ .inner = sphtud.datetime.Date.fromCeDay(v) } else null,
        .last_update_time = last_update_timestamp,
        .notes = notes,
    };
}

pub fn setMovieRating(self: *Db, movie_id: types.MovieId, rating_id: ?types.RatingId) !void {
    if (rating_id) |rid| {
        try self.exec(
            \\INSERT INTO movie_ratings(movie_id, rating_id)
            \\VALUES (?1, ?2)
            \\ON CONFLICT(movie_id) DO UPDATE SET rating_id = ?2
        , &.{
            .{ .i64 = movie_id.inner },
            .{ .i64 = rid.inner },
        });
    } else {
        try self.exec("DELETE FROM movie_ratings WHERE movie_id = ?1", &.{
            .{ .i64 = movie_id.inner },
        });
    }
}

pub fn setMovieNotes(self: *Db, movie_id: types.MovieId, notes: []const u8) !void {
    try self.exec(
        \\UPDATE movies SET notes = ?2 WHERE id = ?1
    , &.{
        .{ .i64 = movie_id.inner },
        .{ .text = notes },
    });
}

pub fn setMovieWatchStatus(self: *Db, id: types.MovieId, watched: ?sphtud.datetime.Date) !void {
    if (watched) |date| {
        try self.exec(
            \\INSERT OR IGNORE INTO movie_watch_status(movie_id, watch_date)
            \\VALUES (?1, ?2)
        , &.{
            .{ .i64 = id.inner },
            .{ .i64 = date.toCeDay() },
        });
    } else {
        try self.exec("DELETE FROM movie_watch_status WHERE movie_id = ?1", &.{
            .{ .i64 = id.inner },
        });
    }
}

pub fn deleteMovie(self: *Db, id: types.MovieId) !void {
    const id_param: []const Binding = &.{.{ .i64 = id.inner }};

    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};

    const image_id = try self.movieImageId(id);

    try self.exec(
        \\DELETE FROM movie_ratings WHERE movie_id = ?1;
        \\DELETE FROM movie_watch_status WHERE movie_id = ?1;
        \\DELETE FROM movies WHERE id = ?1;
    , id_param);

    if (image_id) |img| {
        try self.exec("DELETE FROM images WHERE id = ?1", &.{.{ .i64 = img }});
    }

    try self.exec("COMMIT", &.{});
}

fn movieImageId(self: *Db, id: types.MovieId) !?i64 {
    const stmt = try self.prepare("SELECT image FROM movies WHERE id = ?1", &.{.{ .i64 = id.inner }});
    defer _ = c.sqlite3_finalize(stmt);
    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => return try self.rowi64(stmt, 0),
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

pub fn setPauseStatus(self: *Db, show_id: types.ShowId, paused: bool) !void {
    const query = if (paused)
        "INSERT OR IGNORE INTO paused_shows(show_id) VALUES (?1)"
    else
        "DELETE FROM paused_shows WHERE show_id = ?1";

    try self.exec(query, &.{
        .{ .i64 = show_id.inner },
    });
}

pub fn setShowRating(self: *Db, show_id: types.ShowId, rating_id: ?types.RatingId) !void {
    if (rating_id) |rid| {
        try self.exec(
            \\INSERT INTO show_ratings(show_id, rating_id)
            \\VALUES (?1, ?2)
            \\ON CONFLICT(show_id) DO UPDATE SET rating_id = ?2
        , &.{
            .{ .i64 = show_id.inner },
            .{ .i64 = rid.inner },
        });
    } else {
        try self.exec("DELETE FROM show_ratings WHERE show_id = ?1", &.{
            .{ .i64 = show_id.inner },
        });
    }
}

pub fn setShowNotes(self: *Db, show_id: types.ShowId, notes: []const u8) !void {
    try self.exec(
        \\UPDATE shows SET notes = ?2 WHERE id = ?1
    , &.{
        .{ .i64 = show_id.inner },
        .{ .text = notes },
    });
}

pub fn addShow(self: *Db, show: types.RemoteTvShow(types.TvMazeShowId)) !types.ShowId {
    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};

    const image_id: Binding = if (show.image) |url| blk: {
        try self.exec("INSERT INTO images(url) VALUES (?1)", &.{.{ .text = url }});
        break :blk .{ .i64 = c.sqlite3_last_insert_rowid(self.sqlite) };
    } else .null;

    try self.exec(
        \\INSERT INTO shows(name, tvmaze_id, year, imdb_id, tvdb_id, image_id, tvmaze_url)
        \\VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    , &.{
        .{ .text = show.name },
        .{ .i64 = show.id.inner },
        if (show.year) |y| .{ .i64 = y } else .null,
        if (show.imdb_id) |v| .{ .text = v.inner } else .null,
        if (show.tvdb_id) |v| .{ .i64 = v.inner } else .null,
        image_id,
        if (show.url) |u| .{ .text = u } else .null,
    });

    const last_show_id = c.sqlite3_last_insert_rowid(self.sqlite);

    try self.exec("COMMIT", &.{});

    return .{ .inner = last_show_id };
}

pub fn addEpisode(self: *Db, show_id: types.ShowId, episode: types.RemoteEpisode) !types.EpisodeId {
    const airdate: Binding = if (episode.airdate) |d| .{ .i64 = d.inner.toCeDay() } else .null;

    const existing = try self.findEpisode(show_id, episode);
    if (existing) |episode_id| {
        try self.exec(
            \\UPDATE episodes
            \\SET show_id = ?2, name = ?3, season = ?4, episode = ?5, airdate = ?6
            \\WHERE id = ?1
        , &.{
            .{ .i64 = episode_id.inner },
            .{ .i64 = show_id.inner },
            .{ .text = episode.name },
            .{ .i64 = episode.season },
            .{ .i64 = episode.episode },
            airdate,
        });
        return episode_id;
    }

    try self.exec(
        \\INSERT INTO episodes(show_id, name, season, episode, airdate)
        \\VALUES (?1, ?2, ?3, ?4, ?5)
    , &.{
        .{ .i64 = show_id.inner },
        .{ .text = episode.name },
        .{ .i64 = episode.season },
        .{ .i64 = episode.episode },
        airdate,
    });

    return .{ .inner = c.sqlite3_last_insert_rowid(self.sqlite) };
}

fn findEpisode(self: *Db, show_id: types.ShowId, episode: types.RemoteEpisode) !?types.EpisodeId {
    const stmt = try self.prepare(
        "SELECT id FROM episodes WHERE show_id = ?1 AND season = ?2 AND episode = ?3",
        &.{
            .{ .i64 = show_id.inner },
            .{ .i64 = episode.season },
            .{ .i64 = episode.episode },
        },
    );
    defer _ = c.sqlite3_finalize(stmt);

    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => return .{ .inner = (try self.rowi64(stmt, 0)) orelse return error.InvalidData },
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

pub fn removeShow(self: *Db, id: types.ShowId) !void {
    try self.exec("BEGIN", &.{});
    errdefer self.exec("ROLLBACK", &.{}) catch {};

    const id_param: []const Binding = &.{.{ .i64 = id.inner }};

    const image_id = try self.showImageId(id);

    try self.exec(
        \\DELETE FROM paused_shows WHERE show_id = ?1;
        \\DELETE FROM show_ratings WHERE show_id = ?1;
        \\DELETE FROM episode_watch_status WHERE episode_id IN (
        \\    SELECT id FROM episodes WHERE show_id = ?1
        \\);
        \\DELETE FROM skipped_episodes WHERE episode_id IN (
        \\    SELECT id FROM episodes WHERE show_id = ?1
        \\);
        \\DELETE FROM episodes WHERE show_id = ?1;
        \\DELETE FROM shows WHERE id = ?1;
    , id_param);

    if (image_id) |img| {
        try self.exec("DELETE FROM images WHERE id = ?1", &.{.{ .i64 = img }});
    }

    try self.exec("COMMIT", &.{});
}

fn showImageId(self: *Db, id: types.ShowId) !?i64 {
    const stmt = try self.prepare("SELECT image_id FROM shows WHERE id = ?1", &.{.{ .i64 = id.inner }});
    defer _ = c.sqlite3_finalize(stmt);
    switch (c.sqlite3_step(stmt)) {
        c.SQLITE_ROW => return try self.rowi64(stmt, 0),
        c.SQLITE_DONE => return null,
        else => return error.Sqlite,
    }
}

fn exec(self: *Db, sql: [:0]const u8, args: []const Binding) !void {
    if (args.len == 0) {
        return try retCheck(c.sqlite3_exec(self.sqlite, sql.ptr, null, null, null));
    }

    var remaining: [*c]const u8 = sql.ptr;
    while (remaining[0] != 0) {
        var stmt: ?*c.sqlite3_stmt = null;
        var tail: [*c]const u8 = null;
        try retCheck(c.sqlite3_prepare_v2(self.sqlite, remaining, -1, &stmt, &tail));
        remaining = tail;

        // Whitespace or comments between statements prepare to a null stmt.
        const s = stmt orelse continue;
        defer _ = c.sqlite3_finalize(s);

        const param_count: usize = @intCast(c.sqlite3_bind_parameter_count(s));
        if (param_count > args.len) return error.Sqlite;
        try bindArgs(s, args[0..param_count]);

        switch (c.sqlite3_step(s)) {
            c.SQLITE_DONE => {},
            else => return error.Sqlite,
        }
    }
}

fn bindArgs(stmt: *c.sqlite3_stmt, args: []const Binding) !void {
    for (args, 1..) |arg, idx| switch (arg) {
        .i64 => |val| try retCheck(c.sqlite3_bind_int64(stmt, @intCast(idx), val)),
        .text => |text| try retCheck(c.sqlite3_bind_text(stmt, @intCast(idx), text.ptr, @intCast(text.len), null)),
        .null => try retCheck(c.sqlite3_bind_null(stmt, @intCast(idx))),
    };
}

fn retCheck(val: c_int) !void {
    if (val == c.SQLITE_OK) return;
    return error.Sqlite;
}

const Binding = union(enum) {
    i64: i64,
    text: []const u8,
    null,
};

fn prepare(self: *Db, query: []const u8, args: []const Binding) !*c.sqlite3_stmt {
    var ret: ?*c.sqlite3_stmt = null;
    try retCheck(c.sqlite3_prepare_v2(
        self.sqlite,
        query.ptr,
        @intCast(query.len),
        &ret,
        null,
    ));

    const stmt = ret orelse return error.Sqlite;
    try bindArgs(stmt, args);
    return stmt;
}

pub fn toCeDay(now: std.Io.Timestamp) i64 {
    return sphtud.datetime.epochToCeDay(now.toSeconds());
}

const ShowIndices = struct {
    id: c_int,
    remote_id: c_int,
    name: c_int,
    image: c_int,
    year: c_int,
    url: c_int,
    imdb_id: c_int,
    tvdb_id: c_int,
    num_episodes: c_int,
    pause_status: c_int,
    rating_id: c_int,
    notes: c_int,
};

const ShowWithoutWatchStatus = struct {
    id: types.ShowId,
    remote_id: types.TvMazeShowId,
    name: []const u8,
    image: ?types.ImageId,
    year: ?i32,
    url: ?[]const u8,
    imdb_id: ?types.ImdbShowId,
    tvdb_id: ?types.TvdbShowId,
    pause_status: bool,
    episodes_aired: i64,
    rating_id: ?types.RatingId,
    notes: ?[]const u8,
};

fn rowi64(self: *Db, stmt: *c.sqlite3_stmt, idx: i32) !?i64 {
    _ = self;
    if (c.sqlite3_column_type(stmt, idx) == c.SQLITE_NULL) return null;
    return c.sqlite3_column_int64(stmt, idx);
}

fn rowi32(self: *Db, stmt: *c.sqlite3_stmt, idx: i32) !?i32 {
    _ = self;
    if (c.sqlite3_column_type(stmt, idx) == c.SQLITE_NULL) return null;
    return c.sqlite3_column_int(stmt, idx);
}

fn rowText(self: *Db, alloc: std.mem.Allocator, stmt: *c.sqlite3_stmt, idx: i32) !?[]const u8 {
    _ = self;
    if (c.sqlite3_column_type(stmt, idx) == c.SQLITE_NULL) return null;
    const ret = c.sqlite3_column_text(stmt, idx);
    return try alloc.dupe(u8, std.mem.span(ret));
}

fn showFromRowIndices(self: *Db, alloc: std.mem.Allocator, stmt: *c.sqlite3_stmt, indices: ShowIndices) !ShowWithoutWatchStatus {
    const id = try self.rowi64(stmt, indices.id) orelse return error.InvalidData;
    const remote_id = try self.rowi64(stmt, indices.remote_id) orelse return error.InvalidData;
    const rating_id = try self.rowi64(stmt, indices.rating_id);
    const name = try self.rowText(alloc, stmt, indices.name) orelse return error.InvalidData;
    const year = try self.rowi32(stmt, indices.year);
    const imdb_id = try self.rowText(alloc, stmt, indices.imdb_id);
    const tvdb_id = try self.rowi64(stmt, indices.tvdb_id);
    const image = try self.rowi64(stmt, indices.image);
    const url = try self.rowText(alloc, stmt, indices.url);
    const episodes_aired = try self.rowi64(stmt, indices.num_episodes) orelse 0;
    const pause_status = (try self.rowi64(stmt, indices.pause_status)) != null;
    const notes = try self.rowText(alloc, stmt, indices.notes);

    return .{
        .id = .{ .inner = id },
        .remote_id = .{ .inner = remote_id },
        .name = name,
        .year = year,
        .imdb_id = if (imdb_id) |v| .{ .inner = v } else null,
        .tvdb_id = if (tvdb_id) |v| .{ .inner = v } else null,
        .image = if (image) |v| .{ .inner = v } else null,
        .url = url,
        .pause_status = pause_status,
        .episodes_aired = episodes_aired,
        .rating_id = if (rating_id) |v| .{ .inner = v } else null,
        .notes = notes,
    };
}

fn makeWatchStatusStatementText(comptime table: []const u8) []const u8 {
    return "SELECT playthrough_id, COUNT(*)  FROM " ++ table ++ " WHERE episode_id IN (SELECT id FROM episodes WHERE show_id = ?1) GROUP BY playthrough_id";
}

fn appendShowWatchStatus(
    self: *Db,
    gpa: std.mem.Allocator,
    show: ShowWithoutWatchStatus,
) !Show {
    var num_watched = std.ArrayList(i64).empty;
    var num_skipped = std.ArrayList(i64).empty;

    const mapping: []const struct { []const u8, *std.ArrayList(i64) } = &.{
        .{ makeWatchStatusStatementText("episode_watch_status"), &num_watched },
        .{ makeWatchStatusStatementText("skipped_episodes"), &num_skipped },
    };
    for (mapping) |item| {
        const stmt = try self.prepare(item[0], &.{.{ .i64 = show.id.inner }});
        defer _ = c.sqlite3_finalize(stmt);

        while (true) {
            switch (c.sqlite3_step(stmt)) {
                c.SQLITE_ROW => {
                    const playthrough_id_i = try self.rowi64(stmt, 0) orelse return error.InvalidData;
                    const playthrough_id = std.math.cast(usize, playthrough_id_i) orelse return error.InvalidData;
                    const count = try self.rowi64(stmt, 1) orelse return error.InvalidData;

                    while (item[1].items.len <= playthrough_id) {
                        try item[1].append(gpa, 0);
                    }
                    item[1].items[playthrough_id] = count;
                },
                c.SQLITE_DONE => break,
                else => return error.Sqlite,
            }
        }
    }

    // Make sure all playthroughs are accounted for in both arrays
    while (num_watched.items.len < num_skipped.items.len) {
        try num_watched.append(gpa, 0);
    }

    while (num_skipped.items.len < num_watched.items.len) {
        try num_skipped.append(gpa, 0);
    }

    return .{
        .id = show.id,
        .remote_id = show.remote_id,
        .name = show.name,
        .image = show.image,
        .year = show.year,
        .url = show.url,
        .imdb_id = show.imdb_id,
        .tvdb_id = show.tvdb_id,
        .pause_status = show.pause_status,
        .episodes_watched = num_watched.items,
        .episodes_skipped = num_skipped.items,
        .episodes_aired = show.episodes_aired,
        .rating_id = show.rating_id,
        .notes = show.notes,
    };
}

const EpisodeIndices = struct {
    id: c_int,
    show_id: c_int,
    name: c_int,
    season: c_int,
    episode: c_int,
    airdate: c_int,
};

const EpisodeWithoutWatchStatus = struct {
    id: types.EpisodeId,
    show_id: types.ShowId,
    name: []const u8,
    season: i64,
    episode: i64,
    airdate: ?types.Date,
};

fn episodeFromRowIndices(self: *Db, alloc: std.mem.Allocator, stmt: *c.sqlite3_stmt, indices: EpisodeIndices) !EpisodeWithoutWatchStatus {
    const id = try self.rowi64(stmt, indices.id) orelse return error.InvalidData;
    const show_id = try self.rowi64(stmt, indices.show_id) orelse return error.InvalidData;
    const name = try self.rowText(alloc, stmt, indices.name) orelse return error.InvalidData;
    const season = try self.rowi64(stmt, indices.season) orelse return error.InvalidData;
    const episode = try self.rowi64(stmt, indices.episode) orelse return error.InvalidData;
    const airdate = try self.rowi64(stmt, indices.airdate);

    return .{
        .id = .{ .inner = id },
        .show_id = .{ .inner = show_id },
        .name = name,
        .season = season,
        .episode = episode,
        .airdate = if (airdate) |v| .{ .inner = sphtud.datetime.Date.fromCeDay(v) } else null,
    };
}

fn makeMaxPlaythroughStatementText(comptime table: []const u8) []const u8 {
    return "SELECT MAX(playthrough_id) FROM " ++ table ++ " WHERE episode_id IN (SELECT id FROM episodes WHERE show_id = ?1)";
}

fn numPlaythroughsForShow(self: *Db, show_id: types.ShowId) !usize {
    var max_playthrough_id: i64 = 0;

    const queries: []const []const u8 = &.{
        makeMaxPlaythroughStatementText("episode_watch_status"),
        makeMaxPlaythroughStatementText("skipped_episodes"),
    };

    for (queries) |query| {
        const stmt = try self.prepare(query, &.{.{ .i64 = show_id.inner }});
        defer _ = c.sqlite3_finalize(stmt);

        switch (c.sqlite3_step(stmt)) {
            c.SQLITE_ROW => {
                const this_max = try self.rowi64(stmt, 0) orelse continue;
                max_playthrough_id = @max(max_playthrough_id, this_max);
            },
            c.SQLITE_DONE => continue,
            else => return error.Sqlite,
        }
    }

    return @intCast(max_playthrough_id + 1);
}

fn appendEpisodeWatchStatus(
    self: *Db,
    gpa: std.mem.Allocator,
    episode: EpisodeWithoutWatchStatus,
    num_playthroughs: usize,
) !types.Episode {
    const watch_statuses = try gpa.alloc(types.WatchStatus, num_playthroughs);
    for (watch_statuses) |*ws| ws.* = .unwatched;

    {
        const stmt = try self.prepare("SELECT watch_date, playthrough_id FROM episode_watch_status WHERE episode_id = ?1", &.{.{ .i64 = episode.id.inner }});
        defer _ = c.sqlite3_finalize(stmt);

        while (true) {
            switch (c.sqlite3_step(stmt)) {
                c.SQLITE_ROW => {
                    const watch_date = try self.rowi64(stmt, 0) orelse return error.InvalidData;
                    const playthrough_id_i = try self.rowi64(stmt, 1) orelse return error.InvalidData;
                    const playthrough_id = std.math.cast(usize, playthrough_id_i) orelse return error.InvalidData;
                    if (playthrough_id >= watch_statuses.len) return error.InvalidData;
                    watch_statuses[playthrough_id] = .{ .watched = .{ .inner = sphtud.datetime.Date.fromCeDay(watch_date) } };
                },
                c.SQLITE_DONE => break,
                else => return error.Sqlite,
            }
        }
    }

    {
        const stmt = try self.prepare("SELECT playthrough_id FROM skipped_episodes WHERE episode_id = ?1", &.{.{ .i64 = episode.id.inner }});
        defer _ = c.sqlite3_finalize(stmt);

        while (true) {
            switch (c.sqlite3_step(stmt)) {
                c.SQLITE_ROW => {
                    const playthrough_id_i = try self.rowi64(stmt, 0) orelse return error.InvalidData;
                    const playthrough_id = std.math.cast(usize, playthrough_id_i) orelse return error.InvalidData;
                    if (playthrough_id >= watch_statuses.len) return error.InvalidData;
                    if (std.meta.activeTag(watch_statuses[playthrough_id]) != .unwatched) {
                        std.log.warn("Episode {d} has both skipped and watched set, choosing watched", .{episode.id.inner});
                        continue;
                    }
                    watch_statuses[playthrough_id] = .skipped;
                },
                c.SQLITE_DONE => break,
                else => return error.Sqlite,
            }
        }
    }

    return .{
        .id = episode.id,
        .show_id = episode.show_id,
        .name = episode.name,
        .season = episode.season,
        .episode = episode.episode,
        .airdate = episode.airdate,
        .watch_status = watch_statuses,
    };
}

fn errCheck(self: *Db) !void {
    try retCheck(c.sqlite3_errcode(self.sqlite));
}

const testing = std.testing;

fn genDate(num_days: i64) types.Date {
    return .{ .inner = sphtud.datetime.Date.fromCeDay(num_days) };
}

fn genTimestamp(num_days: i64) std.Io.Timestamp {
    const seconds = (num_days - sphtud.datetime.epoch_days_from_ce) * std.time.s_per_day;
    return .{ .nanoseconds = @as(i96, seconds) * std.time.ns_per_s };
}

fn generateEmptyShow(name: []const u8, id: i64) types.RemoteTvShow(types.TvMazeShowId) {
    return .{
        .id = .{ .inner = id },
        .name = name,
        .image = null,
        .year = null,
        .url = null,
        .imdb_id = null,
        .tvdb_id = null,
    };
}

fn remoteFromShow(gpa: std.mem.Allocator, db: *Db, show: Show) !types.RemoteTvShow(types.TvMazeShowId) {
    const image_url: ?[]const u8 = if (show.image) |image_id|
        (try db.getImageUrl(gpa, image_id)) orelse return error.MissingImage
    else
        null;

    return .{
        .id = show.remote_id,
        .name = show.name,
        .image = image_url,
        .year = show.year,
        .url = show.url,
        .imdb_id = show.imdb_id,
        .tvdb_id = show.tvdb_id,
    };
}

fn remoteFromMovie(gpa: std.mem.Allocator, db: *Db, movie: types.Movie) !types.RemoteMovie {
    const image_url = (try db.getImageUrl(gpa, movie.image)) orelse return error.MissingImage;
    return .{
        .imdb_id = movie.imdb_id,
        .name = movie.name,
        .year = movie.year,
        .image = image_url,
        .theater_release_date = movie.theater_release_date,
        .home_release_date = movie.home_release_date,
    };
}

fn remoteFromEpisode(ep: types.Episode) types.RemoteEpisode {
    return .{
        .name = ep.name,
        .season = ep.season,
        .episode = ep.episode,
        .airdate = ep.airdate,
    };
}

fn findShowById(id: types.ShowId, shows: Shows) ?Show {
    for (shows) |s| if (s.id.inner == id.inner) return s;
    return null;
}

fn findEpisodeById(id: types.EpisodeId, episodes: []const types.Episode) ?types.Episode {
    for (episodes) |e| if (e.id.inner == id.inner) return e;
    return null;
}

fn findRatingById(id: types.RatingId, ratings: []const Rating) ?Rating {
    for (ratings) |r| if (r.id.inner == id.inner) return r;
    return null;
}

const TestDb = struct {
    db: Db,
    arena: std.heap.ArenaAllocator,

    fn init() !TestDb {
        return .{
            .db = try Db.init(":memory:"),
            .arena = std.heap.ArenaAllocator.init(testing.allocator),
        };
    }

    fn deinit(self: *TestDb) void {
        _ = c.sqlite3_close(self.db.sqlite);
        self.arena.deinit();
    }

    fn gpa(self: *TestDb) std.mem.Allocator {
        return self.arena.allocator();
    }
};

test "full show in out" {
    var t = try TestDb.init();
    defer t.deinit();

    const show: types.RemoteTvShow(types.TvMazeShowId) = .{
        .id = .{ .inner = 0 },
        .name = "Test Show",
        .image = "test_url",
        .year = 1234,
        .url = "tvmaze_url",
        .imdb_id = .{ .inner = "imdbid" },
        .tvdb_id = .{ .inner = 12 },
    };

    const show_id = try t.db.addShow(show);

    const inserted_show = (try t.db.getShow(t.gpa(), show_id, genTimestamp(1234))).?;
    try testing.expectEqualDeep(show, try remoteFromShow(t.gpa(), &t.db, inserted_show));

    const retrieved_shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
    try testing.expectEqual(1, retrieved_shows.len);
    try testing.expectEqualDeep(inserted_show, findShowById(show_id, retrieved_shows).?);
}

test "empty show in out" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);

    const show_id = try t.db.addShow(show);

    const inserted_show = (try t.db.getShow(t.gpa(), show_id, genTimestamp(1234))).?;
    try testing.expectEqualDeep(show, try remoteFromShow(t.gpa(), &t.db, inserted_show));

    const retrieved_shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
    try testing.expectEqual(1, retrieved_shows.len);
    try testing.expectEqualDeep(inserted_show, findShowById(show_id, retrieved_shows).?);
}

test "get shows" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);
    const show2 = generateEmptyShow("Test show 2", 1);

    const show_id1 = try t.db.addShow(show);
    const show_id2 = try t.db.addShow(show2);

    const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));

    try testing.expectEqual(2, shows.len);
    try testing.expectEqual(types.TvMazeShowId{ .inner = 0 }, findShowById(show_id1, shows).?.remote_id);
    try testing.expectEqualDeep(show, try remoteFromShow(t.gpa(), &t.db, findShowById(show_id1, shows).?));
    try testing.expectEqual(types.TvMazeShowId{ .inner = 1 }, findShowById(show_id2, shows).?.remote_id);
    try testing.expectEqualDeep(show2, try remoteFromShow(t.gpa(), &t.db, findShowById(show_id2, shows).?));
}

test "episode in out" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);

    const episode: types.RemoteEpisode = .{
        .name = "Test Episode",
        .season = 1,
        .episode = 34,
        .airdate = genDate(1023),
    };

    const show_id = try t.db.addShow(show);

    const id = try t.db.addEpisode(show_id, episode);

    const retrieved_episodes = try t.db.getEpisodesForShow(t.gpa(), show_id);

    try testing.expectEqual(1, retrieved_episodes.len);
    try testing.expectEqualDeep(episode, remoteFromEpisode(findEpisodeById(id, retrieved_episodes).?));
    try testing.expectEqual(show_id.inner, findEpisodeById(id, retrieved_episodes).?.show_id.inner);
}

test "update episode" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);

    const episode: types.RemoteEpisode = .{
        .name = "Test Episode",
        .season = 1,
        .episode = 34,
        .airdate = genDate(1023),
    };

    const episode_update: types.RemoteEpisode = .{
        .name = "Test Episode updated",
        .season = 1,
        .episode = 34,
        .airdate = genDate(1024),
    };

    const show_id = try t.db.addShow(show);

    const id = try t.db.addEpisode(show_id, episode);
    _ = try t.db.addEpisode(show_id, episode_update);

    const retrieved_episodes = try t.db.getEpisodesForShow(t.gpa(), show_id);

    try testing.expectEqual(1, retrieved_episodes.len);
    try testing.expectEqualDeep(episode_update, remoteFromEpisode(findEpisodeById(id, retrieved_episodes).?));
}

test "set watch status" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);

    const episode: types.RemoteEpisode = .{
        .name = "Test Episode",
        .season = 1,
        .episode = 34,
        .airdate = genDate(1023),
    };

    const episode2: types.RemoteEpisode = .{
        .name = "Test Episode 2",
        .season = 1,
        .episode = 35,
        .airdate = genDate(1023),
    };

    const show_id = try t.db.addShow(show);

    const episode_id = try t.db.addEpisode(show_id, episode);
    _ = try t.db.addEpisode(show_id, episode2);

    const watch_date = genDate(1024);

    try t.db.setEpisodeWatchStatus(episode_id, &.{.{ .watched = watch_date }});
    {
        const retrieved = (try t.db.getEpisode(t.gpa(), episode_id)).?;
        try testing.expectEqualDeep(@as([]const types.WatchStatus, &.{.{ .watched = watch_date }}), retrieved.watch_status);
    }

    try t.db.setEpisodeWatchStatus(episode_id, &.{.skipped});
    {
        const retrieved = (try t.db.getEpisode(t.gpa(), episode_id)).?;
        try testing.expectEqualDeep(@as([]const types.WatchStatus, &.{.skipped}), retrieved.watch_status);
    }

    try t.db.setEpisodeWatchStatus(episode_id, &.{.unwatched});
    {
        const retrieved = (try t.db.getEpisode(t.gpa(), episode_id)).?;
        try testing.expectEqualDeep(@as([]const types.WatchStatus, &.{.unwatched}), retrieved.watch_status);
    }
}

test "multiple playthroughs" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);

    const episode: types.RemoteEpisode = .{
        .name = "Test Episode",
        .season = 1,
        .episode = 34,
        .airdate = genDate(1023),
    };

    const episode2: types.RemoteEpisode = .{
        .name = "Test Episode 2",
        .season = 1,
        .episode = 35,
        .airdate = genDate(1023),
    };

    const show_id = try t.db.addShow(show);

    const episode_id = try t.db.addEpisode(show_id, episode);
    const episode_id_2 = try t.db.addEpisode(show_id, episode2);

    const watch_date = genDate(1024);

    try t.db.setEpisodeWatchStatus(episode_id, &.{
        .{ .watched = watch_date },
        .unwatched,
        .skipped,
    });

    {
        const retrieved = (try t.db.getEpisode(t.gpa(), episode_id)).?;
        try testing.expectEqualDeep(@as([]const types.WatchStatus, &.{
            .{ .watched = watch_date },
            .unwatched,
            .skipped,
        }), retrieved.watch_status);
    }

    {
        const retrieved_show = (try t.db.getShow(t.gpa(), show_id, genTimestamp(1024))).?;
        // Watched and skipped counts should reflect episode watch status
        try testing.expectEqualDeep(@as([]const i64, &.{ 1, 0, 0 }), retrieved_show.episodes_watched);
        try testing.expectEqualDeep(@as([]const i64, &.{ 0, 0, 1 }), retrieved_show.episodes_skipped);
    }

    {
        const retrieved = (try t.db.getEpisode(t.gpa(), episode_id_2)).?;
        // Second episode should still be tracked for all playthroughs
        try testing.expectEqualDeep(@as([]const types.WatchStatus, &.{
            .unwatched,
            .unwatched,
            .unwatched,
        }), retrieved.watch_status);
    }

    // Replacing the first element shouldn't change the number of playthroughs
    try t.db.setEpisodeWatchStatus(episode_id, &.{
        .unwatched,
        .unwatched,
        .skipped,
    });

    {
        const retrieved = (try t.db.getEpisode(t.gpa(), episode_id)).?;
        try testing.expectEqualDeep(@as([]const types.WatchStatus, &.{
            .unwatched,
            .unwatched,
            .skipped,
        }), retrieved.watch_status);
    }

    try t.db.setEpisodeWatchStatus(episode_id, &.{
        .unwatched,
        .unwatched,
        .unwatched,
    });

    {
        const retrieved = (try t.db.getEpisode(t.gpa(), episode_id)).?;
        // After all elements have been removed, we should be back to our first playthrough
        try testing.expectEqualDeep(@as([]const types.WatchStatus, &.{.unwatched}), retrieved.watch_status);
    }

    {
        const retrieved_show = (try t.db.getShow(t.gpa(), show_id, genTimestamp(1024))).?;
        // No playthroughs have been started now
        try testing.expectEqualDeep(@as([]const i64, &.{}), retrieved_show.episodes_watched);
        try testing.expectEqualDeep(@as([]const i64, &.{}), retrieved_show.episodes_skipped);
    }
}

test "set pause status" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);
    const show2 = generateEmptyShow("Test Show 2", 1);

    const show_id1 = try t.db.addShow(show);
    const show_id2 = try t.db.addShow(show2);

    {
        const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
        try testing.expectEqual(false, findShowById(show_id1, shows).?.pause_status);
        try testing.expectEqual(false, findShowById(show_id2, shows).?.pause_status);
    }

    try t.db.setPauseStatus(show_id1, false);
    {
        const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
        try testing.expectEqual(false, findShowById(show_id1, shows).?.pause_status);
        try testing.expectEqual(false, findShowById(show_id2, shows).?.pause_status);
    }

    try t.db.setPauseStatus(show_id1, true);
    {
        const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
        try testing.expectEqual(true, findShowById(show_id1, shows).?.pause_status);
        try testing.expectEqual(false, findShowById(show_id2, shows).?.pause_status);
    }

    try t.db.setPauseStatus(show_id1, false);
    {
        const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
        try testing.expectEqual(false, findShowById(show_id1, shows).?.pause_status);
        try testing.expectEqual(false, findShowById(show_id2, shows).?.pause_status);
    }
}

test "remove show" {
    var t = try TestDb.init();
    defer t.deinit();

    var show = generateEmptyShow("Test Show", 0);
    show.image = "test_image";

    const episode: types.RemoteEpisode = .{
        .name = "Test Episode",
        .season = 1,
        .episode = 34,
        .airdate = genDate(1023),
    };

    const show_id = try t.db.addShow(show);
    const inserted_show = (try t.db.getShow(t.gpa(), show_id, genTimestamp(1234))).?;
    const image_id = inserted_show.image.?;
    try testing.expect((try t.db.getImageUrl(t.gpa(), image_id)) != null);

    try t.db.setPauseStatus(show_id, true);

    const episode_id = try t.db.addEpisode(show_id, episode);

    const watch_date = genDate(1024);
    try t.db.setEpisodeWatchStatus(episode_id, &.{.{ .watched = watch_date }});

    try testing.expectEqual(1, (try t.db.getEpisodesForShow(t.gpa(), show_id)).len);

    try t.db.removeShow(show_id);

    try testing.expect((try t.db.getImageUrl(t.gpa(), image_id)) == null);

    try testing.expectEqual(0, (try t.db.getEpisodesForShow(t.gpa(), show_id)).len);
}

test "shows aired between" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);
    const show2 = generateEmptyShow("Test Show 2", 0);
    const show3 = generateEmptyShow("Test Show 3", 0);

    const show_id1 = try t.db.addShow(show);
    const show_id2 = try t.db.addShow(show2);

    // Show 3 has no episodes
    _ = try t.db.addShow(show3);

    const show_ids = [_]types.ShowId{ show_id1, show_id2 };
    var i: i64 = 0;
    while (i < 100) : (i += 1) {
        const show_id = show_ids[@intCast(@mod(i, 2))];
        const episode: types.RemoteEpisode = .{
            .name = "Test episode",
            .season = 1,
            .episode = i,
            // 4 episodes a day, starting at 1000
            .airdate = genDate(@divTrunc(4000 + i, 4)),
        };
        _ = try t.db.addEpisode(show_id, episode);
    }

    const start_date = sphtud.datetime.Date.fromCeDay(1012);
    const end_date = sphtud.datetime.Date.fromCeDay(1014);

    const episodes = try t.db.getEpisodesAiredBetween(t.gpa(), start_date, end_date);

    // Airdates are inclusive, so we should expect 3 days of 4 episodes a day
    try testing.expectEqual(12, episodes.len);

    var min_days: i64 = std.math.maxInt(i64);
    var max_days: i64 = std.math.minInt(i64);
    for (episodes) |ep| {
        const days = ep.airdate.?.inner.toCeDay();
        min_days = @min(min_days, days);
        max_days = @max(max_days, days);
    }
    try testing.expectEqual(start_date.toCeDay(), min_days);
    try testing.expectEqual(end_date.toCeDay(), max_days);
}

test "ratings" {
    var t = try TestDb.init();
    defer t.deinit();

    const show_names = [_][]const u8{ "Show 1", "Show 2", "Show 3" };
    var ids: [3]types.ShowId = undefined;
    for (show_names, 0..) |name, idx| ids[idx] = try t.db.addShow(generateEmptyShow(name, @intCast(idx)));

    const rating_names = [_][]const u8{ "Super good", "bad", "good", "unbearable" };
    var rating_ids: [4]types.RatingId = undefined;
    for (rating_names, 0..) |name, idx| rating_ids[idx] = try t.db.addRating(name);

    {
        const ratings = try t.db.getRatings(t.gpa());
        try testing.expectEqual(4, ratings.len);
        try testing.expectEqual(@as(i64, 1), findRatingById(rating_ids[0], ratings).?.priority);
        try testing.expectEqualStrings("Super good", findRatingById(rating_ids[0], ratings).?.name);
        try testing.expectEqual(@as(i64, 2), findRatingById(rating_ids[1], ratings).?.priority);
        try testing.expectEqualStrings("bad", findRatingById(rating_ids[1], ratings).?.name);
        try testing.expectEqual(@as(i64, 3), findRatingById(rating_ids[2], ratings).?.priority);
        try testing.expectEqualStrings("good", findRatingById(rating_ids[2], ratings).?.name);
        try testing.expectEqual(@as(i64, 4), findRatingById(rating_ids[3], ratings).?.priority);
        try testing.expectEqualStrings("unbearable", findRatingById(rating_ids[3], ratings).?.name);

        // Swap the priorities of "bad" and "good"
        var bad = findRatingById(rating_ids[1], ratings).?;
        bad.priority = 3;
        try t.db.updateRating(bad);

        var good = findRatingById(rating_ids[2], ratings).?;
        good.priority = 2;
        try t.db.updateRating(good);
    }

    {
        const ratings = try t.db.getRatings(t.gpa());
        try testing.expectEqual(4, ratings.len);
        try testing.expectEqual(@as(i64, 1), findRatingById(rating_ids[0], ratings).?.priority);
        try testing.expectEqualStrings("Super good", findRatingById(rating_ids[0], ratings).?.name);
        try testing.expectEqual(@as(i64, 3), findRatingById(rating_ids[1], ratings).?.priority);
        try testing.expectEqualStrings("bad", findRatingById(rating_ids[1], ratings).?.name);
        try testing.expectEqual(@as(i64, 2), findRatingById(rating_ids[2], ratings).?.priority);
        try testing.expectEqualStrings("good", findRatingById(rating_ids[2], ratings).?.name);
        try testing.expectEqual(@as(i64, 4), findRatingById(rating_ids[3], ratings).?.priority);
        try testing.expectEqualStrings("unbearable", findRatingById(rating_ids[3], ratings).?.name);
    }

    try t.db.setShowRating(ids[0], rating_ids[0]);
    try t.db.setShowRating(ids[2], rating_ids[3]);

    {
        const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
        try testing.expectEqual(3, shows.len);
        try testing.expectEqualDeep(@as(?types.RatingId, rating_ids[0]), findShowById(ids[0], shows).?.rating_id);
        try testing.expectEqualDeep(@as(?types.RatingId, null), findShowById(ids[1], shows).?.rating_id);
        try testing.expectEqualDeep(@as(?types.RatingId, rating_ids[3]), findShowById(ids[2], shows).?.rating_id);
    }

    try t.db.deleteRating(rating_ids[0]);

    {
        const ratings = try t.db.getRatings(t.gpa());
        try testing.expectEqual(3, ratings.len);
        try testing.expectEqual(@as(i64, 3), findRatingById(rating_ids[1], ratings).?.priority);
        try testing.expectEqualStrings("bad", findRatingById(rating_ids[1], ratings).?.name);
        try testing.expectEqual(@as(i64, 2), findRatingById(rating_ids[2], ratings).?.priority);
        try testing.expectEqualStrings("good", findRatingById(rating_ids[2], ratings).?.name);
        try testing.expectEqual(@as(i64, 4), findRatingById(rating_ids[3], ratings).?.priority);
        try testing.expectEqualStrings("unbearable", findRatingById(rating_ids[3], ratings).?.name);
    }

    {
        const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
        try testing.expectEqual(3, shows.len);
        // Deleting the rating clears it from the show that referenced it
        try testing.expectEqualDeep(@as(?types.RatingId, null), findShowById(ids[0], shows).?.rating_id);
        try testing.expectEqualDeep(@as(?types.RatingId, null), findShowById(ids[1], shows).?.rating_id);
        try testing.expectEqualDeep(@as(?types.RatingId, rating_ids[3]), findShowById(ids[2], shows).?.rating_id);
    }

    try t.db.setShowRating(ids[2], null);

    {
        const shows = try t.db.getShows(t.gpa(), genTimestamp(1234));
        try testing.expectEqual(3, shows.len);
        try testing.expectEqualDeep(@as(?types.RatingId, null), findShowById(ids[0], shows).?.rating_id);
        try testing.expectEqualDeep(@as(?types.RatingId, null), findShowById(ids[1], shows).?.rating_id);
        try testing.expectEqualDeep(@as(?types.RatingId, null), findShowById(ids[2], shows).?.rating_id);
    }
}

fn testMovie() types.RemoteMovie {
    return .{
        .imdb_id = "test",
        .name = "movie",
        .year = 1234,
        .image = "http://image",
        .theater_release_date = genDate(1234),
        .home_release_date = genDate(1244),
    };
}

fn testTimestamp() std.Io.Timestamp {
    return .fromNanoseconds(123094 * std.time.ns_per_s);
}

test "full movie in out" {
    var t = try TestDb.init();
    defer t.deinit();

    const movie = testMovie();

    const movie_id = try t.db.addMovie(movie, testTimestamp());

    const inserted_movie = (try t.db.getMovie(t.gpa(), movie_id)).?;
    try testing.expectEqual(inserted_movie.last_update_time, testTimestamp());
    try testing.expectEqualDeep(movie, try remoteFromMovie(t.gpa(), &t.db, inserted_movie));

    const movies = try t.db.getMovies(t.gpa());
    try testing.expectEqual(1, movies.len);
    try testing.expectEqualDeep(inserted_movie, movies[0]);
}

test "add duplicate movie" {
    var t = try TestDb.init();
    defer t.deinit();

    const movie = testMovie();

    const movie_id = try t.db.addMovie(movie, testTimestamp());
    const movie_id2 = try t.db.addMovie(movie, testTimestamp());
    try testing.expectEqual(movie_id.inner, movie_id2.inner);
}

test "update movie poster uri" {
    var t = try TestDb.init();
    defer t.deinit();

    var movie = testMovie();
    const movie_id = try t.db.addMovie(movie, testTimestamp());

    movie.image = "http://new_image";
    _ = try t.db.addMovie(movie, testTimestamp());

    const updated = (try t.db.getMovie(t.gpa(), movie_id)).?;
    const remote = try remoteFromMovie(t.gpa(), &t.db, updated);
    try testing.expectEqualStrings("http://new_image", remote.image);
}

test "watch movie" {
    var t = try TestDb.init();
    defer t.deinit();

    const movie_id = try t.db.addMovie(testMovie(), testTimestamp());
    try testing.expectEqual(false, (try t.db.getMovie(t.gpa(), movie_id)).?.watched);

    try t.db.setMovieWatchStatus(movie_id, genDate(1234).inner);
    try testing.expectEqual(true, (try t.db.getMovie(t.gpa(), movie_id)).?.watched);

    try t.db.setMovieWatchStatus(movie_id, null);
    try testing.expectEqual(false, (try t.db.getMovie(t.gpa(), movie_id)).?.watched);
}

test "rate movie" {
    var t = try TestDb.init();
    defer t.deinit();

    const movie_id = try t.db.addMovie(testMovie(), testTimestamp());
    try testing.expectEqualDeep(@as(?types.RatingId, null), (try t.db.getMovie(t.gpa(), movie_id)).?.rating_id);

    const rating_id = try t.db.addRating("test");

    try t.db.setMovieRating(movie_id, rating_id);
    try testing.expectEqualDeep(@as(?types.RatingId, rating_id), (try t.db.getMovie(t.gpa(), movie_id)).?.rating_id);

    try t.db.setMovieRating(movie_id, null);
    try testing.expectEqualDeep(@as(?types.RatingId, null), (try t.db.getMovie(t.gpa(), movie_id)).?.rating_id);
}

test "delete movie" {
    var t = try TestDb.init();
    defer t.deinit();

    const movie_id = try t.db.addMovie(testMovie(), testTimestamp());
    const inserted_movie = (try t.db.getMovie(t.gpa(), movie_id)).?;
    try testing.expect((try t.db.getImageUrl(t.gpa(), inserted_movie.image)) != null);

    try t.db.deleteMovie(movie_id);
    try testing.expect((try t.db.getMovie(t.gpa(), movie_id)) == null);
    try testing.expect((try t.db.getImageUrl(t.gpa(), inserted_movie.image)) == null);
    try testing.expectEqual(0, (try t.db.getMovies(t.gpa())).len);
}

test "show notes" {
    var t = try TestDb.init();
    defer t.deinit();

    const show = generateEmptyShow("Test Show", 0);
    const show_id = try t.db.addShow(show);

    {
        const s = (try t.db.getShow(t.gpa(), show_id, genTimestamp(1234))).?;
        try testing.expect(s.notes == null);
    }

    try t.db.setShowNotes(show_id, "These are my notes");

    {
        const s = (try t.db.getShow(t.gpa(), show_id, genTimestamp(1234))).?;
        try testing.expectEqualStrings("These are my notes", s.notes.?);
    }
}
