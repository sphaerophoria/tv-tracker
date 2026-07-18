const std = @import("std");
const sphtud = @import("sphtud");
const types = @import("types.zig");

const RemoteTvShow = types.RemoteTvShow(types.TvMazeShowId);
const RemoteEpisode = types.RemoteEpisode;

// Intermediate representations matching the TvMaze JSON, mirroring the `Api*`
// structs in the Rust `tv_maze` module. Only the fields we care about are
// declared; everything else is ignored during parsing.
const ApiImage = struct {
    medium: []const u8,
};

const ApiExternals = struct {
    thetvdb: ?i64 = null,
    imdb: ?[]const u8 = null,
};

const ApiShow = struct {
    id: i64,
    name: []const u8,
    premiered: ?[]const u8 = null,
    image: ?ApiImage = null,
    url: ?[]const u8 = null,
    externals: ?ApiExternals = null,

    fn toRemote(self: ApiShow) RemoteTvShow {
        var imdb_id: ?types.ImdbShowId = null;
        var tvdb_id: ?types.TvdbShowId = null;
        if (self.externals) |ext| {
            if (ext.imdb) |v| imdb_id = .{ .inner = v };
            if (ext.thetvdb) |v| tvdb_id = .{ .inner = v };
        }

        // Mirrors Rust's `premiered.map(|d| d.year())`.
        const year: ?i32 = if (self.premiered) |p| blk: {
            const d = sphtud.datetime.Date.parse(p) catch break :blk null;
            break :blk @intCast(d.year);
        } else null;

        return .{
            .id = .{ .inner = self.id },
            .name = self.name,
            .image = if (self.image) |img| img.medium else null,
            .year = year,
            .url = self.url,
            .imdb_id = imdb_id,
            .tvdb_id = tvdb_id,
        };
    }
};

const ApiSearchItem = struct {
    score: f32,
    show: ApiShow,
};

// Mirrors Rust's `ApiEpisode`. The episode number is `number` in the TvMaze
// JSON but `episode` in our domain type; `airdate` arrives as a string and is
// parsed to an optional Date, matching Rust's `parse_from_str(...).ok()`.
const ApiEpisode = struct {
    id: i64,
    name: []const u8,
    season: i64,
    number: i64,
    airdate: []const u8,

    fn toRemote(self: ApiEpisode) RemoteEpisode {
        const airdate: ?types.Date = if (sphtud.datetime.Date.parse(self.airdate)) |d|
            .{ .inner = d }
        else |_|
            null;

        return .{
            .name = self.name,
            .season = self.season,
            .episode = self.number,
            .airdate = airdate,
        };
    }
};

pub const Search = union(enum) {
    waiting: struct {
        fetcher: sphtud.io.SimpleHttpTls,
        service_id: usize,
    },
    finished: []const RemoteTvShow,

    pub fn initPinned(
        self: *Search,
        alloc: std.mem.Allocator,
        query_percent_encoded: []const u8,
        spawner: *sphtud.io.tls.Spawner,
        service_id: usize,
    ) !void {
        const url = try buildSearchUrl(alloc, query_percent_encoded);

        // Pin the fetcher in its final location: SimpleHttpTls is
        // self-referential, and `Search` itself is heap-allocated and never
        // moved, so an in-place `initPinned` is safe.
        self.* = .{ .waiting = .{ .fetcher = undefined, .service_id = service_id } };
        try self.waiting.fetcher.initPinned(alloc, try .parse(url), .{}, spawner, service_id);
    }

    pub fn poll(self: *Search, loop: *sphtud.io.Loop) !?[]const RemoteTvShow {
        switch (self.*) {
            .waiting => |*p| {
                // Mirror Rust's `App::search`, which logs and falls back to an
                // empty result set on any tv_maze failure rather than failing
                // the request.
                const body = p.fetcher.poll(loop, p.service_id) catch |e| {
                    std.log.err("Failed to execute tv search: {t}", .{e});
                    return self.finish(&.{});
                } orelse return null;

                const results = parseResults(p.fetcher.alloc, body) catch |e| {
                    std.log.err("Failed to parse tv search: {t}", .{e});
                    return self.finish(&.{});
                };

                return self.finish(results);
            },
            .finished => |res| return res,
        }
    }

    // Tear down the fetcher and transition to `finished`. Takes `self` by
    // pointer; the caller must not touch the `waiting` payload afterwards.
    fn finish(self: *Search, results: []const RemoteTvShow) []const RemoteTvShow {
        self.waiting.fetcher.deinit();
        self.* = .{ .finished = results };
        return results;
    }

    pub fn deinit(self: *Search) void {
        switch (self.*) {
            .waiting => |*p| p.fetcher.deinit(),
            .finished => {},
        }
    }
};

pub const Episodes = struct {
    state: union(enum) {
        wait: struct {
            fetcher: sphtud.io.SimpleHttpTls,
            service_id: usize,
        },
        finished: []const RemoteEpisode,
    },

    pub fn initPinned(
        self: *Episodes,
        alloc: std.mem.Allocator,
        remote_id: types.TvMazeShowId,
        spawner: *sphtud.io.tls.Spawner,
        service_id: usize,
    ) !void {
        const episodes_url = try std.fmt.allocPrint(alloc, "https://api.tvmaze.com/shows/{d}/episodes", .{remote_id.inner});

        self.state = .{ .wait = .{ .service_id = service_id, .fetcher = undefined } };
        try self.state.wait.fetcher.initPinned(
            alloc,
            try .parse(episodes_url),
            .{},
            spawner,
            service_id,
        );
    }

    pub fn deinit(self: *Episodes) void {
        switch (self.state) {
            .wait => |*w| w.fetcher.deinit(),
            .finished => {},
        }
    }

    pub fn poll(self: *Episodes, loop: *sphtud.io.Loop) !?[]const RemoteEpisode {
        switch (self.state) {
            .wait => |*w| {
                const body = try w.fetcher.poll(loop, w.service_id) orelse return null;
                self.state = .{ .finished = try parseEpisodes(w.fetcher.alloc, body) };
                return self.state.finished;
            },
            .finished => |res| return res,
        }
    }
};

