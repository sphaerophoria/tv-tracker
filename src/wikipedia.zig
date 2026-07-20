pub const Search = @import("wikipedia/Search.zig");
pub const RemoteMovieResolver = @import("wikipedia/RemoteMovieResolver.zig");

pub const SearchMovie = struct {
    name: []const u8,
    year: i32,
    image: []const u8,
    wikipedia_page_id: i64,
};
