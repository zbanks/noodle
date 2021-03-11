const std = @import("std");
const Char = @import("char.zig").Char;
const log = std.log.scoped(.nx);

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

        pub const failure = num_states - 1;
        pub const success = failure - 1;

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
            return self.bitset.set(index);
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
            return std.mem.eql(self, other);
        }

        pub fn isEmpty(self: @This()) bool {
            return self.bitset.findFirstSet() == null;
        }

        pub fn format(self: @This(), comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
            try writer.print("{{", .{});
            var first = true;
            var iter = self.iterator();
            while (iter.next()) |i| {
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
        UnmatchedParentheses,
        UnmatchedSquareBrackets,
        InvalidCharacterClass,
    };

    const State = struct {
        const num_states = 1500; // if @sizeOf(StateSet) > 256, triggers memcpy, gets ~2x slower
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
            .ignore_whitespace = false,
            .ignore_punctuation = false,
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
                if (ss.isEmpty()) {
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
        var state = try self.states.addOne();
        for (state.branches) |*b| {
            b.next_state = 0;
            b.char_bitset = 0;
        }
        state.epsilon_states = State.Set.initEmpty();
        return state;
    }

    fn compile_subexpression(self: *Self, subexpression: []const u8) CompileError!usize {
        var previous_initial_state: State.Index = State.Set.failure;
        var subexpression_initial_state: State.Index = @intCast(State.Index, self.states.items.len);
        var subexpression_final_state: State.Index = State.Set.failure;

        var i: usize = 0;
        while (i < subexpression.len) {
            const c = subexpression[i];
            var nc: Char = Char.fromU8(c);
            switch (c) {
                '\\', '^', '$', ' ' => {},
                ')' => { // End of parethetical expression
                    if (subexpression_final_state != State.Set.failure) {
                        log.info("Subexpression {}\n", .{subexpression_final_state});
                        self.states.items[subexpression_final_state].branches[0].next_state = @intCast(State.Index, self.states.items.len);
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

        if (subexpression_final_state != State.Set.failure) {
            log.info("Subexpression {}\n", .{subexpression_final_state});
            self.states.items[subexpression_final_state].branches[0].next_state = @intCast(State.Index, self.states.items.len);
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

        for (self.states.items) |*state, si| {
            if (!start_states.isSet(@intCast(State.Index, si))) {
                continue;
            }

            for (state.branches) |b| {
                if ((char_bitset & b.char_bitset) != 0) {
                    end_states.set(b.next_state);
                }
            }
        }

        for (self.states.items) |*state, si| {
            if (!end_states.isSet(@intCast(State.Index, si))) {
                continue;
            }
            end_states.setUnion(state.epsilon_states);
        }

        return end_states;
    }
};