// Fetches a show's metadata and its full episode list from TvMaze, mirroring
// Rust's `tv_maze::show` + `tv_maze::episodes` pair (used together by
// `App::add_show`). Both requests are issued up front and polled concurrently;
// the retriever only reports `finished` once both bodies have arrived. Unlike
// `Search`, any fetch/parse failure is propagated so the caller fails the
// request, matching Rust's `add_show` which returns an error on lookup failure.
pub const ShowMeta = struct {
    alloc: std.mem.Allocator,
    show: sphtud.io.SimpleHttpTls,
    episodes: Episodes,
    service_id: usize,
    state: union(enum) {
        wait,
        finished: Result,
    },

    pub const Result = struct {
        show: RemoteTvShow,
        episodes: []const RemoteEpisode,
    };

    pub fn initPinned(
        self: *ShowMeta,
        alloc: std.mem.Allocator,
        remote_id: types.TvMazeShowId,
        spawner: *sphtud.io.tls.Spawner,
        service_id: usize,
    ) !void {
        const show_url = try std.fmt.allocPrint(alloc, "https://api.tvmaze.com/shows/{d}", .{remote_id.inner});

        // Pin both fetchers in place: SimpleHttpTls is self-referential and
        // `ShowMeta` itself is heap-allocated and never moved.
        self.* = .{
            .alloc = alloc,
            .show = undefined,
            .episodes = undefined,
            .service_id = service_id,
            .state = .wait,
        };
        try self.show.initPinned(alloc, try .parse(show_url), .{}, spawner, service_id);
        try self.episodes.initPinned(alloc, remote_id, spawner, service_id);
    }

    pub fn poll(self: *ShowMeta, loop: *sphtud.io.Loop) !?Result {
        switch (self.state) {
            .finished => |res| return res,
            .wait => {},
        }

        const show_response_opt = try self.show.poll(loop, self.service_id);
        const episodes_response_opt = try self.episodes.poll(loop);

        const show_response = show_response_opt orelse return null;
        const episodes_response = episodes_response_opt orelse return null;

        const api_show = try std.json.parseFromSliceLeaky(ApiShow, self.alloc, show_response, .{
            .ignore_unknown_fields = true,
        });

        const result: Result = .{
            .show = api_show.toRemote(),
            .episodes = episodes_response,
        };

        self.show.deinit();
        self.episodes.deinit();
        self.state = .{ .finished = result };
        return result;
    }

    pub fn deinit(self: *ShowMeta) void {
        switch (self.state) {
            .wait => {
                self.show.deinit();
                self.episodes.deinit();
            },
            .finished => {},
        }
    }
};

fn parseEpisodes(alloc: std.mem.Allocator, body: []const u8) ![]const RemoteEpisode {
    const items = try std.json.parseFromSliceLeaky([]const ApiEpisode, alloc, body, .{
        .ignore_unknown_fields = true,
    });

    const out = try alloc.alloc(RemoteEpisode, items.len);
    for (items, out) |item, *dst| {
        dst.* = item.toRemote();
    }
    return out;
}

fn parseResults(alloc: std.mem.Allocator, body: []const u8) ![]const RemoteTvShow {
    const items = try std.json.parseFromSliceLeaky([]const ApiSearchItem, alloc, body, .{
        .ignore_unknown_fields = true,
    });

    const out = try alloc.alloc(RemoteTvShow, items.len);
    for (items, out) |item, *dst| {
        dst.* = item.show.toRemote();
    }
    return out;
}

fn buildSearchUrl(alloc: std.mem.Allocator, query: []const u8) ![]const u8 {
    return try std.fmt.allocPrint(alloc, "https://api.tvmaze.com/search/shows?q={s}", .{query});
}

// Percent-encode a query the same way the Rust `urlencoding` crate does:
// everything except the unreserved set (A-Z a-z 0-9 - _ . ~) is escaped.
fn percentEncode(w: *std.Io.Writer, s: []const u8) !void {
    const hex = "0123456789ABCDEF";
    for (s) |ch| switch (ch) {
        'A'...'Z', 'a'...'z', '0'...'9', '-', '_', '.', '~' => try w.writeByte(ch),
        else => {
            try w.writeByte('%');
            try w.writeByte(hex[ch >> 4]);
            try w.writeByte(hex[ch & 0x0f]);
        },
    };
}

const testing = std.testing;

// Ported from the Rust `tv_maze` tests: confirm the real TvMaze fixtures
// deserialize into our intermediate `Api*` structs.
test "search deserialization" {
    var arena = std.heap.ArenaAllocator.init(testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    inline for (.{
        @embedFile("res/tv_maze/search_result_banshee.json"),
        @embedFile("res/tv_maze/search_result_arcane.json"),
    }) |body| {
        _ = try std.json.parseFromSliceLeaky([]const ApiSearchItem, alloc, body, .{
            .ignore_unknown_fields = true,
        });
    }
}

test "episodes deserialization" {
    var arena = std.heap.ArenaAllocator.init(testing.allocator);
    defer arena.deinit();
    const alloc = arena.allocator();

    inline for (.{
        @embedFile("res/tv_maze/episodes_result.json"),
        @embedFile("res/tv_maze/episodes_resident_alien.json"),
    }) |body| {
        _ = try std.json.parseFromSliceLeaky([]const ApiEpisode, alloc, body, .{
            .ignore_unknown_fields = true,
        });
    }
}
