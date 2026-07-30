const std = @import("std");
const sphtud = @import("sphtud");
const TemplateIterator = @import("TemplateIterator.zig");

pub const user_agent = "SphaeroBot2/0.0 (https://twitch.tv/sphaerophoria) sphtud/0.0.1";

pub const ImdbRef = union(enum) {
    wikidata_name: []const u8,
    wikidata_id: []const u8,
    direct: []const u8,
    direct_no_prefix: []const u8,
};

pub const InfoboxFilm = struct {
    title: []const u8,
    released: sphtud.datetime.Date,
    poster_filename: []const u8,
};

pub const default_release_date: sphtud.datetime.Date = .{ .year = 1, .day = 1, .month = .jan };

pub fn pageNameBase(title: []const u8) []const u8 {
    const end = std.mem.lastIndexOfScalar(u8, title, '(') orelse title.len;
    return std.mem.trim(u8, title[0..end], &std.ascii.whitespace);
}

pub fn extractReleaseDate(released: []const u8) sphtud.datetime.Date {
    const date_keys: []const []const u8 = &.{ "{{Film date", "{{film date" };

    const idx: usize = blk: for (date_keys) |k| {
        break :blk std.mem.indexOf(u8, released, k) orelse continue;
    } else {
        return default_release_date;
    };

    var it = TemplateIterator.init(released[idx..]) catch return default_release_date;
    _ = it.next() catch return default_release_date;

    var components: [3]?[]const u8 = @splat(null);
    var ranges_idx: u8 = 0;

    while (ranges_idx < 3) {
        const param = (it.next() catch return default_release_date) orelse break;
        if (std.mem.indexOfScalar(u8, param, '=') != null) {
            continue;
        }

        components[ranges_idx] = param;
        ranges_idx += 1;
    }

    const year = if (components[0]) |year_s| std.fmt.parseInt(u16, year_s, 10) catch 1 else 1;
    const month_raw = if (components[1]) |month_s| std.fmt.parseInt(u8, month_s, 10) catch 0 else 0;
    const month: u8 = if (month_raw >= 1 and month_raw <= 12) month_raw else 1;
    const day = if (components[2]) |day_s| std.fmt.parseInt(u8, day_s, 10) catch 1 else 1;
    return .{
        .year = year,
        .month = @enumFromInt(month - 1),
        .day = day,
    };
}

pub fn parseInfoboxFilm(title: []const u8, page_source: []const u8) !InfoboxFilm {
    const infobox_key = "{{Infobox film";
    const idx = std.mem.indexOf(u8, page_source, infobox_key) orelse return error.NotAMovie;

    var ret = InfoboxFilm{
        .title = pageNameBase(title),
        .released = default_release_date,
        .poster_filename = "",
    };

    var it = try TemplateIterator.init(page_source[idx..]);
    _ = try it.next();

    while (try it.next()) |param| {
        var key, var value = std.mem.cutScalar(u8, param, '=') orelse return error.Invalid;
        key = std.mem.trim(u8, key, &std.ascii.whitespace);
        value = std.mem.trim(u8, value, &std.ascii.whitespace);

        if (std.mem.eql(u8, key, "name")) {
            ret.title = value;
        } else if (std.mem.eql(u8, key, "released")) {
            ret.released = extractReleaseDate(value);
        } else if (std.mem.eql(u8, key, "image")) {
            ret.poster_filename = value;
        }
    }

    return ret;
}

pub fn parseImdbId(movie_title: []const u8, page_source: []const u8) !ImdbRef {
    //https://en.wikipedia.org/wiki/Template:IMDb_title
    var best_ref: ImdbRef = .{ .wikidata_name = movie_title };
    var best_similarity: usize = std.math.maxInt(usize);

    var key_pos: usize = 0;

    var alloc_buf: [4096]u8 = undefined;
    var scratch = sphtud.alloc.BufAllocator.init(&alloc_buf);

    outer: while (true) {
        scratch.reset();
        const template_key = "{{IMDb title";
        key_pos = std.mem.indexOfPos(u8, page_source, key_pos, template_key) orelse break;
        key_pos += template_key.len;
        const template_end = std.mem.indexOfPos(u8, page_source, key_pos, "}}") orelse break;

        var arg_it = std.mem.tokenizeScalar(u8, page_source[key_pos..template_end], '|');
        const first = arg_it.next() orelse continue;

        const using_named = std.mem.indexOfScalar(u8, first, '=') != null;

        const id, const title, const qid = if (using_named) blk: {
            arg_it.index = 0;
            var id: []const u8 = "";
            var title: []const u8 = "";
            var qid: []const u8 = "";

            while (arg_it.next()) |kv| {
                const key, const val = std.mem.cut(u8, kv, "=") orelse continue :outer;
                if (std.mem.eql(u8, key, "id")) {
                    id = val;
                } else if (std.mem.eql(u8, key, "title")) {
                    title = val;
                } else if (std.mem.eql(u8, key, "qid")) {
                    qid = val;
                }
            }

            break :blk .{ id, title, qid };
        } else blk: {
            break :blk .{ first, arg_it.next() orelse "", "" };
        };

        const ref: ImdbRef = if (id.len == 0) ref: {
            if (qid.len != 0) {
                break :ref .{ .wikidata_id = qid };
            } else if (title.len != 0) {
                break :ref .{ .wikidata_name = title };
            } else {
                break :ref .{ .wikidata_name = movie_title };
            }
        } else if (std.mem.startsWith(u8, id, "tt")) .{ .direct = id } else .{ .direct_no_prefix = id };

        const similarity = try sphtud.diff.levenshteinDistance(scratch.allocator(), movie_title, title);

        if (similarity < best_similarity) {
            best_ref = ref;
            best_similarity = similarity;
        }
    }

    return best_ref;
}

