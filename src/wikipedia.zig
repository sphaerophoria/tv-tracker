pub const Search = @import("wikipedia/Search.zig");
pub const RemoteMovieResolver = @import("wikipedia/RemoteMovieResolver.zig");

const std = @import("std");

pub const SearchMovie = struct {
    name: []const u8,
    year: i32,
    image: []const u8,
    wikipedia_page_id: i64,
};

pub fn makeApiQuery(query: []const u8) std.Uri {
    return std.Uri{
        .scheme = "https",
        .host = .{ .raw = "en.wikipedia.org" },
        .path = .{ .percent_encoded = "/w/api.php" },
        .query = .{ .percent_encoded = query },
    };
}
