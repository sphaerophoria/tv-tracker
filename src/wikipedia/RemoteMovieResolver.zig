const std = @import("std");
const sphtud = @import("sphtud");
const types = @import("../types.zig");
const util = @import("util.zig");
const wikipedia = @import("../wikipedia.zig");

alloc: std.mem.Allocator,
service_id: usize,
page: sphtud.io.LimitedHttpTls,
wikidata: sphtud.io.LimitedHttpTls,
timer_service: *sphtud.io.TimerService,
rate_limiter: *sphtud.util.RateLimiter,

info: ?util.InfoboxFilm,
imdb_id: ?util.ImdbRef,
poster_uri: []const u8,

page_state: enum {
    wait,
    poster_uri,
    done,
},
wikidata_state: enum {
    wait_id,
    wait_data,
},

const page_query_base = "action=query&prop=revisions&format=json&formatversion=2&rvslots=*&rvprop=content&rvlimit=1";
const wikidata_query_base = "action=query&format=json&prop=pageprops";

pub fn initTitlePinned(self: *@This(), alloc: std.mem.Allocator, page_title: []const u8, spawner: *sphtud.io.tls.Spawner, timer_service: *sphtud.io.TimerService, rate_limiter: *sphtud.util.RateLimiter, service_id: usize) !void {
    var page_query = std.Io.Writer.Allocating.init(alloc);
    try page_query.writer.writeAll(page_query_base ++ "&titles=");
    try sphtud.http.urlencode(page_title, &page_query.writer);

    const page_uri = wikipedia.makeApiQuery(page_query.written());

    var wikidata_query = std.Io.Writer.Allocating.init(alloc);
    try wikidata_query.writer.writeAll(wikidata_query_base ++ "&titles=");
    try sphtud.http.urlencode(page_title, &wikidata_query.writer);

    const pageprops_url = wikipedia.makeApiQuery(wikidata_query.written());

    try self.initImpl(alloc, page_uri, pageprops_url, spawner, timer_service, rate_limiter, service_id);
}

pub fn initPinned(self: *@This(), alloc: std.mem.Allocator, page_id: i64, spawner: *sphtud.io.tls.Spawner, timer_service: *sphtud.io.TimerService, rate_limiter: *sphtud.util.RateLimiter, service_id: usize) !void {
    const page_query = try std.fmt.allocPrint(alloc, page_query_base ++ "&pageids={d}", .{page_id});
    const page_uri = wikipedia.makeApiQuery(page_query);

    const wikidata_query = try std.fmt.allocPrint(alloc, wikidata_query_base ++ "&pageids={d}", .{page_id});
    const wikidata_uri = wikipedia.makeApiQuery(wikidata_query);

    try self.initImpl(alloc, page_uri, wikidata_uri, spawner, timer_service, rate_limiter, service_id);
}

fn initImpl(self: *@This(), alloc: std.mem.Allocator, page_uri: std.Uri, pageprops_uri: std.Uri, spawner: *sphtud.io.tls.Spawner, timer_service: *sphtud.io.TimerService, rate_limiter: *sphtud.util.RateLimiter, service_id: usize) !void {
    self.* = .{
        .alloc = alloc,
        .service_id = service_id,
        .page = undefined,
        .wikidata = undefined,
        .timer_service = timer_service,
        .rate_limiter = rate_limiter,
        .info = null,
        .imdb_id = null,
        .poster_uri = &.{},
        .page_state = .wait,
        .wikidata_state = .wait_id,
    };
    try self.page.initPinned(alloc, page_uri, .{ .user_agent = util.user_agent }, spawner, timer_service, rate_limiter, service_id);
    errdefer self.page.deinit();

    try self.wikidata.initPinned(alloc, pageprops_uri, .{ .user_agent = util.user_agent }, spawner, timer_service, rate_limiter, service_id);
    errdefer self.wikidata.deinit();
}

pub fn deinit(self: *@This()) void {
    self.page.deinit();
    self.wikidata.deinit();
}

