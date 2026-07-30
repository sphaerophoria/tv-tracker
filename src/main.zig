const std = @import("std");
const sphtud = @import("sphtud");
const sys = sphtud.io.system;
const Server = @import("Server.zig");
const types = @import("types.zig");
const Db = @import("Db.zig");
const ImageCache = @import("ImageCache.zig");
const PeriodicUpdater = @import("PeriodicUpdater.zig");

const Ids = struct {
    runtime: sphtud.io.Runtime.Ids,
    server: Server.Ids,
    rescan: usize,
    tv_updater: sphtud.io.IdAlloc.Range,
    movie_updater: sphtud.io.IdAlloc.Range,
    signal: usize,

    fn init() Ids {
        var alloc = sphtud.io.IdAlloc.init;
        return .{
            .runtime = .init(&alloc),
            .server = .init(&alloc),
            .rescan = alloc.allocOne(),
            .tv_updater = alloc.allocMany(PeriodicUpdater.concurrency),
            .movie_updater = alloc.allocMany(8),
            .signal = alloc.allocOne(),
        };
    }
};

const ids = Ids.init();

const Args = struct {
    html_path: [:0]const u8,
    port: u16,
    db_path: [:0]const u8,
    cache_path: [:0]const u8,
    poll_indexers: bool,

    fn parse(args: std.process.Args) Args {
        var it = args.iterate();

        const process_name = it.next() orelse "tv-tracker";

        var html_path: ?[:0]const u8 = null;
        var db_path: ?[:0]const u8 = null;
        var port: ?u16 = null;
        var cache_path: ?[:0]const u8 = null;
        var poll_indexers = true;

        const Switch = enum {
            @"--help",
            @"--cache-path",
            @"--html-path",
            @"--db-path",
            @"--port",
            @"--no-poll",
        };

        while (it.next()) |arg| {
            const s = std.meta.stringToEnum(Switch, arg) orelse {
                std.debug.print("Unknown arg {s}\n", .{arg});
                help(process_name);
            };
            switch (s) {
                .@"--help" => help(process_name),
                .@"--cache-path" => cache_path = it.next() orelse {
                    std.debug.print("Missing cache path\n", .{});
                    help(process_name);
                },
                .@"--html-path" => html_path = it.next() orelse {
                    std.debug.print("Missing html path\n", .{});
                    help(process_name);
                },
                .@"--db-path" => db_path = it.next() orelse {
                    std.debug.print("Missing db path\n", .{});
                    help(process_name);
                },
                .@"--port" => {
                    const port_s = it.next() orelse {
                        std.debug.print("Missing port\n", .{});
                        help(process_name);
                    };
                    port = std.fmt.parseInt(u16, port_s, 10) catch {
                        std.debug.print("Invalid port\n", .{});
                        help(process_name);
                    };
                },
                .@"--no-poll" => poll_indexers = false,
            }
        }

        return .{
            .html_path = html_path orelse {
                std.debug.print("Missing html path\n", .{});
                help(process_name);
            },
            .port = port orelse {
                std.debug.print("Missing port\n", .{});
                help(process_name);
            },
            .db_path = db_path orelse {
                std.debug.print("Missing db path\n", .{});
                help(process_name);
            },
            .cache_path = cache_path orelse {
                std.debug.print("Missing cache path\n", .{});
                help(process_name);
            },
            .poll_indexers = poll_indexers,
        };
    }

    fn help(process_name: []const u8) noreturn {
        std.debug.print(
            \\Track your tv watching
            \\
            \\Usage: {s} [ARGS]
            \\
            \\Args:
            \\--help: Show this help
            \\--cache-path: Where to cache assets retrieved from remote
            \\--html-path: Optional path to filesystem to serve html files from. Useful for debugging
            \\--db-path: Where to store database
            \\--port: Port to serve UI on
            \\--no-poll: Optional, when passed will not poll remote indexers for new data
            \\
        , .{process_name});

        std.process.exit(1);
    }
};

pub fn main(init: std.process.Init.Minimal) !void {
    var tpa: sphtud.alloc.TinyPageAllocator = undefined;
    try tpa.initPinned();

    var alloc: sphtud.alloc.Sphalloc = undefined;
    try alloc.initPinned(tpa.allocator(), "root");

    var io: sphtud.io.Runtime = undefined;
    try io.initPinned(try alloc.makeSubAlloc("io"), ids.runtime);

    const args = Args.parse(init.args);

    const resources = try sphtud.io.open(args.html_path, .{ .DIRECTORY = true }, 0);
    defer sphtud.io.close(resources);

    const cache_dir = try sphtud.io.open(args.cache_path, .{ .DIRECTORY = true }, 0);
    defer sphtud.io.close(cache_dir);
    var image_cache = ImageCache.init(cache_dir, &io.tls_spawner);

    var db = try Db.init(args.db_path);
    defer db.deinit();

    var server = try Server.init(&alloc, args.port, resources, &db, &image_cache, &io.tls_spawner, &io.loop, ids.server);
    defer server.deinit();

    var tv_updater = try PeriodicUpdater.init(&alloc, &db, &io.tls_spawner, ids.tv_updater.start);
    if (args.poll_indexers) try tv_updater.trigger();

    const timer = try sphtud.io.timerfd_create(.BOOTTIME);
    try io.loop.register(.{
        .handle = timer,
        .id = ids.rescan,
        .read = true,
        .write = false,
    });
    const timer_duration: std.Io.Duration = .fromSeconds(std.time.s_per_day);
    try sphtud.io.timerfd_settime(timer, .{ .rel = timer_duration }, timer_duration);

    var sig_mask = sys.sigemptyset();
    sys.sigaddset(&sig_mask, sys.SIG.INT);
    sys.sigaddset(&sig_mask, sys.SIG.PIPE);
    _ = sys.sigprocmask(sys.SIG.BLOCK, &sig_mask, null);
    sys.sigdelset(&sig_mask, sys.SIG.PIPE);

    const signal_fd = try sphtud.io.signalfd(&sig_mask);
    try io.loop.register(.{
        .handle = signal_fd,
        .id = ids.signal,
        .read = true,
        .write = false,
    });

    std.log.info("Server running at http://0.0.0.0:{d}", .{args.port});

    while (true) {
        const event = try io.service(ids.runtime);
        switch (event) {
            ids.server.total.start...ids.server.total.end => {
                try server.service(event, ids.server);
            },
            ids.rescan => {
                var val: u64 = 0;
                _ = try sphtud.io.read(timer, std.mem.asBytes(&val));

                std.log.info("Updating episodes database", .{});
                try tv_updater.trigger();
            },
            ids.tv_updater.start...ids.tv_updater.end => {
                try tv_updater.poll(&io.loop, event);
            },
            ids.signal => {
                std.log.info("Caught sigint, exiting\n", .{});
                break;
            },
            else => unreachable,
        }
    }
}

test {
    std.testing.refAllDecls(@This());
}
