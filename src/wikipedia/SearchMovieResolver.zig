const std = @import("std");
const sphtud = @import("sphtud");
const wikipedia = @import("../wikipedia.zig");
const util = @import("util.zig");

alloc: std.mem.Allocator,
service_id: usize,
page_id: i64,
title: []const u8,
page: sphtud.io.LimitedHttpTls,
timer_service: *sphtud.io.TimerService,
rate_limiter: *sphtud.util.RateLimiter,
state: union(enum) {
    wait_page,
    wait_poster_uri: util.InfoboxFilm,
},

pub const PollRes = union(enum) {
    movie: wikipedia.SearchMovie,
    not_a_movie,
    wait,
};

pub fn initPinned(self: *@This(), alloc: std.mem.Allocator, page_id: i64, title: []const u8, tls_spawner: *sphtud.io.tls.Spawner, timer_service: *sphtud.io.TimerService, rate_limiter: *sphtud.util.RateLimiter, service_id: usize) !void {
    const page_uri = try util.pageUriFromPageId(alloc, page_id);

    self.* = .{
        .alloc = alloc,
        .service_id = service_id,
        .page_id = page_id,
        .title = title,
        .timer_service = timer_service,
        .rate_limiter = rate_limiter,
        .state = .wait_page,
        .page = undefined,
    };

    try self.page.initPinned(alloc, page_uri, .{ .user_agent = util.user_agent }, tls_spawner, timer_service, rate_limiter, service_id);
    errdefer self.page.deinit();
}

pub fn deinit(self: *@This()) void {
    self.page.deinit();
}

pub fn poll(self: *@This(), spawner: *sphtud.io.tls.Spawner, loop: *sphtud.io.Loop) !PollRes {
    const page_body_opt = try self.page.poll(loop, self.service_id);
    const page_body = page_body_opt orelse return .wait;

    switch (self.state) {
        .wait_page => {
            const infobox = util.parseInfoboxFilm(self.title, page_body) catch |e| {
                if (e == error.NotAMovie) {
                    return .not_a_movie;
                }
                return e;
            };

            if (infobox.poster_filename.len == 0) return .not_a_movie;

            const poster_meta_uri = try util.posterMetaUriFromFilename(self.alloc, infobox.poster_filename);
            self.page.deinit();
            try self.page.initPinned(
                self.alloc,
                poster_meta_uri,
                .{ .user_agent = util.user_agent },
                spawner,
                self.timer_service,
                self.rate_limiter,
                self.service_id,
            );
            self.state = .{ .wait_poster_uri = infobox };
            return .wait;
        },
        .wait_poster_uri => |infobox| {
            return .{
                .movie = .{
                    .name = infobox.title,
                    .year = @intCast(infobox.released.year),
                    .wikipedia_page_id = self.page_id,
                    .image = try util.posterUriFromMeta(self.alloc, page_body),
                },
            };
        },
    }
}
