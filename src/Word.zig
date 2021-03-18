const std = @import("std");
const Char = @import("char.zig").Char;
const log = std.log.scoped(.Word);

text: []u8,

const Self = @This();

pub fn compareLengthDesc(context: void, left: Self, right: Self) bool {
    return left.text.len > right.text.len;
}

pub fn format(self: @This(), comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
    try writer.print("\"{s}\"", .{self.text});
}
