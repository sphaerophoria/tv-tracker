const std = @import("std");
const sphtud = @import("sphtud");
const types = @import("../types.zig");
const util = @import("util.zig");

alloc: std.mem.Allocator,
service_id: usize,
page: sphtud.io.SimpleHttpTls,
wikidata: sphtud.io.SimpleHttpTls,

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

pub fn initPinned(self: *@This(), alloc: std.mem.Allocator, page_id: i64, spawner: *sphtud.io.tls.Spawner, service_id: usize) !void {
    const page_query = try std.fmt.allocPrint(alloc, "action=query&prop=revisions&format=json&formatversion=2&pageids={d}&rvslots=*&rvprop=content&rvlimit=1", .{page_id});
    const page_uri = std.Uri{
        .scheme = "https",
        .host = .{ .raw = "en.wikipedia.org" },
        .path = .{ .percent_encoded = "/w/api.php" },
        .query = .{ .percent_encoded = page_query },
    };

    self.* = .{
        .alloc = alloc,
        .service_id = service_id,
        .page = undefined,
        .wikidata = undefined,
        .info = null,
        .imdb_id = null,
        .poster_uri = &.{},
        .page_state = .wait,
        .wikidata_state = .wait_id,
    };
    try self.page.initPinned(alloc, page_uri, .{ .user_agent = util.user_agent }, spawner, service_id);
    errdefer self.page.deinit();

    const wikidata_query = try std.fmt.allocPrint(alloc, "action=query&format=json&prop=pageprops&pageids={d}", .{page_id});

    //https://en.wikipedia.org/w/api.php?action=query&format=json&prop=pageprops&pageids=12345
    const pageprops_url = std.Uri{
        .scheme = "https",
        .host = .{ .raw = "en.wikipedia.org" },
        .path = .{ .percent_encoded = "/w/api.php" },
        .query = .{ .percent_encoded = wikidata_query },
    };

    try self.wikidata.initPinned(alloc, pageprops_url, .{ .user_agent = util.user_agent }, spawner, service_id);
    errdefer self.wikidata.deinit();
}

pub fn deinit(self: *@This()) void {
    self.page.deinit();
    self.wikidata.deinit();
}

pub fn poll(self: *@This(), spawner: *sphtud.io.tls.Spawner, loop: *sphtud.io.Loop) !?types.RemoteMovie {
    const page_opt = try self.page.poll(loop, self.service_id);
    const wikidata_opt = try self.wikidata.poll(loop, self.service_id);

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
            try self.wikidata.initPinned(
                self.alloc,
                data_uri,
                .{ .user_agent = util.user_agent },
                spawner,
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

    if (imdb_ref != .direct) {
        switch (self.wikidata_state) {
            .wait_id => return null,
            .wait_data => {},
        }

        const wikidata = wikidata_opt orelse return null;
        self.imdb_id = .{ .direct = util.imdbFromWikiData(self.alloc, wikidata) orelse return error.InvalidData };
        imdb_ref = self.imdb_id.?;
    }

    switch (imdb_ref) {
        .direct => |id| {
            return .{
                .imdb_id = id,
                .name = info.title,
                .year = @intCast(info.released.year),
                .image = self.poster_uri,
                .theater_release_date = .{ .inner = info.released },
                .home_release_date = null,
            };
        },
        else => return null,
    }
}
