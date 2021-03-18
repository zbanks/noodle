const std = @import("std");
const Char = @import("char.zig").Char;
const Expression = @import("Expression.zig");
const Word = @import("Word.zig");
const Wordlist = @import("Wordlist.zig");
const log = std.log.scoped(.Matcher);

caches: []MatchCache,
layers: []Layer,
fuzz_max: usize,
words_max: usize,
match_count: usize,
index: usize,
allocator: *std.mem.Allocator,
wordlist: []*const Word,
output_buffer: [1024]u8,

const MatchCache = struct {
    const TransitionsSet = struct {
        items: []Expression.State.Set,

        fn init(size: usize, allocator: *std.mem.Allocator) !TransitionsSet {
            std.debug.assert(size > 0);
            var self = TransitionsSet{ .items = try allocator.alloc(Expression.State.Set, size) };
            self.clear();
            return self;
        }

        fn clear(self: *TransitionsSet) void {
            for (self.items) |*t| {
                t.* = Expression.State.Set.initEmpty();
            }
        }

        fn free(self: *TransitionsSet, allocator: *std.mem.Allocator) void {
            allocator.free(self.items);
        }

        fn hash(self: TransitionsSet) u32 {
            var hasher = std.hash.Wyhash.init(0);
            std.hash.autoHashStrat(&hasher, self, .Deep);
            return @truncate(u32, hasher.final());
        }

        fn eql(a: TransitionsSet, b: TransitionsSet) bool {
            std.debug.assert(a.items.len == b.items.len);
            for (a.items) |x, i| {
                if (!std.meta.eql(x, b.items[i])) {
                    return false;
                }
            }
            return true;
            //return std.mem.eql(Expression.State.Set, a, b);
        }

        fn slice(self: TransitionsSet, width: usize, index: usize) []Expression.State.Set {
            const i = index * width;
            return self.items[i .. i + width];
        }
    };

    const CacheClass = struct {
        words: std.ArrayList(*const Word),
        nonnull_transitions: Expression.State.Set,
        transitions: TransitionsSet,
        transitions_width: usize,
        allocator: *std.mem.Allocator,

        fn init(expression: *const Expression, allocator: *std.mem.Allocator) !CacheClass {
            const transitions_width = expression.fuzz + 1;
            const transitions_size = expression.states.items.len * (expression.fuzz + 1);

            return CacheClass{
                .words = std.ArrayList(*const Word).init(allocator),
                .nonnull_transitions = Expression.State.Set.initEmpty(),
                .transitions = try TransitionsSet.init(transitions_size, allocator),
                .transitions_width = transitions_width,
                .allocator = allocator,
            };
        }

        fn deinit(self: CacheClass) void {
            self.words.deinit();
            self.allocator.free(self.transitions.items);
        }

        fn transitionsSlice(self: CacheClass, index: usize) []Expression.State.Set {
            return self.transitions.slice(self.transitions_width, index);
        }
    };

    const ClassesHashMap = std.array_hash_map.ArrayHashMap(TransitionsSet, CacheClass, TransitionsSet.hash, TransitionsSet.eql, false);

    classes: ClassesHashMap,
    //classes: []CacheClass,
    //num_classes: usize,
    word_classes: []usize,
    nonnull_words: []*const Word,
    expression: *Expression,
    allocator: *std.mem.Allocator,
    wordlist: []*const Word,

    const Self = @This();

    pub fn init(expression: *Expression, wordlist: []*const Word) !Self {
        var allocator = expression.allocator;
        var timer = try std.time.Timer.start();

        var word_classes = try allocator.alloc(usize, wordlist.len);
        errdefer allocator.free(word_classes);

        var nonnull_words = std.ArrayList(*const Word).init(allocator);
        errdefer nonnull_words.deinit();

        var self: MatchCache = .{
            .classes = ClassesHashMap.init(allocator),
            .word_classes = word_classes,
            .expression = expression,
            .allocator = allocator,
            .nonnull_words = &[0]*const Word{},
            .wordlist = wordlist,
        };
        errdefer self.classes.deinit();

        // The first class is always the empty class
        var empty_class = try CacheClass.init(expression, allocator);
        try self.classes.put(empty_class.transitions, empty_class);

        var temp_class = try CacheClass.init(expression, allocator);
        defer temp_class.deinit();

        var buffer = std.ArrayList(Char).init(allocator);
        defer buffer.deinit();

        for (wordlist) |word, w| {
            try Char.translate(word.text, &buffer);

            // TODO: This is O(n^2)ish, could probably be closer to O(n)ish
            temp_class.transitions.clear();
            for (expression.states.items) |state, i| {
                expression.matchPartial(buffer.items, @intCast(Expression.State.Index, i), temp_class.transitionsSlice(i));
            }

            var result = try self.classes.getOrPut(temp_class.transitions);
            if (!result.found_existing) {
                result.entry.value = try CacheClass.init(expression, allocator);
                var class = &result.entry.value;
                std.mem.copy(Expression.State.Set, class.transitions.items, temp_class.transitions.items);
                result.entry.key = class.transitions;

                for (expression.states.items) |state, i| {
                    if (!class.transitionsSlice(i)[0].isEmpty()) {
                        class.nonnull_transitions.set(i);
                    }
                }

                const c = self.classes.count();
                if (c < 20) {
                    log.info("{}: nonnull: {a}: {s}", .{ c - 1, class.nonnull_transitions, word.text });
                }
            }
            try result.entry.value.words.append(word);

            if (result.index != 0) {
                self.word_classes[nonnull_words.items.len] = result.index;
                try nonnull_words.append(word);
            }
        }

        self.nonnull_words = nonnull_words.toOwnedSlice();

        const num_classes = self.classes.count();
        const dt = timer.read();
        std.debug.print("{} distinct classes with {} words in {}ms ({} ns/word)\n", .{ num_classes, self.nonnull_words.len, dt / 1_000_000, dt / wordlist.len });
        //std.debug.print("{} input words; {} nonmatch\n", .{wordlist.words.items.len, self.classes[0].words.items.len});

        return self;
    }

    pub fn reduceWordlist(self: *Self, new_wordlist: []*const Word) !void {
        //std.debug.assert(self.nonnull_words != null);
        var new_word_classes = try self.allocator.alloc(usize, new_wordlist.len);
        errdefer self.allocator.free(new_word_classes);

        var i: usize = 0;
        for (self.nonnull_words) |word, w| {
            if (i < new_wordlist.len and word == new_wordlist[i]) {
                new_word_classes[i] = self.word_classes[w];
                i += 1;
            }
        }
        std.debug.assert(i == new_wordlist.len);

        self.allocator.free(self.nonnull_words);
        self.nonnull_words = &[0]*const Word{};

        self.allocator.free(self.word_classes);
        self.word_classes = new_word_classes;
    }

    pub fn deinit(self: *Self) void {
        self.allocator.free(self.nonnull_words);
        self.allocator.free(self.word_classes);
        var it = self.classes.iterator();
        while (it.next()) |entry| {
            entry.value.deinit();
        }
        self.classes.deinit();
    }
};

