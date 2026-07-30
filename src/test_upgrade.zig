const std = @import("std");
const sphtud = @import("sphtud");
const Db = @import("Db.zig");
const types = @import("types.zig");
const wikipedia = @import("wikipedia.zig");

const Ids = struct {
    movie_updater: sphtud.util.IdAlloc.Range,
    runtime: sphtud.io.Runtime.Ids,

    pub fn init() Ids {
        var alloc = sphtud.util.IdAlloc.init;
        return .{
            .movie_updater = alloc.allocMany(8),
            .runtime = .init(&alloc),
        };
    }
};

const ids = Ids.init();

const MovieUpdater = struct {
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
        complete: u8,

        fn pop(self: *ResolveState) ?Job {
            if (self.queue_idx >= self.queue.len) return null;
            defer self.queue_idx += 1;
            return self.queue[self.queue_idx];
        }

        fn setComplete(self: *ResolveState, idx: usize) void {
            self.complete |= @as(u8, 1) << @as(u3, @intCast(idx));
        }

        fn clearComplete(self: *ResolveState, idx: usize) void {
            self.complete &= ~(@as(u8, 1) << @as(u3, @intCast(idx)));
        }

        fn isComplete(self: *const ResolveState, idx: usize) bool {
            const val: u1 = @truncate(self.complete >> @as(u3, @intCast(idx)));
            return val == 1;
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
            return self.complete == 0xFF and self.queue_idx >= self.queue.len;
        }
    };

    const Job = struct {
        db_id: usize,
        title: []const u8,
    };

    pub fn initPinned(
        self: *MovieUpdater,
        alloc: std.mem.Allocator,
        db: *Db,
        movies: []const types.Movie,
        spawner: *sphtud.io.tls.Spawner,
        base_id: usize,
        concurrency: usize,
    ) !void {
        var writer = std.Io.Writer.Allocating.init(alloc);
        try writer.writer.writeAll(
            "SELECT ?movie ?imdbId ?wikiTitle WHERE {" ++ " VALUES ?imdbId {",
        );

        for (movies) |movie| {
            try writer.writer.print(" \"{s}\"", .{movie.imdb_id});
        }

        try writer.writer.writeAll(
            " }" ++ " ?movie p:P345 ?stmt ." ++ " ?stmt ps:P345 ?imdbId ." ++ " ?article schema:about ?movie ;" ++ " schema:isPartOf <https://en.wikipedia.org/> ;" ++ " schema:name ?wikiTitle ." ++ " SERVICE wikibase:label { bd:serviceParam wikibase:language \"en\". }" ++ " }",
        );

        var uri_query = std.Io.Writer.Allocating.init(alloc);
        try uri_query.writer.writeAll("query=");
        try std.Uri.Component.formatQuery(.{ .raw = writer.written() }, &uri_query.writer);
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
            .movies = movies,
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
                    .complete = 0xFF,
                } };

                const rs = &self.state.resolve;
                for (0..rs.running.len) |i| {
                    try rs.queueNextJob(self, spawner, i);
                }
            },
            .resolve => |*rs| {
                const idx = id - self.base_id;
                if (rs.isComplete(idx)) return;

                if (rs.running[idx].poll(spawner, loop)) |res_opt| {
                    const res = res_opt orelse return;
                    _ = self.db.addMovie(res) catch |e| {
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
            const db_id = for (movies) |m| {
                if (std.mem.eql(u8, m.imdb_id, binding.imdbId.value)) break m.id.inner;
            } else continue;

            try ret.append(alloc, .{
                .db_id = @intCast(db_id),
                .title = binding.wikiTitle.value,
            });
        }
        return ret.items;
    }
};

pub fn main(init: std.process.Init.Minimal) !void {
    var tpa: sphtud.alloc.TinyPageAllocator = undefined;
    try tpa.initPinned();

    var root_alloc: sphtud.alloc.Sphalloc = undefined;
    try root_alloc.initPinned(tpa.allocator(), "root");

    var runtime: sphtud.io.Runtime = undefined;
    try runtime.initPinned(&root_alloc, ids.runtime);

    var args = init.args.iterate();
    _ = args.next();
    const db_path = args.next() orelse return error.NoDbPath;

    var db = try Db.init(db_path);
    const movies = try db.getMovies(root_alloc.general());
    std.debug.print("movies len: {d}\n", .{movies.len});

    var updater: MovieUpdater = undefined;
    try updater.initPinned(
        root_alloc.general(),
        &db,
        movies,
        &runtime.tls_spawner,
        ids.movie_updater.start,
        ids.movie_updater.end - ids.movie_updater.start + 1,
    );

    while (!updater.isDone()) {
        const id = try runtime.service(ids.runtime);

        if (ids.movie_updater.contains(id)) {
            try updater.poll(&runtime.tls_spawner, &runtime.loop, id);
        }
    }

    std.log.info("movie update complete", .{});
}
