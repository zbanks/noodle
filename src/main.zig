const std = @import("std");
const nx = @import("nx.zig");
const Char = @import("char.zig").Char;
const log = std.log;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer {
        const leaked = gpa.deinit();
    }

    const x = Char.fromU8('d');
    //var nx = try Expression.init("e.p[^aeiou][aeiou](ss)iontest", &gpa.allocator);
    var n = try nx.Expression.init("expressiontest", 2, &gpa.allocator);
    defer {
        n.deinit();
    }
    try n.printNfa();

    const input_text = "expressiontest";
    var input_chars: []Char = try gpa.allocator.alloc(Char, input_text.len + 1);
    defer gpa.allocator.free(input_chars);
    Char.translate(input_text, input_chars);

    var timer = try std.time.Timer.start();
    timer.reset();
    const cycles: usize = 100000;
    const m1 = n.matchFuzzyTest(input_chars, 0, cycles);
    var dt = timer.read();
    const m2 = try n.match("ekpresiontqest", 3);
    std.debug.print("Match results: {}, {}; dt={} for {} cycles ({} ns/cycle)\n", .{ m1, m2, dt, cycles, dt / cycles });
    std.debug.print("input: {a}", .{input_chars});
}
