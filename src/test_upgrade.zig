const std = @import("std");
const sphtud = @import("sphtud");
const Db = @import("Db.zig");
const types = @import("types.zig");
const wikipedia = @import("wikipedia.zig");
const MovieUpdater = @import("MovieUpdater.zig");

const Ids = struct {
    movie_updater: sphtud.util.IdAlloc.Range,
    runtime: sphtud.io.Runtime.Ids,
    resolver: usize,

    pub fn init() Ids {
        var alloc = sphtud.util.IdAlloc.init;
        return .{
            .movie_updater = alloc.allocMany(8),
            .resolver = alloc.allocOne(),
            .runtime = .init(&alloc),
        };
    }
};

const ids = Ids.init();

// FIXME: Unit test selectMoviesToUpdate

pub fn main(init: std.process.Init.Minimal) !void {
    var tpa: sphtud.alloc.TinyPageAllocator = undefined;
    try tpa.initPinned();

    var root_alloc: sphtud.alloc.Sphalloc = undefined;
    try root_alloc.initPinned(tpa.allocator(), "root");

    var runtime: sphtud.io.Runtime = undefined;
    try runtime.initPinned(&root_alloc, ids.runtime);

    var args = init.args.iterate();
    _ = args.next();
    const db_path = args.next() orelse return error.NoDbPath;

    var db = try Db.init(db_path);

    var updater: MovieUpdater = undefined;
    try updater.initPinned(
        root_alloc.general(),
        &db,
        &runtime.tls_spawner,
        ids.movie_updater.start,
        ids.movie_updater.end - ids.movie_updater.start + 1,
    );

    //var resolver: wikipedia.RemoteMovieResolver = undefined;
    //try resolver.initTitlePinned(
    //    root_alloc.general(),
    //    "Lilo & Stitch",
    //    &runtime.tls_spawner,
    //    ids.resolver,
    //);

    while (!updater.isDone()) {
        const id = try runtime.service(ids.runtime);

        if (ids.movie_updater.contains(id)) {
            try updater.poll(&runtime.tls_spawner, &runtime.loop, id);
        }
    }

    //while (true) {
    //    const id = try runtime.service(ids.runtime);

    //    if (id == ids.resolver) {
    //        const movie = try resolver.poll(&runtime.tls_spawner, &runtime.loop) orelse continue;
    //        std.debug.print("Movie has id {s}\n", .{movie.imdb_id});
    //        return;
    //    }
    //}

    std.log.info("movie update complete", .{});
}
