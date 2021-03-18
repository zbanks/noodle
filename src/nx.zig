const std = @import("std");
const Char = @import("char.zig").Char;
const log = std.log.scoped(.nx);

const wl = @import("wordlist.zig");
const Wordlist = wl.Wordlist;
const Word = wl.Word;

fn StateSet(comptime num_states: comptime_int) type {
    return struct {
        // TODO: Size at compile time, based on desired max # of states
        bitset: BitSet,

        // For some reason, u16/u32 is faster than u8-u15?
        //pub const Index = std.math.IntFittingRange(0, std.math.max(num_states - 1, std.math.maxInt(u32)));
        pub const Index = std.math.IntFittingRange(0, num_states - 1);
        //pub const Index = u32;
        const BitSet = std.bit_set.StaticBitSet(num_states);
        const Self = @This();

        pub const nonempty = num_states - 1;
        pub const failure = num_states - 2;
        pub const success = num_states - 3;
        pub const max_normal = num_states - 4;

        /// Creates a bit set with no elements present.
        pub fn initEmpty() Self {
            //@compileLog("Index = ", Index);
            return .{ .bitset = BitSet.initEmpty() };
        }

        /// Returns true if the bit at the specified index
        /// is present in the set, false otherwise.
        pub fn isSet(self: Self, index: usize) bool {
            return self.bitset.isSet(index);
        }

        /// Adds a specific bit to the bit set
        pub fn set(self: *Self, index: usize) void {
            self.bitset.set(nonempty);
            self.bitset.set(index);
        }

        /// Performs a union of two bit sets, and stores the
        /// result in the first one.  Bits in the result are
        /// set if the corresponding bits were set in either input.
        pub fn setUnion(self: *Self, other: Self) void {
            return self.bitset.setUnion(other.bitset);
        }

        /// Iterates through the items in the set, according to the options.
        /// Modifications to the underlying bit set may or may not be
        /// observed by the iterator.
        const Iterator = @TypeOf(BitSet.iterator(&BitSet.initEmpty(), .{}));
        pub fn iterator(self: *const Self) Iterator {
            return self.bitset.iterator(.{});
        }

        pub fn eql(self: Self, other: Self) bool {
            if (@hasField(BitSet, "masks")) {
                if (self.isEmpty() and other.isEmpty()) {
                    return true;
                }
                return std.mem.eql(BitSet.MaskInt, &self.bitset.masks, &other.bitset.masks);
            } else {
                return self.bitset.mask == other.bitset.mask;
            }
        }

        pub fn isEmpty(self: @This()) bool {
            return !self.bitset.isSet(nonempty);
            //return self.bitset.findFirstSet() == null;
        }

        pub fn format(self: @This(), comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
            try writer.print("{{", .{});
            var first = true;
            var iter = self.iterator();
            while (iter.next()) |i| {
                if (i == Self.nonempty) {
                    continue;
                }
                if (!first) {
                    try writer.print(",", .{});
                }
                try writer.print("{}", .{i});
                first = false;
            }
            try writer.print("}}", .{});
        }
    };
}

