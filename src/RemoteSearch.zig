const std = @import("std");
const sphtud = @import("sphtud");
const wikipedia = @import("wikipedia.zig");
const tv_maze = @import("tv_maze.zig");
const types = @import("types.zig");

tv: tv_maze.Search,
movies: wikipedia.Search,

const RemoteSearch = @This();

const Ids = struct {
    const offsets = struct {
        const shows = 0;
        const movies = 1;
    };

    const Purpose = enum {
        shows,
        movies,
    };

    fn purpose(base_id: usize, event_id: usize) Purpose {
        const offset = event_id - base_id;
        if (offset == offsets.shows) return .shows;
        return .movies;
    }

    fn shows(base_id: usize) usize {
        return base_id;
    }

    fn movies(base_id: usize) usize {
        return base_id + offsets.movies;
    }
};

pub fn initPinned(
    self: *RemoteSearch,
    alloc: std.mem.Allocator,
    query_percent_encoded: []const u8,
    spawner: *sphtud.io.tls.Spawner,
    timer_service: *sphtud.io.TimerService,
    rate_limiter: *sphtud.util.RateLimiter,
    base_id: usize,
    concurrency: usize,
) !void {
    if (concurrency < 2) return error.ConcurrencyTooLow;

    const movie_concurrency = concurrency - 1;

    try self.tv.initPinned(
        alloc,
        query_percent_encoded,
        spawner,
        Ids.shows(base_id),
    );
    errdefer self.tv.deinit();

    try self.movies.initPinned(
        alloc,
        query_percent_encoded,
        spawner,
        timer_service,
        rate_limiter,
        Ids.movies(base_id),
        movie_concurrency,
    );
    errdefer self.movies.deinit();
}

pub fn deinit(self: *RemoteSearch) void {
    self.tv.deinit();
    self.movies.deinit();
}

pub const SearchResults = struct {
    shows: []const types.RemoteTvShow(types.TvMazeShowId),
    movies: []const wikipedia.SearchMovie,
};

pub fn poll(
    self: *RemoteSearch,
    spawner: *sphtud.io.tls.Spawner,
    loop: *sphtud.io.Loop,
    id: usize,
    base_id: usize,
) !?SearchResults {
    switch (Ids.purpose(base_id, id)) {
        .movies => try self.movies.poll(spawner, loop, id),
        else => {},
    }

    const shows_opt = try self.tv.poll(loop);
    const movies_opt = self.movies.result();

    return .{
        .shows = shows_opt orelse return null,
        .movies = movies_opt orelse return null,
    };
}
