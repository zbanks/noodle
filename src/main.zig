const std = @import("std");
const nx = @import("nx.zig");
const Char = @import("char.zig").Char;
const log = std.log;

pub const log_level: std.log.Level = .warn;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer {
        const leaked = gpa.deinit();
    }

    const x = Char.fromU8('d');
    //var nx = try Expression.init("e.p[^aeiou][aeiou](ss)iontest", &gpa.allocator);
    var n = try nx.Expression.init("expres*iontest", 1, &gpa.allocator);
    defer n.deinit();
    try n.printNfa();

    const input_text = "expressiontest";
    var input_chars: []Char = try gpa.allocator.alloc(Char, input_text.len + 1);
    defer gpa.allocator.free(input_chars);
    Char.translate(input_text, input_chars);

    var words = try nx.Wordlist.initFromFile("/usr/share/dict/words", &gpa.allocator);
    defer words.deinit();

    var timer = try std.time.Timer.start();
    var dt = timer.read();
    std.debug.print("loading wordlist took {} ns ({} ms)\n", .{ dt, dt / 1_000_000 });

    var n2 = try nx.Expression.init("express[^i].*", 0, &gpa.allocator);
    //var n2 = try nx.Expression.init("expressiontest", 0, &gpa.allocator);
    defer n2.deinit();
    try n2.printNfa();
    _ = n2;

    timer.reset();
    var matcher = try nx.Matcher.init(&.{ &n, &n2 }, words, 7, &gpa.allocator);
    defer matcher.deinit();

    while (matcher.match()) |m| {
        log.info("Match {s}", .{m});
    }

    dt = timer.read();
    std.debug.print("results: {} in {}ns ({} ms)\n", .{ matcher.match_count, dt, dt / 1_000_000 });
}
