const std = @import("std");
const sphtud = @import("sphtud");
const sys = sphtud.io.system;
const Db = @import("Db.zig");
const ImageCache = @import("ImageCache.zig");
const tv_maze = @import("tv_maze.zig");
const types = @import("types.zig");
const wikipedia = @import("wikipedia.zig");
const RemoteSearch = @import("RemoteSearch.zig");

alloc: *sphtud.alloc.Sphalloc,
pool: sphtud.util.ObjectPool(Connection, usize),
loop: *sphtud.io.Loop,
db: *Db,
image_cache: *ImageCache,
spawner: *sphtud.io.tls.Spawner,
timer_service: *sphtud.io.TimerService,
rate_limiter: *sphtud.util.RateLimiter,
listener: c_int,
resource_dir: c_int,

const Server = @This();

pub const Ids = struct {
    accept: usize,
    connection: sphtud.util.IdAlloc.Range,
    total: sphtud.util.IdAlloc.Range,

    const connection_concurrency = 9;

    pub fn init(alloc: *sphtud.util.IdAlloc) Ids {
        const start = alloc.mark();
        return .{
            .accept = alloc.allocOne(),
            .connection = alloc.allocMany(1024 * connection_concurrency),
            .total = start.range(),
        };
    }
};

pub fn init(parent_alloc: *sphtud.alloc.Sphalloc, port: u16, resource_dir: c_int, db: *Db, image_cache: *ImageCache, spawner: *sphtud.io.tls.Spawner, timer_service: *sphtud.io.TimerService, rate_limiter: *sphtud.util.RateLimiter, loop: *sphtud.io.Loop, ids: Ids) !Server {
    const socket = try sphtud.io.createTcpListener(.{
        .ip4 = .{
            .bytes = .{ 0, 0, 0, 0 },
            .port = port,
        },
    }, 1024);
    errdefer sphtud.io.close(socket);

    try loop.register(.{
        .handle = socket,
        .id = ids.accept,
        .write = false,
        .read = true,
    });

    const alloc = try parent_alloc.makeSubAlloc("server");

    return .{
        .alloc = alloc,
        .pool = try .init(
            alloc.arena(),
            alloc.expansion(),
            8,
            (ids.connection.end - ids.connection.start + 1) / Ids.connection_concurrency,
        ),
        .loop = loop,
        .db = db,
        .image_cache = image_cache,
        .spawner = spawner,
        .timer_service = timer_service,
        .rate_limiter = rate_limiter,
        .listener = socket,
        .resource_dir = resource_dir,
    };
}

pub fn deinit(self: *Server) void {
    var it = self.pool.iter();
    while (it.next()) |item| {
        item.val.deinit();
    }

    sphtud.io.close(self.listener);
    self.alloc.deinit();
}

pub fn service(self: *Server, id: usize, comptime ids: Ids) !void {
    switch (id) {
        ids.accept => {
            while (true) {
                const conn_fd = sphtud.io.accept(self.listener) catch |e| {
                    if (e == error.WouldBlock) return;
                    return e;
                };
                errdefer sphtud.io.close(conn_fd);

                const expansion_alloc = self.alloc.expansion();
                const conn = try self.pool.acquire(expansion_alloc);
                errdefer self.pool.release(expansion_alloc, conn.handle);

                try self.loop.register(.{
                    .id = ids.connection.start + conn.handle * Ids.connection_concurrency,
                    .handle = conn_fd,
                    .read = true,
                    .write = true,
                });

                conn.val.* = .{
                    .alloc = try self.alloc.makeSubAlloc("connection"),
                    .fd = conn_fd,
                    .reader_buf = undefined,
                    .writer_buf = undefined,
                    .body_buf = undefined,
                    .reader = .init(conn_fd, &conn.val.reader_buf),
                    .http_reader = .init(&conn.val.reader.interface),
                    .writer = .init(conn_fd, &conn.val.writer_buf),
                    .body = .empty,
                    .state = .recv_head,
                    .base_id = ids.connection.start + conn.handle * Ids.connection_concurrency,
                };
            }
        },
        ids.connection.start...ids.connection.end => {
            const conn_id = (id - ids.connection.start) / Ids.connection_concurrency;
            const conn = self.pool.get(conn_id);
            switch (conn.poll(self, id)) {
                .wait => {},
                .finish => {
                    const base_id = conn.base_id;
                    conn.deinit();
                    self.pool.release(self.alloc.expansion(), conn_id);
                    for (base_id..base_id + Ids.connection_concurrency) |i| {
                        self.loop.clearEvents(i);
                    }
                },
            }
        },
        else => unreachable,
    }
}

