const std = @import("std");
const sphtud = @import("sphtud");
const Db = @import("Db.zig");
const tv_maze = @import("tv_maze.zig");
const types = @import("types.zig");

alloc: *sphtud.alloc.Sphalloc,
base_id: usize,
db: *Db,
spawner: *sphtud.io.tls.Spawner,
state: union(enum) {
    empty,
    updating: Updating,
},

const PeriodicUpdater = @This();

pub const concurrency = 8;

const Job = struct {
    id: types.ShowId,
    name: []const u8,
    remote_id: types.TvMazeShowId,
};

const Updating = struct {
    queue: std.ArrayList(Job),
    in_flight: []tv_maze.Episodes,
    in_flight_show: []types.ShowId,
    available: std.DynamicBitSet,
};

pub fn init(parent_alloc: *sphtud.alloc.Sphalloc, db: *Db, spawner: *sphtud.io.tls.Spawner, base_id: usize) !PeriodicUpdater {
    const alloc = try parent_alloc.makeSubAlloc("updater");
    return .{
        .alloc = alloc,
        .base_id = base_id,
        .db = db,
        .spawner = spawner,
        .state = .empty,
    };
}

pub fn trigger(self: *PeriodicUpdater) !void {
    switch (self.state) {
        .empty => {
            const gpa = self.alloc.general();

            const now = try sphtud.io.clock_gettime(.REALTIME);
            const shows = try self.db.getShows(gpa, now);

            // Nothing to update: stay empty rather than entering a run that
            // would never receive an event to complete it.
            if (shows.len == 0) return;

            var u: Updating = .{
                .queue = .empty,
                .in_flight = try gpa.alloc(tv_maze.Episodes, concurrency),
                .in_flight_show = try gpa.alloc(types.ShowId, concurrency),
                .available = try std.DynamicBitSet.initFull(gpa, concurrency),
            };

            for (shows) |show| {
                try u.queue.append(gpa, .{ .id = show.id, .remote_id = show.remote_id, .name = show.name });
            }

            for (0..concurrency) |i| {
                if (u.queue.pop()) |job| {
                    try self.startFetch(&u, job, i);
                }
            }

            self.state = .{ .updating = u };
        },
        // Already running, no need to immediately re-run
        .updating => {},
    }
}

pub fn poll(self: *PeriodicUpdater, loop: *sphtud.io.Loop, id: usize) !void {
    const u = switch (self.state) {
        .empty => return,
        .updating => |*u| u,
    };

    const idx = id - self.base_id;

    // Avoid double update on spurious event
    if (u.available.isSet(idx)) return;

    const show_id = u.in_flight_show[idx];

    const episodes_opt = u.in_flight[idx].poll(loop) catch |e| blk: {
        std.log.err("Failed to fetch episodes for show {d}: {t}", .{ show_id.inner, e });
        break :blk &.{};
    };
    const episodes = episodes_opt orelse return;

    for (episodes) |episode| {
        _ = try self.db.addEpisode(show_id, episode);
    }

    // All done. Mark available and queue a new one if there's more work to do
    u.in_flight[idx].deinit();
    u.available.set(idx);

    if (u.queue.pop()) |next| {
        try self.startFetch(u, next, idx);
    }

    if (u.available.count() == u.in_flight.len) {
        std.log.info("Finished DB update", .{});
        try self.alloc.reset();
        self.state = .empty;
    }
}

fn startFetch(self: *PeriodicUpdater, u: *Updating, job: Job, idx: usize) !void {
    u.in_flight_show[idx] = job.id;
    std.log.info("Fetching info for show {d} ({s})", .{ job.id.inner, job.name });

    try u.in_flight[idx].initPinned(
        self.alloc.general(),
        job.remote_id,
        self.spawner,
        self.base_id + idx,
    );
    u.available.unset(idx);
}