pub fn imdbFromWikiData(alloc: std.mem.Allocator, body: []const u8) ?[]const u8 {
    var parsed = std.json.parseFromSliceLeaky(std.json.Value, alloc, body, .{}) catch return null;
    if (parsed != .object) return null;
    const entities = parsed.object.get("entities") orelse return null;
    if (entities != .object) return null;
    var entity_it = entities.object.iterator();
    const entity = entity_it.next() orelse return null;
    if (entity.value_ptr.* != .object) return null;
    const claims = entity.value_ptr.object.get("claims") orelse return null;
    if (claims != .object) return null;
    const imdb_prop = claims.object.get("P345") orelse return null;

    const imdb_prop_parsed = std.json.parseFromValueLeaky([]const struct {
        mainsnak: struct {
            datavalue: struct {
                value: []const u8,
            },
        },
    }, alloc, imdb_prop, .{ .ignore_unknown_fields = true }) catch return null;
    if (imdb_prop_parsed.len == 0) return null;
    return imdb_prop_parsed[0].mainsnak.datavalue.value;
}

pub fn parseWikibaseIdFromPageProps(alloc: std.mem.Allocator, body: []const u8) ![]const u8 {
    const Outer = struct {
        query: struct {
            pages: std.json.Value,
        },
    };

    const outer = try std.json.parseFromSliceLeaky(Outer, alloc, body, .{ .ignore_unknown_fields = true });
    if (outer.query.pages != .object) return error.InvalidResponse;

    var it = outer.query.pages.object.iterator();
    const inner_value = it.next() orelse return error.InvalidResponse;

    const Inner = struct {
        pageprops: struct {
            wikibase_item: []const u8,
        },
    };

    const inner = try std.json.parseFromValueLeaky(Inner, alloc, inner_value.value_ptr.*, .{ .ignore_unknown_fields = true });
    return inner.pageprops.wikibase_item;
}

pub fn pageUriFromPageId(alloc: std.mem.Allocator, page_id: i64) !std.Uri {
    const page_query = try std.fmt.allocPrint(alloc, "curid={d}&action=raw", .{page_id});

    return std.Uri{
        .scheme = "https",
        .host = .{ .raw = "en.wikipedia.org" },
        .path = .{ .percent_encoded = "/w/index.php" },
        .query = .{ .percent_encoded = page_query },
    };
}

pub fn posterMetaUriFromFilename(alloc: std.mem.Allocator, filename: []const u8) !std.Uri {
    const path = try std.fmt.allocPrint(alloc, "/w/rest.php/v1/file/{s}", .{filename});
    return std.Uri{
        .scheme = "https",
        .host = .{ .raw = "en.wikipedia.org" },
        .path = .{ .raw = path },
    };
}

pub fn posterUriFromMeta(alloc: std.mem.Allocator, body: []const u8) ![]const u8 {
    const parsed = try std.json.parseFromSliceLeaky(struct {
        preferred: struct { url: []const u8 },
    }, alloc, body, .{ .ignore_unknown_fields = true });

    return try std.fmt.allocPrint(alloc, "https:{s}", .{parsed.preferred.url});
}

test "parseInfoboxFilm furious 7" {
    const source = @embedFile("res/furious_7.txt");
    const film = try parseInfoboxFilm("Furious 7", source);
    try std.testing.expectEqualStrings("Furious 7", film.title);
    try std.testing.expectEqual(sphtud.datetime.Date{ .year = 2015, .month = .apr, .day = 1 }, film.released);
    try std.testing.expectEqualStrings("Furious 7 poster.jpg", film.poster_filename);
}

test "parseInfoboxFilm odyssey" {
    const source = @embedFile("res/odyssey.txt");
    const film = try parseInfoboxFilm("The Odyssey (2026 film)", source);
    try std.testing.expectEqualStrings("The Odyssey", film.title);
    try std.testing.expectEqual(sphtud.datetime.Date{ .year = 2026, .month = .jul, .day = 6 }, film.released);
    try std.testing.expectEqualStrings("The Odyssey (2026 film) poster.jpg", film.poster_filename);
}

test "parseInfoboxFilm jurassic park" {
    const source = @embedFile("res/jurassic_park.txt");
    const film = try parseInfoboxFilm("Jurassic Park", source);
    try std.testing.expectEqualStrings("Jurassic Park", film.title);
    try std.testing.expectEqual(sphtud.datetime.Date{ .year = 1993, .month = .jun, .day = 9 }, film.released);
    try std.testing.expectEqualStrings("Jurassic Park poster.jpg", film.poster_filename);
}

test "parseImdbId furious 7" {
    const source = @embedFile("res/furious_7.txt");
    const ref = try parseImdbId("Furious 7", source);
    try std.testing.expectEqualDeep(ImdbRef{ .direct_no_prefix = "2820852" }, ref);
}

test "parseImdbId odyssey falls back to wikidata name" {
    const source = @embedFile("res/odyssey.txt");
    const ref = try parseImdbId("The Odyssey", source);
    try std.testing.expectEqualDeep(ImdbRef{ .wikidata_name = "The Odyssey" }, ref);
}
