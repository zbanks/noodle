const std = @import("std");
const log = std.log.scoped(.char);

pub const Char = enum(u5) {
    end,
    epsilon,
    punct,
    space,
    a,
    b,
    c,
    d,
    e,
    f,
    g,
    h,
    i,
    j,
    k,
    l,
    m,
    n,
    o,
    p,
    q,
    r,
    s,
    t,
    u,
    v,
    w,
    x,
    y,
    z,

    const Self = @This();
    // TODO: Should this use std.bit_set.IntegerBitSet?
    pub const Bitset = u32;

    pub const letters_bitset: Bitset = comptime {
        var b: Bitset = 0;
        var j: u8 = 'a';
        while (j <= 'z') {
            b |= Self.fromU8(j).toBitset();
            j += 1;
        }
        return b;
    };

    pub fn toU8(self: Self) u8 {
        return switch (self) {
            .end => '$',
            .epsilon => '*',
            .punct => '\'',
            .space => '_',
            else => @intCast(u8, @enumToInt(self) - @enumToInt(Self.a)) + 'a',
        };
    }

    pub fn fromU8(u: u8) Self {
        return switch (u) {
            0 => .end,
            ' ', '_' => .space,
            'A'...'Z' => @intToEnum(Self, @intCast(u5, u - 'A') + @enumToInt(Self.a)),
            'a'...'z' => @intToEnum(Self, @intCast(u5, u - 'a') + @enumToInt(Self.a)),
            else => .punct,
        };
    }

    pub fn toBitset(self: Self) Bitset {
        const one: Bitset = 1;
        return one << @enumToInt(self);
    }

    pub fn translate(text: []const u8, chars: *std.ArrayList(Self)) !void {
        try chars.resize(text.len + 1);
        for (text) |t, i| {
            chars.items[i] = Self.fromU8(t);
        }
        chars.items[text.len] = Self.end;
    }

    pub fn format(self: Self, comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
        return writer.print("{c}", .{self.toU8()});
    }

    pub fn formatBitset(writer: anytype, bitset: Bitset) !void {
        try writer.print("[", .{});
        var i: u5 = 0;
        while (true) {
            var c = @intToEnum(Self, i);
            if ((bitset & c.toBitset()) != 0) {
                try writer.print("{any}", .{c});
            }
            if (c == .z) {
                break;
            }
            i += 1;
        }
        try writer.print("]", .{});
    }
};

test "Char enum reflection" {
    inline for (@typeInfo(Char).Enum.fields) |field| {
        const c1 = @field(Char, field.name);
        const u = c1.toU8();
        const c2 = Char.fromU8(u);

        switch (c1) {
            // Exceptions to reflection
            .end, .epsilon => {},
            else => std.testing.expectEqual(c1, c2),
        }
    }
}
