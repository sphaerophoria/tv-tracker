const std = @import("std");
const sphtud = @import("sphtud");
const wikipedia = @import("wikipedia.zig");
const Db = @import("Db.zig");
const types = @import("types.zig");

const MovieUpdater = @This();

alloc: std.mem.Allocator,
db: *Db,
base_id: usize,
concurrency: usize,
movies: []const types.Movie,
state: union(enum) {
    query_pages: sphtud.io.SimpleHttpTls,
    resolve: ResolveState,
},

const ResolveState = struct {
    queue: []const Job,
    queue_idx: usize,
    running: []wikipedia.RemoteMovieResolver,
    running_jobs: []Job,
    complete: std.DynamicBitSet,

    fn pop(self: *ResolveState) ?Job {
        if (self.queue_idx >= self.queue.len) return null;
        defer self.queue_idx += 1;
        return self.queue[self.queue_idx];
    }

    fn setComplete(self: *ResolveState, idx: usize) void {
        self.complete.set(idx);
    }

    fn clearComplete(self: *ResolveState, idx: usize) void {
        self.complete.unset(idx);
    }

    fn isComplete(self: *const ResolveState, idx: usize) bool {
        return self.complete.isSet(idx);
    }

    fn queueNextJob(self: *ResolveState, parent: *MovieUpdater, spawner: *sphtud.io.tls.Spawner, idx: usize) !void {
        std.debug.assert(self.isComplete(idx));

        const job = self.pop() orelse return;
        std.log.info("Retrieving movie info for {s}", .{job.title});
        self.running_jobs[idx] = job;
        try self.running[idx].initTitlePinned(
            parent.alloc,
            job.title,
            spawner,
            parent.base_id + idx,
        );
        self.clearComplete(idx);
    }

    fn isDone(self: *const ResolveState) bool {
        return self.complete.count() == self.complete.capacity() and self.queue_idx >= self.queue.len;
    }
};

const Job = struct {
    db_id: usize,
    imdb_id: []const u8,
    title: []const u8,
};

pub fn initPinned(
    self: *MovieUpdater,
    alloc: std.mem.Allocator,
    db: *Db,
    spawner: *sphtud.io.tls.Spawner,
    base_id: usize,
    concurrency: usize,
) !void {
    const movies = try db.getMovies(alloc);
    const to_update = try selectMoviesToUpdate(movies);

    var writer = std.Io.Writer.Allocating.init(alloc);
    try writer.writer.writeAll(
        "SELECT ?movie ?imdbId ?wikiTitle WHERE {" ++ " VALUES ?imdbId {",
    );

    for (to_update) |movie| {
        try writer.writer.print(" \"{s}\"", .{movie.imdb_id});
    }

    try writer.writer.writeAll(
        " }" ++
        " ?movie p:P345 ?stmt ."
        ++ " ?stmt ps:P345 ?imdbId ."
        ++ " ?article schema:about ?movie ;"
        ++ " schema:isPartOf <https://en.wikipedia.org/> ;"
        ++ " schema:name ?wikiTitle ."
        ++ " SERVICE wikibase:label { bd:serviceParam wikibase:language \"en\". }"
        ++ " }",
    );

    var uri_query = std.Io.Writer.Allocating.init(alloc);
    try uri_query.writer.writeAll("query=");
    try sphtud.http.urlencode(writer.written(), &uri_query.writer);
    try uri_query.writer.writeAll("&format=json");

    const uri = std.Uri{
        .scheme = "https",
        .host = .{ .raw = "query.wikidata.org" },
        .path = .{ .percent_encoded = "/sparql" },
        .query = .{ .percent_encoded = uri_query.written() },
    };

    self.* = .{
        .alloc = alloc,
        .db = db,
        .base_id = base_id,
        .concurrency = concurrency,
        .movies = to_update,
        .state = .{ .query_pages = undefined },
    };
    try self.state.query_pages.initPinned(alloc, uri, .{}, spawner, base_id);
}

pub fn isDone(self: *const MovieUpdater) bool {
    return switch (self.state) {
        .query_pages => false,
        .resolve => |*rs| rs.isDone(),
    };
}