pub const Expression = struct {
    allocator: *std.mem.Allocator,
    states: std.ArrayList(State),
    expression: []u8,
    ignore_whitespace: bool,
    ignore_punctuation: bool,
    letters_bitset: Char.Bitset,
    fuzz: usize,
    next_state_sets: []State.Set,

    const Self = @This();

    const CompileError = error{
        OutOfMemory,
        TooManyStates,
        BareModifier,
        UnmatchedParentheses,
        UnmatchedSquareBrackets,
        InvalidCharacterClass,
    };

    const State = struct {
        const num_states = 256; // if @sizeOf(StateSet) > 256, triggers memcpy, gets ~2x slower
        const Set = StateSet(num_states);
        const Index = Set.Index;

        const Branch = struct {
            next_state: Index,
            char_bitset: Char.Bitset,
        };

        branches: [2]Branch,
        epsilon_states: Set,
    };

    pub fn init(expression: []const u8, fuzz: usize, allocator: *std.mem.Allocator) !Self {
        const expr = try std.mem.dupe(allocator, u8, expression);
        errdefer allocator.free(expr);

        var next_state_sets = try allocator.alloc(State.Set, fuzz + 1);
        errdefer allocator.free(next_state_sets);

        var self = Self{
            .allocator = allocator,
            .states = std.ArrayList(State).init(allocator),
            .expression = expr,
            .ignore_whitespace = true,
            .ignore_punctuation = true,
            .letters_bitset = Char.letters_bitset,
            .fuzz = fuzz,
            .next_state_sets = next_state_sets,
        };
        errdefer self.states.deinit();

        const len = try self.compile_subexpression(expression);
        if (len != expression.len) {
            return error.UnmatchedParentheses;
        }

        // Calculate epsilon transitions
        for (self.states.items) |*s, i| {
            var next_ss = s.epsilon_states;
            for (s.branches) |b| {
                if ((Char.epsilon.toBitset() & b.char_bitset) != 0) {
                    next_ss.set(b.next_state);
                }
            }
            while (true) {
                var ss = next_ss;
                for (self.states.items) |*s2, si| {
                    if (!next_ss.isSet(@intCast(State.Index, si))) {
                        continue;
                    }
                    for (s2.branches) |b| {
                        if ((Char.epsilon.toBitset() & b.char_bitset) != 0) {
                            ss.set(b.next_state);
                        }
                    }
                }
                if (ss.eql(next_ss)) {
                    break;
                }
                next_ss.setUnion(ss);
            }
            s.epsilon_states = next_ss;
        }
        for (self.states.items) |*s, i| {
            for (s.branches) |*b| {
                if (Char.epsilon.toBitset() == b.char_bitset) {
                    b.char_bitset = 0;
                    b.next_state = 0;
                }
            }
        }

        return self;
    }

    pub fn deinit(self: *Self) void {
        self.allocator.free(self.expression);
        self.allocator.free(self.next_state_sets);
        self.states.deinit();
    }

    pub fn printNfa(self: Self) !void {
        var list = std.ArrayList(u8).init(self.allocator);
        defer list.deinit();
        var writer = list.writer();

        _ = try writer.print("NX NFA: {} states\n", .{self.states.items.len});
        for (self.states.items) |*s, i| {
            _ = try writer.print("    {:3}: ", .{i});
            for (s.branches) |b, j| {
                if (b.char_bitset == 0) {
                    // These two cases are just to catch potentially-invalid representations
                    if (j + 1 < s.branches.len and s.branches[j + 1].char_bitset != 0) {
                        unreachable;
                    }
                    // 0 is technically a valid state; this just catches _most_ errors
                    if (b.next_state != 0) {
                        _ = try writer.print("(null) -> {}    ", .{b.next_state});
                    }
                    continue;
                }
                try Char.formatBitset(writer, b.char_bitset);
                try writer.print(" -> ", .{});
                if (b.next_state > State.Set.success) {
                    _ = try writer.print("!!!{}", .{b.next_state});
                } else if (b.next_state == State.Set.success) {
                    _ = try writer.print("MATCH", .{});
                } else {
                    _ = try writer.print("{:3}", .{b.next_state});
                }
                _ = try writer.print("     ", .{});
            }
            if (!s.epsilon_states.isEmpty()) {
                _ = try writer.print("* -> {}", .{s.epsilon_states});
            }
            _ = try writer.print("\n", .{});
        }
        _ = try writer.print("\n", .{});
        log.info("{s}", .{list.items});
    }

    fn addState(self: *Self) !*State {
        if (self.states.items.len >= Self.State.Set.max_normal) {
            return error.TooManyStates;
        }

        var state = try self.states.addOne();
        for (state.branches) |*b| {
            b.next_state = 0;
            b.char_bitset = 0;
        }
        state.epsilon_states = State.Set.initEmpty();
        return state;
    }

    fn insertState(self: *Self, insert_index: usize) !*State {
        std.debug.assert(insert_index < self.states.items.len);
        if (self.states.items.len >= Self.State.Set.max_normal) {
            return error.TooManyStates;
        }
        _ = try self.states.addOne();
        std.mem.copyBackwards(Self.State, self.states.items[insert_index + 1 ..], self.states.items[insert_index .. self.states.items.len - 1]);

        for (self.states.items[insert_index + 1 ..]) |*state| {
            for (state.branches) |*branch| {
                if (branch.next_state >= insert_index and branch.next_state < self.states.items.len and branch.char_bitset != 0) {
                    branch.next_state += 1;
                }
            }
        }
        return &self.states.items[insert_index];
    }

    fn compile_subexpression(self: *Self, subexpression: []const u8) CompileError!usize {
        var previous_initial_state: ?State.Index = null;
        var subexpression_initial_state: State.Index = @intCast(State.Index, self.states.items.len);
        var subexpression_final_state: ?State.Index = null;

        var i: usize = 0;
        while (i < subexpression.len) {
            const c = subexpression[i];
            var nc: Char = Char.fromU8(c);
            switch (c) {
                '\\', '^', '$', ' ' => {},
                ')' => { // End of parethetical expression
                    if (subexpression_final_state != null) {
                        log.info("Subexpression {}\n", .{subexpression_final_state.?});
                        self.states.items[subexpression_final_state.?].branches[0].next_state = @intCast(State.Index, self.states.items.len);
                    }
                    return i + 1;
                },
                '(' => { // Start of new parethetical expression
                    previous_initial_state = @intCast(State.Index, self.states.items.len);
                    const sub_len = try self.compile_subexpression(subexpression[i + 1 ..]);
                    if (subexpression[i + sub_len] != ')') {
                        return error.UnmatchedParentheses;
                    }
                    i += sub_len;
                },
                'A'...'Z', 'a'...'z' => { // "Normal" letters
                    var s: *State = try self.addState();
                    s.branches[0] = .{
                        .next_state = @intCast(State.Index, self.states.items.len),
                        .char_bitset = nc.toBitset(),
                    };

                    previous_initial_state = @intCast(State.Index, self.states.items.len - 1);
                },
                '_' => { // Explicit space
                    var s: *State = try self.addState();
                    s.branches[0] = .{
                        .next_state = @intCast(State.Index, self.states.items.len),
                        .char_bitset = Char.space.toBitset(),
                    };

                    // XXX: This allows spaces to absorb an arbitrary number of spaces
                    // effectively replacing each "_" with "_+"
                    // This means we don't need to trim spaces from words when doing matches
                    s.branches[1] = .{
                        .next_state = @intCast(State.Index, self.states.items.len - 1),
                        .char_bitset = Char.space.toBitset(),
                    };

                    previous_initial_state = @intCast(State.Index, self.states.items.len - 1);
                },
                '\'', '-' => { // Explicit punctuation
                    var s: *State = try self.addState();
                    s.branches[0] = .{
                        .next_state = @intCast(State.Index, self.states.items.len),
                        .char_bitset = Char.punct.toBitset(),
                    };

                    previous_initial_state = @intCast(State.Index, self.states.items.len - 1);
                },
                '.' => { // Match any 1 character
                    var s: *State = try self.addState();
                    s.branches[0] = .{
                        .next_state = @intCast(State.Index, self.states.items.len),
                        .char_bitset = Char.letters_bitset,
                    };
                    previous_initial_state = @intCast(State.Index, self.states.items.len - 1);
                },
                '[' => { // Character class
                    i += 1;
                    var inverse: bool = false;
                    if (subexpression[i] == '^') {
                        inverse = true;
                        i += 1;
                    }

                    var char_bitset: Char.Bitset = 0;

                    // TODO: support "[a-z]" syntax
                    while (i < subexpression.len and subexpression[i] != ']') {
                        const sc = subexpression[i];
                        const sn = Char.fromU8(sc);
                        char_bitset |= switch (sn) {
                            .end, .epsilon => unreachable,
                            .punct => switch (sc) {
                                ',', '\'' => sn.toBitset(),
                                '.' => Char.letters_bitset,
                                else => {
                                    return error.InvalidCharacterClass;
                                },
                            },
                            else => sn.toBitset(),
                        };
                        i += 1;
                    }
                    if (i >= subexpression.len) {
                        return error.UnmatchedSquareBrackets;
                    }
                    if (inverse) {
                        char_bitset ^= self.letters_bitset;
                    }

                    var s: *State = try self.addState();
                    s.branches[0] = .{
                        .next_state = @intCast(State.Index, self.states.items.len),
                        .char_bitset = char_bitset,
                    };
                    previous_initial_state = @intCast(State.Index, self.states.items.len - 1);
                },
                '*' => {
                    if (previous_initial_state == null) {
                        log.err("parse error: '{}' without preceeding group", .{c});
                        return error.BareModifier;
                    }

                    try self.states.ensureCapacity(self.states.items.len + 2);
                    var epsilon_s: *State = try self.insertState(previous_initial_state.?);

                    previous_initial_state.? += 1;
                    if (subexpression_final_state != null and previous_initial_state.? < subexpression_final_state.?) {
                        subexpression_final_state.? += 1;
                    }

                    epsilon_s.branches[0] = .{
                        .next_state = @intCast(State.Index, previous_initial_state.?),
                        .char_bitset = Char.epsilon.toBitset(),
                    };
                    epsilon_s.branches[1] = .{
                        .next_state = @intCast(State.Index, self.states.items.len + 1),
                        .char_bitset = Char.epsilon.toBitset(),
                    };

                    var s: *State = try self.addState();
                    s.branches[0] = epsilon_s.branches[0];
                    s.branches[1] = epsilon_s.branches[1];
                },
                // TODO: *, +, ?, |, {,
                else => {
                    log.err("Invalid character in noodle expression: '{c}'\n", .{c});
                    // raise ...
                },
            }

            i += 1;
        }

        // End of (full) expression
        var s: *State = try self.addState();
        s.branches[0] = .{
            .next_state = State.Set.success,
            .char_bitset = Char.end.toBitset(),
        };

        if (subexpression_final_state != null) {
            log.info("Subexpression {}\n", .{subexpression_final_state});
            self.states.items[subexpression_final_state.?].branches[0].next_state = @intCast(State.Index, self.states.items.len);
        }

        return i;
    }

    pub fn match(self: *Self, input_text: []const u8, n_errors: usize) !?usize {
        var input_chars: []Char = try self.allocator.alloc(Char, input_text.len + 1);
        defer self.allocator.free(input_chars);
        Char.translate(input_text, input_chars);

        // `epsilon_states` are accounted for *after* "normal" states in `nx_match_transition`
        // Therefore it is important to include them here for correctness
        var ss = State.Set.initEmpty();
        ss.set(0);
        ss.setUnion(self.states.items[0].epsilon_states);

        return self.matchFuzzy(input_chars, ss, n_errors);
    }

    pub fn matchPartial(self: *Self, input: []const Char, initial_state: State.Index, state_sets: []State.Set) void {
        // Start with an initial `state_sets` containing only `initial_state` with 0 fuzz
        std.debug.assert(state_sets.len >= self.fuzz + 1);
        var i: usize = 0;
        while (i < self.fuzz + 1) : (i += 1) {
            state_sets[i] = State.Set.initEmpty();
        }
        state_sets[0].set(initial_state);

        // Add all `epsilon_states`, which are the states reachable from `initial_state`
        // without consuming a character from `buffer`
        state_sets[0].setUnion(self.states.items[initial_state].epsilon_states);

        for (input) |c| {
            if (c == .end) {
                break;
            }
            // Consume 1 character from the buffer and compute the set of possible resulting states
            i = 0;
            while (i < self.fuzz + 1) : (i += 1) {
                self.next_state_sets[i] = self.matchTransition(c.toBitset(), state_sets[i]);
            }

            // For a fuzzy match, expand `next_error_sets[fi+1]` by adding all states
            // reachable from `state_sets[fi]` *but* with a 1-character change to `buffer`
            i = 0;
            while (i < self.fuzz) : (i += 1) {
                // Deletion
                self.next_state_sets[i + 1].setUnion(state_sets[i]);

                // Change
                var change_set = self.matchTransition(self.letters_bitset, state_sets[i]);
                self.next_state_sets[i + 1].setUnion(change_set);

                // Insertion
                var insertion_set = self.matchTransition(c.toBitset(), change_set);
                self.next_state_sets[i + 1].setUnion(insertion_set);
            }

            // Shift next_state_sets into state_sets
            std.mem.copy(State.Set, state_sets, self.next_state_sets);

            // We can terminate early if there are no possible valid states
            for (state_sets) |*s| {
                if (!s.isEmpty()) {
                    break;
                }
            } else {
                return;
            }
        }
    }

    pub fn matchFuzzyTest(self: *Self, input: []const Char, n_errors: usize, count: usize) ?usize {
        var ss = Self.State.Set.initEmpty();
        ss.set(0);
        ss.setUnion(self.states.items[0].epsilon_states);

        var i: usize = 0;
        while (i < count) : (i += 1) {
            _ = self.matchFuzzy(input, ss, 0);
        }

        return self.matchFuzzy(input, ss, n_errors);
    }

    // TODO: Delete this function; use matchPartial instead
    pub fn matchFuzzy(self: *Self, input: []const Char, initial_state_set: State.Set, n_errors: usize) ?usize {
        // If the initial `state_set` is already a match, we're done!
        if (initial_state_set.isSet(State.Set.success)) {
            return 0;
        }

        var state_set = initial_state_set;

        // Keep track of which states are reachable with *exactly* 1 error, initially empty
        var error_state_set = State.Set.initEmpty();

        // Iterate over the characters in `buffer` (exactly 1 character per iteration)
        for (input) |c, i| {
            var next_state_set = self.matchTransition(c.toBitset(), state_set);
            var next_error_set = State.Set.initEmpty();

            if (next_state_set.isSet(State.Set.success)) {
                std.debug.assert(c == .end);
                return 0;
            }

            if (n_errors > 0) {
                next_error_set = self.matchTransition(c.toBitset(), error_state_set);

                if (next_error_set.isSet(State.Set.success)) {
                    std.debug.assert(c == .end);
                    return 1;
                }

                if (c != .end) {
                    // Deletion
                    next_error_set.setUnion(state_set);
                    // Change
                    const es = self.matchTransition(self.letters_bitset, state_set);
                    next_error_set.setUnion(es);
                }

                // XXX: handle two inserts in a row
                // Insertion
                var es = self.matchTransition(self.letters_bitset, state_set);
                es = self.matchTransition(c.toBitset(), es);
                next_error_set.setUnion(es);
            }

            if (next_state_set.isEmpty()) {
                if (n_errors > 0) {
                    const rc = self.matchFuzzy(input[i + 1 ..], next_error_set, n_errors - 1);
                    if (rc) |errors| {
                        return errors + 1;
                    }
                }

                return null;
            }

            if (c == .end) {
                log.err("Buffer is end: next_state_set = {}", .{next_state_set});
                unreachable;
            }

            state_set = next_state_set;
            error_state_set = next_error_set;
        }

        unreachable;
    }

    fn matchTransition(self: *Self, char_bitset: Char.Bitset, start_states: State.Set) State.Set {
        var end_states = State.Set.initEmpty();

        if (start_states.isEmpty()) {
            return end_states;
        }

        if (true) {
            // This style is faster for sparse (large) bitsets
            for (self.states.items) |*state, si| {
                if (!start_states.isSet(@intCast(State.Index, si))) {
                    continue;
                }

                for (state.branches) |b| {
                    if ((char_bitset & b.char_bitset) != 0) {
                        end_states.set(b.next_state);

                        if (b.next_state < self.states.items.len) {
                            end_states.setUnion(self.states.items[b.next_state].epsilon_states);
                        }
                    }
                }
            }

            //for (self.states.items) |*state, si| {
            //    if (!end_states.isSet(@intCast(State.Index, si))) {
            //        continue;
            //    }
            //    end_states.setUnion(state.epsilon_states);
            //}
        } else {
            // This style is faster for dense (small) bitsets
            var iter = start_states.iterator();
            while (iter.next()) |si| {
                if (si >= self.states.items.len) {
                    break;
                }
                for (self.states.items[si].branches) |b| {
                    if ((char_bitset & b.char_bitset) != 0) {
                        end_states.set(b.next_state);
                    }
                }
            }

            iter = end_states.iterator();
            while (iter.next()) |si| {
                if (si >= self.states.items.len) {
                    break;
                }
                end_states.setUnion(self.states.items[si].epsilon_states);
            }
        }

        return end_states;
    }
};