const Layer = struct {
    wi: usize,
    stem: *const Word,
    states: MatchCache.TransitionsSet,
};

pub fn init(expressions: []*Expression, wordlist: Wordlist, words_max: usize, allocator: *std.mem.Allocator) !@This() {
    var caches = try std.ArrayList(MatchCache).initCapacity(allocator, expressions.len);
    errdefer caches.deinit();

    var words = wordlist.pointer_slice;
    errdefer for (caches.items) |*cache| cache.deinit();
    for (expressions) |expr, i| {
        caches.appendAssumeCapacity(try MatchCache.init(expr, words));
        words = caches.items[i].nonnull_words;
    }

    for (caches.items[0 .. caches.items.len - 1]) |*cache| {
        try cache.reduceWordlist(words);
    }

    var fuzz_max: usize = 0;
    for (caches.items) |*cache| {
        fuzz_max = std.math.max(fuzz_max, cache.expression.fuzz);
    }
    log.info("fuzz_max={}", .{fuzz_max});

    var layers = try std.ArrayList(Layer).initCapacity(allocator, words_max + 2);
    errdefer layers.deinit();

    errdefer for (layers.items) |*layer| layer.states.free(allocator);
    var l: usize = 0;
    while (l < layers.capacity) : (l += 1) {
        layers.addOneAssumeCapacity().states = try MatchCache.TransitionsSet.init(caches.items.len * (fuzz_max + 1), allocator);
    }

    layers.items[0].states.clear();
    for (expressions) |expr, i| {
        expr.matchPartial(&.{.end}, 0, layers.items[0].states.slice(fuzz_max + 1, i));
    }
    layers.items[0].wi = 0;

    return @This(){
        .caches = caches.toOwnedSlice(),
        .layers = layers.toOwnedSlice(),
        .fuzz_max = fuzz_max,
        .words_max = words_max,
        .allocator = allocator,
        .wordlist = words,
        .output_buffer = undefined,
        .index = 0,
        .match_count = 0,
    };
}