pub fn poll(self: *MovieUpdater, spawner: *sphtud.io.tls.Spawner, loop: *sphtud.io.Loop, id: usize) !void {
    switch (self.state) {
        .query_pages => |*req| {
            const result = try req.poll(loop, id) orelse return;

            const parsed = try SparqlResponse.parse(self.alloc, result);
            const jobs = try makeJobQueue(self.alloc, parsed, self.movies);

            req.deinit();

            self.state = .{ .resolve = .{
                .queue = jobs,
                .queue_idx = 0,
                .running = try self.alloc.alloc(wikipedia.RemoteMovieResolver, self.concurrency),
                .running_jobs = try self.alloc.alloc(Job, self.concurrency),
                .complete = try .initFull(self.alloc, self.concurrency),
            } };

            const rs = &self.state.resolve;
            for (0..rs.running.len) |i| {
                try rs.queueNextJob(self, spawner, i);
            }
        },
        .resolve => |*rs| {
            const idx = id - self.base_id;
            if (rs.isComplete(idx)) return;

            if (rs.running[idx].poll(spawner, loop)) |res_opt| blk: {
                const res = res_opt orelse return;
                if (!std.mem.eql(u8, res.imdb_id, rs.running_jobs[idx].imdb_id)) {
                    std.log.err("Parsed imdb id {s} does not match expected {s} for {s}", .{res.imdb_id, rs.running_jobs[idx].imdb_id, rs.running_jobs[idx].title});
                    break :blk;
                }
                const now = try sphtud.io.clock_gettime(.REALTIME);
                _ = self.db.addMovie(res, now) catch |e| {
                    std.log.err("Failed to save movie {s}: {}", .{ rs.running_jobs[idx].title, e });
                };
            } else |e| {
                std.log.err("Failed to resolve movie {s}: {}", .{ rs.running_jobs[idx].title, e });
                if (@errorReturnTrace()) |st| {
                    std.debug.dumpErrorReturnTrace(st);
                }
            }

            rs.running[idx].deinit();
            rs.setComplete(idx);
            try rs.queueNextJob(self, spawner, idx);
        },
    }
}

const SparqlResponse = struct {
    results: struct {
        bindings: []const struct {
            imdbId: BindingValue,
            wikiTitle: BindingValue,
        },
    },

    const BindingValue = struct {
        value: []const u8,
    };

    pub fn parse(alloc: std.mem.Allocator, input: []const u8) !SparqlResponse {
        return try std.json.parseFromSliceLeaky(SparqlResponse, alloc, input, .{ .ignore_unknown_fields = true });
    }
};

fn makeJobQueue(alloc: std.mem.Allocator, response: SparqlResponse, movies: []const types.Movie) ![]const Job {
    var ret: std.ArrayList(Job) = .empty;
    for (response.results.bindings) |binding| {
        const db_id, const imdb_id = for (movies) |m| {
            if (std.mem.eql(u8, m.imdb_id, binding.imdbId.value)) break .{ m.id.inner, m.imdb_id };
        } else continue;

        try ret.append(alloc, .{
            .db_id = @intCast(db_id),
            .imdb_id = imdb_id,
            .title = binding.wikiTitle.value,
        });
    }
    return ret.items;
}

fn selectMoviesToUpdate(movies: []types.Movie) ![]types.Movie {
    var split_idx: usize = 0;

    //* Get movies from database
    //* If last update time is null, update now
    //* if no home release and last update > 1w ago, update now
    //* fill up to 10 movies with oldest N updates
    for (movies) |*movie| {
        const wants_update = blk: {
            const last_update = movie.last_update_time orelse break :blk true;

            if (movie.home_release_date == null) {
                const now = try sphtud.io.clock_gettime(.REALTIME);
                if (last_update.durationTo(now).toSeconds() > std.time.s_per_week) {
                    break :blk true;
                }
            }

            break :blk false;
        };

        if (wants_update) {
            std.mem.swap(types.Movie, &movies[split_idx], movie);
            split_idx += 1;
        }
    }

    std.sort.block(types.Movie, movies[split_idx..], {}, struct {
        fn f(_: void, a: types.Movie, b: types.Movie) bool {
            return a.last_update_time.?.nanoseconds < b.last_update_time.?.nanoseconds;
        }
    }.f);

    const iter_start = @min(split_idx, 10);
    for (iter_start..10) |_| {
        if (split_idx >= movies.len) {
            split_idx = movies.len;
            break;
        }
        std.mem.swap(types.Movie, &movies[split_idx], &movies[split_idx + 1]);
        split_idx += 1;
    }

    return movies[0..split_idx];
}