pub fn poll(self: *@This(), spawner: *sphtud.io.tls.Spawner, loop: *sphtud.io.Loop) !?types.RemoteMovie {
    const page_opt = try self.page.poll(loop, self.service_id);
    var wikidata_opt = try self.wikidata.poll(loop, self.service_id);

    switch (self.wikidata_state) {
        .wait_id => blk: {
            const wikibase_id = try util.parseWikibaseIdFromPageProps(self.alloc, wikidata_opt orelse break :blk);

            //https://www.wikidata.org/wiki/Special:EntityData/Q14650496.json
            const next_path = try std.fmt.allocPrint(self.alloc, "/wiki/Special:EntityData/{s}.json", .{wikibase_id});

            const data_uri = std.Uri{
                .scheme = "https",
                .host = .{ .raw = "www.wikidata.org" },
                .path = .{ .percent_encoded = next_path },
            };
            self.wikidata.deinit();
            wikidata_opt = null;
            try self.wikidata.initPinned(
                self.alloc,
                data_uri,
                .{ .user_agent = util.user_agent },
                spawner,
                self.timer_service,
                self.rate_limiter,
                self.service_id,
            );

            self.wikidata_state = .wait_data;
        },
        // Only matters when imdb ref is not direct, no work to do
        .wait_data => {},
    }

    switch (self.page_state) {
        .wait => blk: {
            const Parse = struct {
                query: struct {
                    pages: []const struct {
                        title: []const u8,
                        revisions: []const struct {
                            slots: struct {
                                main: struct {
                                    content: []const u8,
                                },
                            },
                        },
                    },
                },
            };

            const parsed = try std.json.parseFromSliceLeaky(Parse, self.alloc, page_opt orelse break :blk, .{ .ignore_unknown_fields = true });

            if (parsed.query.pages.len == 0) return error.InvalidResposne;

            const title = parsed.query.pages[0].title;
            if (parsed.query.pages[0].revisions.len == 0) return error.InvalidResponse;

            const source = parsed.query.pages[0].revisions[0].slots.main.content;
            self.info = try util.parseInfoboxFilm(title, source);
            self.imdb_id = try util.parseImdbId(title, source);

            self.page.deinit();
            try self.page.initPinned(
                self.alloc,
                try util.posterMetaUriFromFilename(self.alloc, self.info.?.poster_filename),
                .{ .user_agent = util.user_agent },
                spawner,
                self.timer_service,
                self.rate_limiter,
                self.service_id,
            );
            self.page_state = .poster_uri;
        },
        .poster_uri => blk: {
            const data = page_opt orelse break :blk;
            self.poster_uri = try util.posterUriFromMeta(self.alloc, data);
            self.page_state = .done;
        },
        .done => {},
    }

    var imdb_ref = self.imdb_id orelse return null;
    const info = self.info orelse return null;
    if (self.poster_uri.len == 0) return null;

    switch (imdb_ref) {
        .direct, .direct_no_prefix => {},
        .wikidata_name, .wikidata_id => {
            switch (self.wikidata_state) {
                .wait_id => return null,
                .wait_data => {},
            }

            const wikidata = wikidata_opt orelse return null;
            self.imdb_id = .{ .direct = util.imdbFromWikiData(self.alloc, wikidata) orelse return error.InvalidData };
            imdb_ref = self.imdb_id.?;
        },
    }

    const imdb_id: ?[]const u8 = switch (imdb_ref) {
        .direct => |id| id,
        .direct_no_prefix => |id| blk: {
            const zero_pad = 7 -| id.len;
            const zeros: [7]u8 = @splat('0');

            break :blk try std.fmt.allocPrint(self.alloc, "tt{s}{s}", .{ zeros[0..zero_pad], id });
        },
        else => null,
    };

    if (imdb_id) |id| {
        return .{
            .imdb_id = id,
            .name = info.title,
            .year = @intCast(info.released.year),
            .image = self.poster_uri,
            .theater_release_date = .{ .inner = info.released },
            .home_release_date = null,
        };
    }

    return null;
}
