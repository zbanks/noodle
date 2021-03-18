const std = @import("std");
const nx = @import("nx.zig");
const Char = @import("char.zig").Char;
const wordlist = @import("wordlist.zig");
const log = std.log;

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

    var timer = try std.time.Timer.start();
    timer.reset();
    const cycles: usize = 100;
    const m1 = n.matchFuzzyTest(input_chars, 0, cycles);
    var dt = timer.read();
    const m2 = try n.match("ekpresiontqest", 3);
    std.debug.print("Match results: {}, {}; dt={} for {} cycles ({} ns/cycle)\n", .{ m1, m2, dt, cycles, dt / cycles });
    std.debug.print("input: {a}\n", .{input_chars});

    timer.reset();
    var words = try wordlist.Wordlist.initFromFile("/usr/share/dict/words", &gpa.allocator);
    defer words.deinit();
    dt = timer.read();
    std.debug.print("loading wordlist took {} ns ({} ms)\n", .{ dt, dt / 1_000_000 });

    //var combo_cache = try nx.ComboCache.init(&n, &words);
    //defer combo_cache.deinit();
    //log.info("test: {any}", .{combo_cache.classes.items()[0..20]});

    var n2 = try nx.Expression.init("express[^i].*", 0, &gpa.allocator);
    //var n2 = try nx.Expression.init("expressiontest", 0, &gpa.allocator);
    defer n2.deinit();
    try n2.printNfa();
    _ = n2;

    timer.reset();
    var combo_match = try nx.ComboMatcher.init(&.{ &n, &n2 }, words, 7, &gpa.allocator);
    defer combo_match.deinit();

    while (combo_match.match()) |m| {
        log.info("Match {s}", .{m});
    }

    dt = timer.read();
    std.debug.print("results: {} in {}ns ({} ms)\n", .{ combo_match.match_count, dt, dt / 1_000_000 });
}
