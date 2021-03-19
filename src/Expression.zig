const std = @import("std");
const Char = @import("char.zig").Char;
const Word = @import("Word.zig");
const Wordlist = @import("Wordlist.zig");
const log = std.log.scoped(.Expression);

allocator: *std.mem.Allocator,
states: std.ArrayList(State),
expression: []u8,
ignore_whitespace: bool,
ignore_punctuation: bool,
letters_bitset: Char.Bitset,
fuzz: usize,
next_state_sets: []State.Set,

const Self = @This();

pub const ParseError = error{
    BareModifier,
    UnmatchedParentheses,
    UnmatchedSquareBrackets,
    InvalidCharacterClass,
};

pub const CompileError = error{
    OutOfMemory,
    TooManyStates,
} || ParseError;

pub const State = struct {
    pub const num_states = 256; // if @sizeOf(StateSet) > 256, triggers memcpy, gets ~2x slower
    pub const Set = StateSet(num_states);
    pub const Index = Set.Index;

    const Branch = struct {
        next_state: Index = 0,
        char_bitset: Char.Bitset = 0,
    };

    branches: [2]Branch = [_]Branch{.{}} ** 2,
    epsilon_states: Set = Set.initEmpty(),
};

fn StateSet(comptime num_states: comptime_int) type {
    return struct {
        // TODO: Size at compile time, based on desired max # of states
        bitset: BitSet,

        // For some reason, u16/u32 is faster than u8-u15?
        //pub const Index = std.math.IntFittingRange(0, std.math.max(num_states - 1, std.math.maxInt(u32)));
        pub const Index = std.math.IntFittingRange(0, num_states - 1);
        //pub const Index = u32;
        const BitSet = std.bit_set.StaticBitSet(num_states);
        const Set = @This();

        pub const nonempty = num_states - 1;
        pub const max_normal = num_states - 2;

        /// Creates a bit set with no elements present.
        pub fn initEmpty() Set {
            return .{ .bitset = BitSet.initEmpty() };
        }

        /// Returns true if the bit at the specified index
        /// is present in the set, false otherwise.
        pub fn isSet(self: Set, index: usize) bool {
            return self.bitset.isSet(index);
        }

        /// Adds a specific bit to the bit set
        pub fn set(self: *Set, index: usize) void {
            self.bitset.set(nonempty);
            self.bitset.set(index);
        }

        /// Performs a union of two bit sets, and stores the
        /// result in the first one.  Bits in the result are
        /// set if the corresponding bits were set in either input.
        pub fn setUnion(self: *Set, other: Set) void {
            return self.bitset.setUnion(other.bitset);
        }

        /// Iterates through the items in the set, according to the options.
        /// Modifications to the underlying bit set may or may not be
        /// observed by the iterator.
        const Iterator = @TypeOf(BitSet.iterator(&BitSet.initEmpty(), .{}));
        pub fn iterator(self: *const Set) Iterator {
            return self.bitset.iterator(.{});
        }

        pub fn eql(self: Set, other: Set) bool {
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
        }

        pub fn format(self: @This(), comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
            try writer.print("{{", .{});
            var first = true;
            var iter = self.iterator();
            while (iter.next()) |i| {
                if (i == Set.nonempty) {
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
                if (b.next_state != 0) {
                    _ = try writer.print("(null) -> {}    ", .{b.next_state});
                }

                // These two cases are just to catch potentially-invalid representations
                // (0 is technically a valid state; this just catches _most_ errors)
                std.debug.assert(j + 1 >= s.branches.len or s.branches[j + 1].char_bitset == 0);
                std.debug.assert(b.next_state == 0);
                continue;
            }
            try Char.formatBitset(writer, b.char_bitset);
            try writer.print(" -> ", .{});

            std.debug.assert(b.next_state < self.states.items.len);
            _ = try writer.print("{:3}", .{b.next_state});
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
    state.* = State{};
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
                        .epsilon => unreachable,
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
            '+' => {
                if (previous_initial_state == null) {
                    log.err("parse error: '{}' without preceeding group", .{c});
                    return error.BareModifier;
                }

                var s: *State = try self.addState();
                s.branches[0] = .{
                    .next_state = @intCast(State.Index, previous_initial_state.?),
                    .char_bitset = Char.epsilon.toBitset(),
                };
                s.branches[1] = .{
                    .next_state = @intCast(State.Index, self.states.items.len),
                    .char_bitset = Char.epsilon.toBitset(),
                };
            },
            // TODO: +, ?, |, {,
            else => {
                log.err("Invalid character in noodle expression: '{c}'\n", .{c});
                // raise ...
            },
        }

        i += 1;
    }

    // End of (full) expression
    var s: *State = try self.addState();

    if (subexpression_final_state != null) {
        log.info("Subexpression {}\n", .{subexpression_final_state});
        self.states.items[subexpression_final_state.?].branches[0].next_state = @intCast(State.Index, self.states.items.len);
    }

    return i;
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

            // TODO: We may be able to guarantee that only branches[0] is used?
            // (May need to refactor '_' handling)
            for (state.branches) |b| {
                if ((char_bitset & b.char_bitset) != 0) {
                    end_states.set(b.next_state);

                    std.debug.assert(b.next_state < self.states.items.len);
                    end_states.setUnion(self.states.items[b.next_state].epsilon_states);
                }
            }
        }
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
                    end_states.setUnion(self.states.items[si].epsilon_states);
                }
            }
        }
    }

    return end_states;
}
