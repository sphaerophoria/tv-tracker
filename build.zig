const std = @import("std");

pub fn build(b: *std.Build) !void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const sphtud_dep = b.dependency("sphtud", .{ .with_ssl = true });

    const sqlite_bindings = b.addTranslateC(.{
        .root_source_file = b.path("sqlite/sqlite3.h"),
        .optimize = optimize,
        .target = target,
    });

    const sqlite_impl = b.createModule(.{
        .optimize = .ReleaseSafe,
        .target = target,
    });
    sqlite_impl.addCSourceFile(.{
        .file = b.path("sqlite/sqlite3.c"),
        .flags = &.{},
    });
    sqlite_impl.link_libc = true;
    const sqlite_lib = b.addLibrary(.{
        .name = "sqlite",
        .root_module = sqlite_impl,
        .linkage = .static,
    });

    const exe = b.addExecutable(.{
        .name = "tv-tracker",
        .root_module = b.createModule(.{
            .root_source_file = b.path("src/main.zig"),
            .target = target,
            .optimize = optimize,
        }),
    });
    exe.root_module.linkLibrary(sqlite_lib);
    exe.root_module.addImport("sqlite", sqlite_bindings.createModule());
    exe.root_module.addImport("sphtud", sphtud_dep.module("sphtud"));
    b.installArtifact(exe);

    const tests = b.addTest(.{
        .root_module = exe.root_module,
    });
    b.installArtifact(tests);
}