pub const Connection = struct {
    // Reset at the end of every response
    alloc: *sphtud.alloc.Sphalloc,

    fd: c_int,
    reader_buf: [4096]u8,
    reader: sphtud.io.Reader,
    writer_buf: [4096]u8,
    writer: sphtud.io.Writer,
    body_buf: [4096]u8,
    http_reader: sphtud.http.HttpRequestReader,
    base_id: usize,

    body: std.ArrayList(u8),

    state: union(enum) {
        recv_head,
        recv_body: struct {
            req: sphtud.http.HttpRequestHeader,
            body_reader: *std.Io.Reader,
        },
        handle: struct {
            req: sphtud.http.HttpRequestHeader,
            target: Target,

            // Type erased response data. Types are resolved according to which target
            // is being processed
            extra_data: ?*anyopaque,
        },
        respond: struct {
            req: sphtud.http.HttpRequestHeader,
            response: []const u8,
        },
    },

    pub fn deinit(self: *Connection) void {
        self.resetExtra();
        self.alloc.deinit();
        sphtud.io.close(self.fd);
    }

    fn resetExtra(self: *Connection) void {
        switch (self.state) {
            .handle => |hp| blk: {
                switch (hp.target) {
                    .get_image => {
                        const getter: *ImageCache.Get = @ptrCast(@alignCast((hp.extra_data orelse break :blk)));
                        getter.cancel();
                    },
                    .search => {
                        const searcher: *RemoteSearch = @ptrCast(@alignCast((hp.extra_data orelse break :blk)));
                        searcher.deinit();
                    },
                    .put_shows => {
                        const retriever: *tv_maze.ShowMeta = @ptrCast(@alignCast((hp.extra_data orelse break :blk)));
                        retriever.deinit();
                    },
                    .put_movies => {
                        const resolver: *wikipedia.RemoteMovieResolver = @ptrCast(@alignCast((hp.extra_data orelse break :blk)));
                        resolver.deinit();
                    },
                    else => {
                        std.debug.assert(hp.extra_data == null);
                    },
                }
            },
            else => {},
        }
    }

    const PollRes = enum {
        wait,
        finish,
    };

    pub fn poll(self: *Connection, server: *Server, event_id: usize) PollRes {
        self.reader.err = null;

        return self.tryPoll(server, event_id) catch |e| {
            if (self.reader.isWouldBlock(e)) return .wait;
            if (e == error.EndOfStream) {
                return .finish;
            }

            std.log.err("Unexpected err: {t}", .{e});
            if (@errorReturnTrace()) |et| {
                std.debug.dumpErrorReturnTrace(et);
            }
            return .finish;
        };
    }

    fn tryPoll(self: *Connection, server: *Server, event_id: usize) !PollRes {
        sw: switch (self.state) {
            .recv_head => {
                const res = try self.http_reader.poll(self.alloc.arena(), &self.body_buf);

                self.state = .{ .recv_body = .{
                    .req = res.header,
                    .body_reader = res.body_reader,
                } };
                continue :sw self.state;
            },
            .recv_body => |r| {
                // Only read a body when one is announced. A body-less request
                // (e.g. a keep-alive GET) has a "read until close" body reader,
                // so reading it would block until the connection closes.
                if (getContentLength(r.req) != null) {
                    try r.body_reader.appendRemaining(self.alloc.general(), &self.body, .unlimited);
                }

                const target_s = r.req.target;
                const target = Target.parse(r.req.method, target_s);
                self.state = .{ .handle = .{ .req = r.req, .target = target, .extra_data = null } };
                continue :sw self.state;
            },
            .handle => |*params| {
                const msg = if (self.handleRequest(server, params.target, &params.extra_data, event_id)) |msg| msg orelse return .wait else |e| msg: {
                    std.log.err("Failed to handle {s}: {t}", .{ params.req.target, e });
                    break :msg response_500;
                };

                self.resetExtra();
                self.state = .{ .respond = .{ .response = msg, .req = params.req } };
                continue :sw self.state;
            },
            .respond => |*ctx| {
                try self.sendResponse(&ctx.response);

                if (try wantsClose(ctx.req)) {
                    return .finish;
                }

                try self.alloc.reset();
                self.body = .empty;
                self.state = .recv_head;
                continue :sw self.state;
            },
        }
    }

    fn handleRequest(self: *Connection, server: *Server, target: Target, extra: *?*anyopaque, event_id: usize) !?[]const u8 {
        const body = self.body.items;
        switch (target) {
            .get_resource => |path| return try resourceFile(self.alloc.general(), server.resource_dir, path),
            .search => |query| {
                if (extra.* == null) {
                    const searcher = try self.alloc.arena().create(RemoteSearch);
                    try searcher.initPinned(
                        self.alloc.general(),
                        query,
                        server.spawner,
                        server.timer_service,
                        server.rate_limiter,
                        self.base_id,
                        Ids.connection_concurrency,
                    );
                    extra.* = searcher;
                }

                const searcher: *RemoteSearch = @ptrCast(@alignCast(extra.*));
                const results = try searcher.poll(server.spawner, server.loop, event_id, self.base_id) orelse return null;
                const out_body = try std.json.Stringify.valueAlloc(
                    self.alloc.general(),
                    results,
                    .{},
                );
                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .get_image => |id| {
                if (extra.* == null) {
                    const url = try server.db.getImageUrl(self.alloc.general(), id) orelse return response_404;
                    const retriever: *ImageCache.Get = try self.alloc.arena().create(ImageCache.Get);
                    retriever.* = try server.image_cache.get(self.alloc.general(), url, self.base_id);
                    extra.* = retriever;
                }

                const retriever: *ImageCache.Get = @ptrCast(@alignCast(extra.*));
                const content = try retriever.poll(server.loop) orelse return null;

                return try respondWithContent(
                    self.alloc.general(),
                    content,
                    "image/jpeg",
                );
            },
            .root => return try redirect(self.alloc.general(), "/watch_list.html"),
            .get_shows => {
                const now = try sphtud.io.clock_gettime(.REALTIME);
                const shows = try server.db.getShows(self.alloc.general(), now);

                var body_w = std.Io.Writer.Allocating.init(self.alloc.general());
                var json_w = std.json.Stringify{
                    .writer = &body_w.writer,
                };

                try json_w.beginObject();
                for (shows) |show| {
                    var id_s_buf: [64]u8 = undefined;
                    const id_s = try std.fmt.bufPrint(&id_s_buf, "{d}", .{show.id.inner});
                    try json_w.objectField(id_s);
                    try json_w.write(show);
                }
                try json_w.endObject();

                return try respondWithContent(
                    self.alloc.general(),
                    body_w.written(),
                    "application/json",
                );
            },
            .put_shows => {
                const params = std.json.parseFromSliceLeaky(types.PutShowsRequest, self.alloc.general(), body, .{
                    .ignore_unknown_fields = true,
                }) catch return response_400;

                const now = try sphtud.io.clock_gettime(.REALTIME);

                if (extra.* == null) {
                    const existing = try server.db.getShows(self.alloc.general(), now);

                    for (existing) |show| {
                        if (show.remote_id.inner == params.remote_id) {
                            std.log.err("Cannot add duplicate of the same show ({s})", .{show.name});
                            return response_500;
                        }
                    }

                    const retriever = try self.alloc.arena().create(tv_maze.ShowMeta);
                    const remote_id = types.TvMazeShowId{ .inner = params.remote_id };
                    try retriever.initPinned(self.alloc.general(), remote_id, server.spawner, self.base_id);
                    extra.* = retriever;
                }

                const retriever: *tv_maze.ShowMeta = @ptrCast(@alignCast(extra.*));
                const meta = try retriever.poll(server.loop) orelse return null;

                const show_id = try server.db.addShow(meta.show);
                for (meta.episodes) |episode| {
                    _ = server.db.addEpisode(show_id, episode) catch |e| {
                        std.log.err("Failed to insert episode into db: {t}", .{e});
                        continue;
                    };
                }

                const show = try server.db.getShow(self.alloc.general(), show_id, now) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), show, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .get_show => |id| {
                const now = try sphtud.io.clock_gettime(.REALTIME);
                const show = try server.db.getShow(self.alloc.general(), id, now) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), show, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .put_show => |id| {
                const update = std.json.parseFromSliceLeaky(types.ShowUpdate, self.alloc.general(), body, .{
                    .ignore_unknown_fields = true,
                }) catch return response_400;

                if (update.id != id.inner) return response_400;

                if (update.pause_status) |pause_status| {
                    try server.db.setPauseStatus(id, pause_status);
                }
                try server.db.setShowRating(id, if (update.rating_id) |r| .{ .inner = r } else null);
                if (update.notes) |notes| {
                    try server.db.setShowNotes(id, notes);
                }

                const now = try sphtud.io.clock_gettime(.REALTIME);
                const show = try server.db.getShow(self.alloc.general(), id, now) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), show, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .delete_show => |id| {
                try server.db.removeShow(id);
                return response_200;
            },
            .get_show_episodes => |id| {
                const episodes = try server.db.getEpisodesForShow(self.alloc.general(), id);

                var body_w = std.Io.Writer.Allocating.init(self.alloc.general());
                var json_w = std.json.Stringify{
                    .writer = &body_w.writer,
                };
                try json_w.beginObject();
                for (episodes) |episode| {
                    var id_s_buf: [64]u8 = undefined;
                    const id_s = try std.fmt.bufPrint(&id_s_buf, "{d}", .{episode.id.inner});
                    try json_w.objectField(id_s);
                    try json_w.write(episode);
                }
                try json_w.endObject();

                return try respondWithContent(
                    self.alloc.general(),
                    body_w.written(),
                    "application/json",
                );
            },
            .get_episodes => |params| {
                const episodes = try server.db.getEpisodesAiredBetween(self.alloc.general(), params.start_date, params.end_date);

                var body_w = std.Io.Writer.Allocating.init(self.alloc.general());
                var json_w = std.json.Stringify{
                    .writer = &body_w.writer,
                };
                try json_w.beginObject();
                for (episodes) |episode| {
                    var id_s_buf: [64]u8 = undefined;
                    const id_s = try std.fmt.bufPrint(&id_s_buf, "{d}", .{episode.id.inner});
                    try json_w.objectField(id_s);
                    try json_w.write(episode);
                }
                try json_w.endObject();

                return try respondWithContent(
                    self.alloc.general(),
                    body_w.written(),
                    "application/json",
                );
            },
            .put_episode => |id| {
                const update = std.json.parseFromSliceLeaky(types.EpisodeUpdate, self.alloc.general(), body, .{
                    .ignore_unknown_fields = true,
                }) catch return response_400;

                if (update.id != id.inner) return response_400;

                const statuses = try self.alloc.general().alloc(types.WatchStatus, update.watch_status.len);
                for (update.watch_status, statuses) |src, *dst| {
                    dst.* = switch (src) {
                        .watched => |date_s| .{ .watched = .{ .inner = sphtud.datetime.Date.parse(date_s) catch return response_400 } },
                        .skipped => .skipped,
                        .unwatched => .unwatched,
                    };
                }

                try server.db.setEpisodeWatchStatus(id, statuses);

                const episode = try server.db.getEpisode(self.alloc.general(), id) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), episode, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .get_ratings => {
                const ratings = try server.db.getRatings(self.alloc.general());

                var body_w = std.Io.Writer.Allocating.init(self.alloc.general());
                var json_w = std.json.Stringify{
                    .writer = &body_w.writer,
                };
                try json_w.beginObject();
                for (ratings) |rating| {
                    var id_s_buf: [64]u8 = undefined;
                    const id_s = try std.fmt.bufPrint(&id_s_buf, "{d}", .{rating.id.inner});
                    try json_w.objectField(id_s);
                    try json_w.write(rating);
                }
                try json_w.endObject();

                return try respondWithContent(
                    self.alloc.general(),
                    body_w.written(),
                    "application/json",
                );
            },
            .put_ratings => {
                const request = std.json.parseFromSliceLeaky(types.SetRatingsRequest, self.alloc.general(), body, .{
                    .ignore_unknown_fields = true,
                }) catch return response_400;

                const id = try server.db.addRating(request.name);
                const rating = try server.db.getRating(self.alloc.general(), id) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), rating, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .get_rating => |id| {
                const rating = try server.db.getRating(self.alloc.general(), id) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), rating, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .put_rating => |id| {
                const update = std.json.parseFromSliceLeaky(types.RatingUpdate, self.alloc.general(), body, .{
                    .ignore_unknown_fields = true,
                }) catch return response_400;

                if (update.id != id.inner) return response_400;

                try server.db.updateRating(.{
                    .id = id,
                    .name = update.name,
                    .priority = update.priority,
                });

                const rating = try server.db.getRating(self.alloc.general(), id) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), rating, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .delete_rating => |id| {
                try server.db.deleteRating(id);
                return response_200;
            },
            .get_movies => {
                const movies = try server.db.getMovies(self.alloc.general());
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), movies, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .put_movies => {
                const update = std.json.parseFromSliceLeaky(struct {
                    wikipedia_page_id: i64,
                }, self.alloc.general(), body, .{
                    .ignore_unknown_fields = true,
                }) catch return response_400;

                if (extra.* == null) {
                    // Create some object who's input is a wikipedia page id and who's output is a RemoteMovie
                    const resolver = try self.alloc.arena().create(wikipedia.RemoteMovieResolver);
                    try resolver.initPinned(
                        self.alloc.arena(),
                        update.wikipedia_page_id,
                        server.spawner,
                        server.timer_service,
                        server.rate_limiter,
                        self.base_id,
                    );
                    extra.* = resolver;
                }

                const resolver: *wikipedia.RemoteMovieResolver = @ptrCast(@alignCast(extra.*));
                const remote_movie = try resolver.poll(server.spawner, server.loop) orelse return null;

                const now = try sphtud.io.clock_gettime(.REALTIME);
                const movie_id = try server.db.addMovie(remote_movie, now);

                const movie = try server.db.getMovie(self.alloc.general(), movie_id) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), movie, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .get_movie => |id| {
                const movie = try server.db.getMovie(self.alloc.general(), id) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), movie, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .put_movie => |id| {
                const update = std.json.parseFromSliceLeaky(types.MovieUpdate, self.alloc.general(), body, .{
                    .ignore_unknown_fields = true,
                }) catch return response_400;

                if (update.id != id.inner) return response_400;

                const now = try sphtud.io.clock_gettime(.REALTIME);
                const watch_date: ?sphtud.datetime.Date = if (update.watched)
                    sphtud.datetime.Date.fromCeDay(Db.toCeDay(now))
                else
                    null;

                try server.db.setMovieWatchStatus(id, watch_date);
                try server.db.setMovieRating(id, if (update.rating_id) |r| .{ .inner = r } else null);

                const movie = try server.db.getMovie(self.alloc.general(), id) orelse return response_404;
                const out_body = try std.json.Stringify.valueAlloc(self.alloc.general(), movie, .{});

                return try respondWithContent(
                    self.alloc.general(),
                    out_body,
                    "application/json",
                );
            },
            .delete_movie => |id| {
                try server.db.deleteMovie(id);
                return response_200;
            },
            .internal_error => return response_500,
            .bad_request => return response_400,
        }
    }

    fn sendResponse(self: *Connection, response: *[]const u8) !void {
        while (true) {
            if (response.len == 0) {
                try self.writer.interface.flush();
                return;
            }
            const written = try self.writer.interface.write(response.*);
            if (written >= response.len) response.* = &.{} else response.* = response.*[written..];
        }
    }
};

pub const Target = union(enum) {
    root,
    get_shows,
    put_shows,
    search: []const u8,
    get_show: types.ShowId,
    put_show: types.ShowId,
    delete_show: types.ShowId,
    get_show_episodes: types.ShowId,
    get_episodes: struct {
        start_date: sphtud.datetime.Date,
        end_date: sphtud.datetime.Date,
    },
    put_episode: types.EpisodeId,
    get_ratings,
    put_ratings,
    get_rating: types.RatingId,
    put_rating: types.RatingId,
    delete_rating: types.RatingId,
    get_movies,
    put_movies,
    get_movie: types.MovieId,
    put_movie: types.MovieId,
    delete_movie: types.MovieId,
    get_image: types.ImageId,
    get_resource: []const u8,
    internal_error,
    bad_request,

    pub fn parse(method: std.http.Method, target_s: []const u8) Target {
        const without_query = stripQuery(target_s);
        const without_slash = stripLeadingSlash(without_query);

        if (without_slash.len == 0) return .root;

        const fallthrough = Target{ .get_resource = without_slash };

        const First = enum {
            shows,
            ratings,
            images,
            episodes,
            movies,
            search,
        };

        var it = std.mem.splitScalar(u8, without_slash, '/');
        const first = std.meta.stringToEnum(First, it.next() orelse return .root) orelse return fallthrough;
        const second = it.next() orelse {
            if (first == .ratings and method == .PUT) return .put_ratings;
            if (first == .shows and method == .PUT) return .put_shows;
            if (first == .movies and method == .PUT) return .put_movies;
            if (method != .GET) return fallthrough;
            switch (first) {
                .shows => return .get_shows,
                .ratings => return .get_ratings,
                .episodes => {
                    var start_date: ?sphtud.datetime.Date = null;
                    var end_date: ?sphtud.datetime.Date = null;

                    var query_it = sphtud.http.url.QueryParamIter.init(target_s);
                    while (query_it.next()) |param| {
                        if (std.mem.eql(u8, param.key, "start_date")) {
                            start_date = sphtud.datetime.Date.parse(param.val) catch return .bad_request;
                        } else if (std.mem.eql(u8, param.key, "end_date")) {
                            end_date = sphtud.datetime.Date.parse(param.val) catch return .bad_request;
                        }
                    }
                    return .{ .get_episodes = .{
                        .start_date = start_date orelse return .bad_request,
                        .end_date = end_date orelse return .bad_request,
                    } };
                },
                .movies => return .get_movies,
                .images => return fallthrough,
                .search => {
                    var query_it = sphtud.http.url.QueryParamIter.init(target_s);
                    while (query_it.next()) |param| {
                        if (std.mem.eql(u8, param.key, "query")) {
                            return .{
                                .search = param.val,
                            };
                        }
                    }

                    return .bad_request;
                },
            }
        };

        const third = it.next();

        // Second is known, third is lazily parsed

        const id = switch (first) {
            .shows, .images, .movies, .episodes, .ratings => std.fmt.parseInt(i64, second, 10) catch return fallthrough,
            .search => return fallthrough,
        };

        switch (first) {
            .shows => {
                if (third) |t| {
                    if (std.mem.eql(u8, "episodes", t)) return .{ .get_show_episodes = .{ .inner = id } } else return fallthrough;
                }
                return switch (method) {
                    .PUT => .{ .put_show = .{ .inner = id } },
                    .DELETE => .{ .delete_show = .{ .inner = id } },
                    else => .{ .get_show = .{ .inner = id } },
                };
            },
            .images => {
                if (third != null) return fallthrough;
                if (method != .GET) return .internal_error;
                return .{ .get_image = .{ .inner = id } };
            },
            .movies => {
                if (third != null) return fallthrough;
                return switch (method) {
                    .PUT => .{ .put_movie = .{ .inner = id } },
                    .DELETE => .{ .delete_movie = .{ .inner = id } },
                    else => .{ .get_movie = .{ .inner = id } },
                };
            },
            .episodes => {
                if (third != null) return fallthrough;
                // Only PUT is implemented; GET /episodes/:id falls through to
                // static file handling (and thus 404s), matching prior behavior.
                return switch (method) {
                    .PUT => .{ .put_episode = .{ .inner = id } },
                    else => fallthrough,
                };
            },
            .ratings => {
                if (third != null) return fallthrough;
                return switch (method) {
                    .PUT => .{ .put_rating = .{ .inner = id } },
                    .DELETE => .{ .delete_rating = .{ .inner = id } },
                    else => .{ .get_rating = .{ .inner = id } },
                };
            },
            // `/search/<x>` already returned `fallthrough` in the id switch above.
            .search => unreachable,
        }
    }
};

fn stripQuery(target: []const u8) []const u8 {
    const end = std.mem.indexOfScalar(u8, target, '?') orelse target.len;
    return target[0..end];
}

fn stripLeadingSlash(target: []const u8) []const u8 {
    if (target.len == 0 or target[0] != '/') return target;
    return target[1..];
}

fn targetAsResourcePath(buf: []u8, target: []const u8) ![:0]const u8 {
    var stripped = if (target.len > 0 and target[0] == '/') target[1..] else target;
    const query_start = std.mem.indexOfScalar(u8, stripped, '?') orelse stripped.len;
    stripped = stripped[0..query_start];

    if (stripped.len + 1 >= buf.len) return error.PathTooLong;
    @memcpy(buf[0..stripped.len], stripped);
    buf[stripped.len] = 0;
    return buf[0..stripped.len :0];
}

fn resourceFile(gpa: std.mem.Allocator, resources: c_int, target: []const u8) ![]const u8 {
    var path_buf: [std.Io.Dir.max_path_bytes]u8 = undefined;
    const path = try targetAsResourcePath(&path_buf, target);

    const file_fd = sphtud.io.openat(resources, path, .{}, 0) catch return response_404;
    defer sphtud.io.close(file_fd);

    const content_len = try sphtud.io.lseek(file_fd, 0, sys.SEEK.END);
    _ = try sphtud.io.lseek(file_fd, 0, sys.SEEK.SET);

    var file_reader = sphtud.io.Reader.init(file_fd, &.{});
    var response_writer = std.Io.Writer.Allocating.init(gpa);
    var http_writer = sphtud.http.HttpWriter.init(&response_writer.writer);
    try http_writer.startResponse(.{
        .status = .ok,
        .content_length = content_len,
        .content_type = contentTypeFromTarget(target),
    });

    try http_writer.startBody();
    const len = try file_reader.interface.streamRemaining(&response_writer.writer);
    try response_writer.writer.flush();

    if (len != content_len) return error.InvalidLen;
    return response_writer.written();
}

fn contentTypeFromTarget(target: []const u8) ?[]const u8 {
    if (std.mem.endsWith(u8, target, ".js")) {
        return "text/javascript";
    } else if (std.mem.endsWith(u8, target, ".html")) {
        return "text/html";
    } else if (std.mem.endsWith(u8, target, ".png")) {
        return "image/png";
    } else if (std.mem.endsWith(u8, target, ".svg")) {
        return "image/svg+xml";
    }

    return null;
}

fn getContentLength(req: sphtud.http.HttpRequestHeader) ?usize {
    var it = req.fieldIter();
    while (it.next() catch return null) |kv| {
        var lower_buf: [1024]u8 = undefined;
        if (kv.key.len > lower_buf.len) continue;
        const lower_key = std.ascii.lowerString(&lower_buf, kv.key);
        if (std.mem.eql(u8, lower_key, "content-length")) {
            return std.fmt.parseInt(usize, kv.value, 10) catch null;
        }
    }
    return null;
}

fn wantsClose(req: sphtud.http.HttpRequestHeader) !bool {
    var it = req.fieldIter();
    while (try it.next()) |kv| {
        var lower_buf: [1024]u8 = undefined;
        if (kv.key.len > lower_buf.len) continue;
        const lower_key = std.ascii.lowerString(&lower_buf, kv.key);
        if (std.mem.eql(u8, lower_key, "connection")) {
            if (kv.value.len > lower_buf.len) return false;
            const lower_val = std.ascii.lowerString(&lower_buf, kv.value);
            return std.mem.eql(u8, lower_val, "close");
        }
    }
    return false;
}

fn redirect(alloc: std.mem.Allocator, target: []const u8) ![]const u8 {
    var allocating = std.Io.Writer.Allocating.init(alloc);
    var w = sphtud.http.HttpWriter.init(&allocating.writer);
    try w.startResponse(.{
        .status = .found,
        .content_length = 0,
    });
    try w.appendHeader("Location", target);
    try w.writeBody("");
    try allocating.writer.flush();

    return allocating.written();
}

fn respondWithContent(alloc: std.mem.Allocator, content: []const u8, content_type: []const u8) ![]const u8 {
    var response_w = std.Io.Writer.Allocating.init(alloc);
    var httpw = sphtud.http.HttpWriter.init(&response_w.writer);
    try httpw.startResponse(.{
        .status = .ok,
        .content_length = content.len,
        .content_type = content_type,
    });
    try httpw.writeBody(content);

    return response_w.written();
}

const response_200 =
    "HTTP/1.1 200 OK\r\n" ++
    "Content-Length: 0\r\n" ++
    "\r\n";

const response_404 =
    "HTTP/1.1 404 Not Found\r\n" ++
    "Content-Length: 0\r\n" ++
    "\r\n";

const response_400 =
    "HTTP/1.1 400 Bad Request\r\n" ++
    "Content-Length: 0\r\n" ++
    "\r\n";

const response_500 =
    "HTTP/1.1 500 Internal Server Error\r\n" ++
    "Content-Length: 0\r\n" ++
    "\r\n";