pub const ComboCache = struct {
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
    nonnull_words: ?[]*const Word,
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

        var self: ComboCache = .{
            .classes = ClassesHashMap.init(allocator),
            .word_classes = word_classes,
            .expression = expression,
            .allocator = allocator,
            .nonnull_words = null,
            .wordlist = wordlist,
        };
        errdefer self.classes.deinit();

        // The first class is always the empty class
        var empty_class = try CacheClass.init(expression, allocator);
        try self.classes.put(empty_class.transitions, empty_class);

        var temp_class = try CacheClass.init(expression, allocator);
        defer temp_class.deinit();

        for (wordlist) |word, w| {
            // TODO remove stack buffer?
            var buffer: [256]Char = undefined;
            var wbuf = buffer[0 .. word.text.len + 1];
            Char.translate(word.text, wbuf);

            // TODO: This is O(n^2)ish, could probably be closer to O(n)ish
            temp_class.transitions.clear();
            for (expression.states.items) |state, i| {
                expression.matchPartial(wbuf, @intCast(Expression.State.Index, i), temp_class.transitionsSlice(i));
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
        std.debug.print("{} distinct classes with {} words in {}ms ({} ns/word)\n", .{ num_classes, self.nonnull_words.?.len, dt / 1_000_000, dt / wordlist.len });
        //std.debug.print("{} input words; {} nonmatch\n", .{wordlist.words.items.len, self.classes[0].words.items.len});

        return self;
    }

    pub fn reduceWordlist(self: *Self, new_wordlist: []*const Word) !void {
        std.debug.assert(self.nonnull_words != null);
        var new_word_classes = try self.allocator.alloc(usize, new_wordlist.len);
        errdefer self.allocator.free(new_word_classes);

        var i: usize = 0;
        for (self.nonnull_words.?) |word, w| {
            if (i < new_wordlist.len and word == new_wordlist[i]) {
                new_word_classes[i] = self.word_classes[w];
                i += 1;
            }
        }
        std.debug.assert(i == new_wordlist.len);

        self.allocator.free(self.nonnull_words.?);
        self.nonnull_words = null;

        self.allocator.free(self.word_classes);
        self.word_classes = new_word_classes;
    }

    pub fn deinit(self: *Self) void {
        if (self.nonnull_words != null) {
            self.allocator.free(self.nonnull_words.?);
        }
        self.allocator.free(self.word_classes);
        var it = self.classes.iterator();
        while (it.next()) |entry| {
            entry.value.deinit();
        }
        self.classes.deinit();
    }
};

pub const ComboMatcher = struct {
    caches: []ComboCache,
    layers: []Layer,
    fuzz_max: usize,
    words_max: usize,
    match_count: usize,
    index: usize,
    allocator: *std.mem.Allocator,
    wordlist: []*const Word,
    output_buffer: [1024]u8,

    const Self = @This();
    const Layer = struct {
        wi: usize,
        stem: *const Word,
        states: ComboCache.TransitionsSet,
    };

    pub fn init(expressions: []*Expression, wordlist: Wordlist, words_max: usize, allocator: *std.mem.Allocator) !Self {
        var caches = try allocator.alloc(ComboCache, expressions.len);
        errdefer allocator.free(caches);

        var words = wordlist.pointer_slice;
        for (expressions) |expr, i| {
            caches[i] = try ComboCache.init(expr, words);
            words = caches[i].nonnull_words.?;
        }
        errdefer {
            // TODO: errdefer free caches incrementally
            for (caches) |*cache| {
                cache.deinit();
            }
        }

        for (caches[0 .. caches.len - 1]) |*cache| {
            try cache.reduceWordlist(words);
        }

        var fuzz_max: usize = 0;
        for (caches) |*cache| {
            fuzz_max = std.math.max(fuzz_max, cache.expression.fuzz);
        }
        log.info("fuzz_max={}", .{fuzz_max});

        var layers = try allocator.alloc(Layer, words_max + 2);
        errdefer allocator.free(layers);

        // TODO: errdefer free states
        for (layers) |*layer| {
            layer.states = try ComboCache.TransitionsSet.init(caches.len * (fuzz_max + 1), allocator);
        }

        layers[0].states.clear();
        for (expressions) |expr, i| {
            expr.matchPartial(&.{.end}, 0, layers[0].states.slice(fuzz_max + 1, i));
        }
        layers[0].wi = 0;

        return Self{
            .caches = caches,
            .layers = layers,
            .fuzz_max = fuzz_max,
            .words_max = words_max,
            .allocator = allocator,
            .wordlist = words,
            .output_buffer = undefined,
            .index = 0,
            .match_count = 0,
        };
    }

    pub fn deinit(self: *Self) void {
        for (self.layers) |*layer| {
            layer.states.free(self.allocator);
        }
        self.allocator.free(self.layers);
        for (self.caches) |*cache| {
            cache.deinit();
        }
        self.allocator.free(self.caches);
    }

    fn formatOutput(self: *Self) []const u8 {
        var writer = std.io.fixedBufferStream(&self.output_buffer);
        var i: usize = 0;
        while (i <= self.index) : (i += 1) {
            _ = writer.write(" ") catch null;
            _ = writer.write(self.layers[i].stem.text) catch null;
        }
        return writer.getWritten()[1..];
    }

    pub fn match(self: *Self) ?[]const u8 {
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

    fn advance(self: *Self, partial_match: bool) bool {
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
};
