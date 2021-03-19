const std = @import("std");
const Char = @import("char.zig").Char;
const log = std.log.scoped(.Word);

text: []u8,
chars: []Char,

const Self = @This();

pub fn compareLengthDesc(context: void, left: Self, right: Self) bool {
    return left.text.len > right.text.len;
}

pub fn compareChars(context: void, left: Self, right: Self) bool {
    const lhs = left.chars;
    const rhs = right.chars;
    const n = std.math.min(lhs.len, rhs.len);
    var i: usize = 0;
    while (i < n) : (i += 1) {
        const l = @enumToInt(lhs[i]);
        const r = @enumToInt(rhs[i]);
        if (l == r) {
            continue;
        }
        return l < r;
    }
    return lhs.len < rhs.len;
}

pub fn format(self: @This(), comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
    try writer.print("\"{s}\"", .{self.text});
}
