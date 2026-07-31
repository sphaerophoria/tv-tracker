const std = @import("std");
const sphtud = @import("sphtud");
const sys = sphtud.io.system;

dir: c_int,
spawner: *sphtud.io.tls.Spawner,
timer: *sphtud.io.TimerService,
wikipedia_rate_limiter: *sphtud.util.RateLimiter,
unlimited_rate_limiter: sphtud.util.RateLimiter,

const ImageCache = @This();

const user_agent = @import("user_agent.zig").user_agent;

pub fn init(
    dir: c_int,
    spawner: *sphtud.io.tls.Spawner,
    timer: *sphtud.io.TimerService,
    wikipedia_rate_limiter: *sphtud.util.RateLimiter,
) ImageCache {
    return .{
        .dir = dir,
        .spawner = spawner,
        .timer = timer,
        .wikipedia_rate_limiter = wikipedia_rate_limiter,
        .unlimited_rate_limiter = .init(std.math.maxInt(u32), .fromSeconds(1), 1),
    };
}

pub const Get = union(enum) {
    fetching: struct {
        cache_dir: c_int,
        url: []const u8,
        fetcher: *sphtud.io.LimitedHttpTls,
        service_id: usize,
    },
    finished: []const u8,

    pub fn cancel(self: *Get) void {
        switch (self.*) {
            .fetching => |f| f.fetcher.deinit(),
            .finished => {},
        }
    }

    pub fn poll(self: *Get, loop: *sphtud.io.Loop) !?[]const u8 {
        switch (self.*) {
            .fetching => |*params| {
                const data = try params.fetcher.poll(loop, params.service_id) orelse return null;

                writeToCache(params.cache_dir, params.url, data) catch {};

                params.fetcher.deinit();

                self.* = .{ .finished = data };
                return data;
            },
            .finished => |res| return res,
        }
    }
};

fn writeToCache(dir: c_int, url: []const u8, data: []const u8) !void {
    var name_buf: [std.Io.Dir.max_path_bytes]u8 = undefined;
    const filename = try encodeUrl(&name_buf, url);

    const file_fd = try sphtud.io.openat(
        dir,
        filename,
        .{ .ACCMODE = .RDWR, .CREAT = true, .TRUNC = true },
        0o664,
    );
    defer sphtud.io.close(file_fd);

    try sphtud.io.writeAll(data, file_fd);
}

pub fn get(self: *ImageCache, gpa: std.mem.Allocator, url: []const u8, service_id: usize) !Get {
    var name_buf: [std.Io.Dir.max_path_bytes]u8 = undefined;
    const filename = try encodeUrl(&name_buf, url);

    const file_fd = sphtud.io.openat(self.dir, filename, .{}, 0) catch {
        const fetcher = try gpa.create(sphtud.io.LimitedHttpTls);
        const rate_limiter = if (std.mem.indexOf(u8, url, "wikipedia") != null) self.wikipedia_rate_limiter else &self.unlimited_rate_limiter;
        try fetcher.initPinned(gpa, try .parse(url), .{
            .user_agent = user_agent,
        }, self.spawner, self.timer, rate_limiter, service_id);
        return .{
            .fetching = .{
                .cache_dir = self.dir,
                .fetcher = fetcher,
                .service_id = service_id,
                .url = url,
            },
        };
    };
    defer sphtud.io.close(file_fd);

    var file_reader = sphtud.io.Reader.init(file_fd, &.{});
    var content_writer = std.Io.Writer.Allocating.init(gpa);
    _ = try file_reader.interface.streamRemaining(&content_writer.writer);

    return .{ .finished = content_writer.written() };
}

fn encodeUrl(buf: []u8, url: []const u8) ![:0]const u8 {
    const hex = "0123456789ABCDEF";
    var i: usize = 0;
    for (url) |ch| {
        switch (ch) {
            'A'...'Z', 'a'...'z', '0'...'9', '-', '_', '.', '~' => {
                if (i + 1 >= buf.len) return error.PathTooLong;
                buf[i] = ch;
                i += 1;
            },
            else => {
                if (i + 3 >= buf.len) return error.PathTooLong;
                buf[i] = '%';
                buf[i + 1] = hex[ch >> 4];
                buf[i + 2] = hex[ch & 0x0f];
                i += 3;
            },
        }
    }

    if (i >= buf.len) return error.PathTooLong;
    buf[i] = 0;
    return buf[0..i :0];
}
