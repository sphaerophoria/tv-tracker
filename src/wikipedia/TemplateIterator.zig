const sphtud = @import("sphtud");
const std = @import("std");

state: union(enum) {
    parsing: sphtud.lex.Buf,
    finished,
},

const TemplateIterator = @This();

pub fn init(data: []const u8) !TemplateIterator {
    var buf = sphtud.lex.Buf.init(data);
    _ = buf.takeSequence("{{") orelse return error.NotATemplate;
    return .{ .state = .{
        .parsing = buf,
    } };
}

pub fn next(self: *TemplateIterator) !?[]const u8 {
    const buf = switch (self.state) {
        .parsing => |*b| b,
        .finished => return null,
    };

    var tmp = buf.tmp();

    const Tag = enum {
        @"{{",
        @"[[",
    };

    const max_depth = 20;
    var tag_stack_buf: [max_depth]Tag = undefined;
    var tag_stack = std.ArrayList(Tag).initBuffer(&tag_stack_buf);

    while (true) {
        _ = tmp.takeUntilAny("{[|}]");
        const idx = tmp.takeOne("{[|}]") orelse return error.EndOfStream;
        switch (idx.data(tmp)) {
            '{' => {
                _ = tmp.takeOne("{") orelse continue;
                try tag_stack.appendBounded(.@"{{");
            },
            '[' => {
                _ = tmp.takeOne("[") orelse continue;
                try tag_stack.appendBounded(.@"[[");
            },
            '}' => {
                _ = tmp.takeOne("}") orelse continue;

                if (tag_stack.items.len == 0) {
                    var range = buf.commit(tmp).?;
                    range.end -= 2;
                    const ret = range.data(buf.*);
                    self.state = .finished;
                    return ret;
                } else {
                    if (tag_stack.getLastOrNull() != .@"{{") continue;
                    _ = tag_stack.pop();
                }
            },
            ']' => {
                if (tag_stack.getLastOrNull() != .@"[[") continue;
                _ = tmp.takeOne("]") orelse continue;
                _ = tag_stack.pop();
            },
            '|' => {
                if (tag_stack.items.len != 0) continue;

                var ret = buf.commit(tmp).?;
                ret.end -= 1;
                return ret.data(buf.*);
            },
            else => unreachable,
        }
    }
}

test "TemplateIterator film date" {
    var it = try TemplateIterator.init("{{Film date|2026|12|10}}");

    try std.testing.expectEqualStrings("Film date", try it.next() orelse unreachable);
    try std.testing.expectEqualStrings("2026", try it.next() orelse unreachable.?);
    try std.testing.expectEqualStrings("12", try it.next() orelse unreachable.?);
    try std.testing.expectEqualStrings("10", try it.next() orelse unreachable.?);
    try std.testing.expectEqual(null, try it.next());
}

test "TemplateIterator nested list" {
    var it = try TemplateIterator.init("{{Something|{{a | b}}}}");

    try std.testing.expectEqualStrings("Something", try it.next() orelse unreachable);
    try std.testing.expectEqualStrings("{{a | b}}", try it.next() orelse unreachable.?);
    try std.testing.expectEqual(null, try it.next());
}

test "TemplateIterator square brackets list" {
    var it = try TemplateIterator.init("{{Something|[[a | b]]}}");

    try std.testing.expectEqualStrings("Something", try it.next() orelse unreachable);
    try std.testing.expectEqualStrings("[[a | b]]", try it.next() orelse unreachable.?);
    try std.testing.expectEqual(null, try it.next());
}
