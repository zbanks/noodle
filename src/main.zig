const std = @import("std");
const nx = @import("nx.zig");
const Char = @import("char.zig").Char;
const Trie = @import("Trie.zig");
const log = std.log;

pub const log_level: std.log.Level = .info;

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer std.debug.assert(!gpa.deinit());

    var timer = try std.time.Timer.start();
    var words = try nx.Wordlist.initFromFile("/usr/share/dict/words", &gpa.allocator);
    defer words.deinit();
    var dt = timer.read();
    std.debug.print("loading wordlist took {} ns ({} ms)\n", .{ dt, dt / 1_000_000 });

    //var nx = try Expression.init("e.p[^aeiou][aeiou](ss)iontest", &gpa.allocator);
    var n = try nx.Expression.init("expres*iontest", 1, &gpa.allocator);
    defer n.deinit();
    try n.printNfa();
    _ = n;

    var n2 = try nx.Expression.init("express+[^i].*", 0, &gpa.allocator);
    //var n2 = try nx.Expression.init("expressiontest", 0, &gpa.allocator);
    defer n2.deinit();
    try n2.printNfa();
    _ = n2;

    timer.reset();
    var matcher = try nx.Matcher.init(&.{ &n, &n2 }, words, 3, &gpa.allocator);
    defer matcher.deinit();

    while (matcher.match()) |m| {
        log.info("Match {s}", .{m});
    }

    dt = timer.read();
    std.debug.print("results: {} in {}ns ({} ms)\n", .{ matcher.match_count, dt, dt / 1_000_000 });
}
