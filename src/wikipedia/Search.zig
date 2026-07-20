const std = @import("std");
const sphtud = @import("sphtud");
const util = @import("util.zig");
const wikipedia = @import("../wikipedia.zig");
const SearchMovieResolver = @import("SearchMovieResolver.zig");

alloc: std.mem.Allocator,
base_id: usize,
concurrency: usize,
state: union(enum) {
    wait_search_res: sphtud.io.SimpleHttpTls,
    wait_movies: WaitMovies,
    finished: []const wikipedia.SearchMovie,
},

const WaitMovies = struct {
    queue: std.ArrayList(Job),
    in_flight: []SearchMovieResolver,
    in_flight_names: [][]const u8,
    complete_mask: std.DynamicBitSet,
    results: std.ArrayList(wikipedia.SearchMovie),

    fn finishItem(self: *WaitMovies, idx: usize) void {
        self.in_flight[idx].deinit();
        self.complete_mask.set(idx);
    }

    fn deinit(self: *WaitMovies) void {
        var it = self.complete_mask.iterator(.{
            .kind = .unset,
        });
        while (it.next()) |idx| {
            self.in_flight[idx].deinit();
        }
    }
};

const Job = struct {
    title: []const u8,
    id: i64,
};

pub fn initPinned(self: *@This(), alloc: std.mem.Allocator, query_percent_coded: []const u8, tls_spawner: *sphtud.io.tls.Spawner, base_id: usize, concurrency: usize) !void {
    if (concurrency < 2) return error.ConcurrencyTooLow;

    const query_string = try std.fmt.allocPrint(alloc, "q={s}&limit=10", .{query_percent_coded});
    const uri = std.Uri{
        .scheme = "https",
        .host = .{ .raw = "en.wikipedia.org" },
        .path = .{ .percent_encoded = "/w/rest.php/v1/search/page" },
        .query = .{ .percent_encoded = query_string },
    };

    self.alloc = alloc;
    self.base_id = base_id;
    self.concurrency = concurrency;
    self.state = .{ .wait_search_res = undefined };
    try self.state.wait_search_res.initPinned(
        alloc,
        uri,
        .{ .user_agent = util.user_agent },
        tls_spawner,
        base_id,
    );
}

pub fn deinit(self: *@This()) void {
    switch (self.state) {
        .wait_search_res => |*f| f.deinit(),
        .wait_movies => |*wm| wm.deinit(),
        .finished => {},
    }
}

pub fn result(self: *@This()) ?[]const wikipedia.SearchMovie {
    switch (self.state) {
        .finished => |r| return r,
        else => return null,
    }
}

pub fn poll(self: *@This(), tls_spawner: *sphtud.io.tls.Spawner, loop: *sphtud.io.Loop, id: usize) !void {
    sw: switch (self.state) {
        .wait_search_res => |*f| {
            const body = try f.poll(loop, self.base_id) orelse return;

            const SearchResult = struct {
                pages: []const struct {
                    id: i64,
                    title: []const u8,
                },
            };
            const parsed = try std.json.parseFromSliceLeaky(SearchResult, self.alloc, body, .{ .ignore_unknown_fields = true });

            var queue = std.ArrayList(Job).empty;
            for (parsed.pages) |page| {
                try queue.append(self.alloc, .{ .id = page.id, .title = page.title });
            }

            var wm = WaitMovies{
                .queue = queue,
                .in_flight = try self.alloc.alloc(SearchMovieResolver, self.concurrency),
                .in_flight_names = try self.alloc.alloc([]const u8, self.concurrency),
                .complete_mask = try std.DynamicBitSet.initFull(self.alloc, self.concurrency),
                .results = .empty,
            };
            errdefer wm.deinit();

            for (0..wm.in_flight.len) |idx| {
                try self.queueMovieResolution(&wm, idx, tls_spawner);
            }

            f.deinit();
            self.state = .{ .wait_movies = wm };
            continue :sw self.state;
        },
        .wait_movies => |*ws| {
            const idx = id - self.base_id;

            if (ws.complete_mask.isSet(idx)) return;

            if (ws.in_flight[idx].poll(tls_spawner, loop)) |res| switch (res) {
                .movie => |movie| {
                    std.log.info("{s} was a movie", .{ws.in_flight_names[idx]});
                    try ws.results.append(self.alloc, movie);
                },
                .not_a_movie => {
                    std.log.info("{s} not a movie", .{ws.in_flight_names[idx]});
                },
                .wait => return,
            } else |e| {
                std.log.err("failed to fetch movie info for {s} {t}", .{ ws.in_flight_names[idx], e });
            }

            ws.finishItem(idx);
            try self.queueMovieResolution(ws, idx, tls_spawner);
        },
        .finished => {},
    }
}

fn queueMovieResolution(self: *@This(), ws: *WaitMovies, idx: usize, tls_spawner: *sphtud.io.tls.Spawner) !void {
    const job = ws.queue.pop() orelse {
        if (ws.complete_mask.count() == self.concurrency) {
            self.state = .{
                .finished = ws.results.items,
            };
        }
        return;
    };

    ws.in_flight_names[idx] = job.title;
    ws.in_flight[idx] = undefined; // Maybe unnecessary, but defensive programming
    try ws.in_flight[idx].initPinned(
        self.alloc,
        job.id,
        job.title,
        tls_spawner,
        self.base_id + idx,
    );
    ws.complete_mask.setValue(idx, false);
}
