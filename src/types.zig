const std = @import("std");
const sphtud = @import("sphtud");

pub const Shows = []const Show;

pub const Date = struct {
    inner: sphtud.datetime.Date,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        var buf: [32]u8 = undefined;
        const str = std.fmt.bufPrint(&buf, "{f}", .{self.inner}) catch unreachable;
        try s.write(str);
    }
};

pub const ShowId = struct {
    inner: i64,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const ImageId = struct {
    inner: i64,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const ImdbShowId = struct {
    inner: []const u8,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const TvdbShowId = struct {
    inner: i64,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const TvMazeShowId = struct {
    inner: i64,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const RatingId = struct {
    inner: i64,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const Rating = struct {
    id: RatingId,
    name: []const u8,
    priority: i64,
};

pub const EpisodeId = struct {
    inner: i64,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const MovieId = struct {
    inner: i64,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        try s.write(self.inner);
    }
};

pub const WatchStatus = union(enum) {
    watched: Date,
    skipped,
    unwatched,

    pub fn jsonStringify(self: @This(), s: *std.json.Stringify) !void {
        switch (self) {
            .watched => |date| {
                try s.beginObject();
                try s.objectField("Watched");
                try s.write(date);
                try s.endObject();
            },
            .skipped => try s.write("Skipped"),
            .unwatched => try s.write("Unwatched"),
        }
    }
};

pub const Episode = struct {
    id: EpisodeId,
    show_id: ShowId,
    name: []const u8,
    season: i64,
    episode: i64,
    airdate: ?Date,
    watch_status: []const WatchStatus,
};

pub const WatchStatusUpdate = union(enum) {
    watched: []const u8,
    skipped,
    unwatched,

    pub fn jsonParse(alloc: std.mem.Allocator, source: anytype, options: std.json.ParseOptions) !@This() {
        switch (try source.peekNextTokenType()) {
            .string => {
                const s = try std.json.innerParse([]const u8, alloc, source, options);
                if (std.mem.eql(u8, s, "Skipped")) return .skipped;
                if (std.mem.eql(u8, s, "Unwatched")) return .unwatched;
                return error.UnexpectedToken;
            },
            .object_begin => {
                if (.object_begin != try source.next()) return error.UnexpectedToken;

                const key = try std.json.innerParse([]const u8, alloc, source, options);
                if (!std.mem.eql(u8, key, "Watched")) return error.UnexpectedToken;

                const date = try std.json.innerParse([]const u8, alloc, source, options);

                if (.object_end != try source.next()) return error.UnexpectedToken;
                return .{ .watched = date };
            },
            else => return error.UnexpectedToken,
        }
    }
};

pub const ShowUpdate = struct {
    id: i64,
    pause_status: ?bool = null,
    rating_id: ?i64 = null,
    notes: ?[]const u8 = null,
};

pub const EpisodeUpdate = struct {
    id: i64,
    watch_status: []const WatchStatusUpdate,
};

pub const MovieUpdate = struct {
    id: i64,
    watched: bool,
    rating_id: ?i64 = null,
};

pub const SetRatingsRequest = struct {
    name: []const u8,
};

pub const RatingUpdate = struct {
    id: i64,
    name: []const u8,
    priority: i64,
};

pub const RemoteEpisode = struct {
    name: []const u8,
    season: i64,
    episode: i64,
    airdate: ?Date,
};

pub const PutShowsRequest = struct {
    remote_id: i64,
};

pub fn RemoteTvShow(RemoteId: type) type {
    return struct {
        id: RemoteId,
        name: []const u8,
        image: ?[]const u8,
        year: ?i32,
        url: ?[]const u8,
        imdb_id: ?ImdbShowId,
        tvdb_id: ?TvdbShowId,
    };
}

pub const Show = struct {
    id: ShowId,
    remote_id: TvMazeShowId,
    name: []const u8,
    image: ?ImageId,
    year: ?i32,
    url: ?[]const u8,
    imdb_id: ?ImdbShowId,
    tvdb_id: ?TvdbShowId,
    pause_status: bool,
    episodes_watched: []const i64,
    episodes_skipped: []const i64,
    episodes_aired: i64,
    rating_id: ?RatingId,
    notes: ?[]const u8,
};

pub const RemoteMovie = struct {
    imdb_id: []const u8,
    name: []const u8,
    year: i32,
    image: []const u8,
    theater_release_date: ?Date,
    home_release_date: ?Date,
};

pub const Movie = struct {
    id: MovieId,
    imdb_id: []const u8,
    name: []const u8,
    image: ImageId,
    year: i32,
    watched: bool,
    rating_id: ?RatingId,
    theater_release_date: ?Date,
    home_release_date: ?Date,
    last_update_time: ?std.Io.Timestamp,
};