pub fn deinit(self: *@This()) void {
    for (self.layers) |*layer| {
        layer.states.free(self.allocator);
    }
    self.allocator.free(self.layers);
    for (self.caches) |*cache| {
        cache.deinit();
    }
    self.allocator.free(self.caches);
}

fn formatOutput(self: *@This()) []const u8 {
    var writer = std.io.fixedBufferStream(&self.output_buffer);
    var i: usize = 0;
    while (i <= self.index) : (i += 1) {
        _ = writer.write(" ") catch null;
        _ = writer.write(self.layers[i].stem.text) catch null;
    }
    return writer.getWritten()[1..];
}

pub fn match(self: *@This()) ?[]const u8 {
    var result: ?[]const u8 = null;
    while (true) {
        var layer = &self.layers[self.index];
        var no_match = false;
        var all_end_match = true;
        const word = self.wordlist[layer.wi];
        match: {
            for (self.caches) |*cache, c| {
                if (cache.word_classes[layer.wi] == 0) {
                    no_match = true;
                    break :match; // non-match
                }

                var end_ss = self.layers[self.index + 1].states.slice(self.fuzz_max + 1, c)[0..(cache.expression.fuzz + 1)];
                var states = layer.states.slice(self.fuzz_max + 1, c)[0..(cache.expression.fuzz + 1)];

                for (end_ss) |*e| {
                    e.* = Expression.State.Set.initEmpty();
                }
                const class = &cache.classes.items()[cache.word_classes[layer.wi]].value;

                const verbose = false;
                //const verbose = std.mem.eql(u8, word.text, "expression") or std.mem.eql(u8, word.text, "test");
                if (verbose) {
                    log.info(">>> expr#{} word={s}, start#0={a}", .{ c, word.text, states[0] });
                }

                // This style is faster for dense (small) bitsets
                for (states) |*fuzz_states, f| {
                    var iter = fuzz_states.iterator();
                    while (iter.next()) |si| {
                        if (si >= cache.expression.states.items.len) {
                            std.debug.assert(si > 250);
                            break;
                        }
                        var fd: usize = 0;
                        while (f + fd <= cache.expression.fuzz) : (fd += 1) {
                            if (verbose) {
                                log.info("transitions from {} = {a}", .{ si, class.transitionsSlice(si)[fd] });
                            }
                            end_ss[f + fd].setUnion(class.transitionsSlice(si)[fd]);
                        }
                    }
                }

                var all_empty = true;
                var any_end_match = false;
                var success = cache.expression.states.items.len - 1;
                for (end_ss) |*es, e| {
                    if (verbose) {
                        log.info("expr#{} word={s}, start#{}={a}, es#{}={a}, success={} ({}), empty={}", .{ c, word.text, e, states[e], e, es.*, es.isSet(success), success, es.isEmpty() });
                    }
                    if (es.isSet(success)) {
                        any_end_match = true;
                        all_empty = false;
                    } else if (!es.isEmpty()) {
                        all_empty = false;
                    }
                }

                if (verbose) {
                    log.info("{s} layer={}, expr={} results: any_end_match={} all_empty={} no_match={} end_ss={a}", .{ word.text, self.index, c, any_end_match, all_empty, no_match, end_ss });
                }

                if (all_empty) {
                    std.debug.assert(!any_end_match);
                    no_match = true;
                    break;
                }
                if (!any_end_match) {
                    all_end_match = false;
                }
            }

            if (no_match) {
                break :match;
            }

            layer.stem = word;

            if (all_end_match) {
                result = self.formatOutput();
                //std.debug.print("Match: {s}\n", .{result});
                self.match_count += 1;
            }
        }
        if (self.advance(!no_match) or result != null) {
            break;
        }
    }
    return result;
}

fn advance(self: *@This(), partial_match: bool) bool {
    var pm = partial_match;
    while (true) {
        if (!pm) {
            self.layers[self.index].wi += 1;
            if (self.layers[self.index].wi >= self.wordlist.len) {
                if (self.index == 0) {
                    log.info("done", .{});
                    return true;
                }
                self.index -= 1;
                continue;
            }
            break;
        } else {
            if (self.index + 1 >= self.words_max) {
                pm = false;
                continue;
            }
            self.index += 1;
            self.layers[self.index].wi = 0;
            break;
        }
    }
    return false;
}
