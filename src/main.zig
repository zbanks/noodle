const std = @import("std");
const log = std.debug.print;

const Char = enum(u5) {
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

    const letters_bitset: u32 = comptime {
        var b: u32 = 0;
        var j: u8 = 'a';
        while (j <= 'z') {
            b |= Char.fromU8(j).toBitset();
            j += 1;
        }
        return b;
    };

    pub fn toU8(self: Char) u8 {
        return switch (self) {
            .end => '$',
            .epsilon => '*',
            .punct => '\'',
            .space => '_',
            else => @intCast(u8, @enumToInt(self) - @enumToInt(Char.a)) + 'a',
        };
    }

    pub fn fromU8(u: u8) Char {
        return switch (u) {
            0 => .end,
            ' ', '_' => .space,
            'A'...'Z' => @intToEnum(Char, @intCast(u5, u - 'A') + @enumToInt(Char.a)),
            'a'...'z' => @intToEnum(Char, @intCast(u5, u - 'a') + @enumToInt(Char.a)),
            else => .punct,
        };
    }

    pub fn toBitset(self: Char) u32 {
        const one: u32 = 1;
        return one << @enumToInt(self);
    }

    pub fn translate(text: []const u8, chars: []Char) void {
        std.debug.assert(text.len + 1 <= chars.len);
        for (text) |t, i| {
            chars[i] = Char.fromU8(t);
        }
        chars[text.len] = Char.end;
    }

    pub fn writeBitset(writer: anytype, bitset: u32) !void {
        try writer.print("[", .{});
        var i: u5 = 0;
        while (true) {
            var c = @intToEnum(Char, i);
            if ((bitset & c.toBitset()) != 0) {
                try writer.print("{c}", .{c.toU8()});
            }
            if (c == .z) {
                break;
            }
            i += 1;
        }
        try writer.print("]", .{});
    }
};

const StateSet = struct {
    // TODO: Size at compile time, based on desired max # of states
    xs: u64,

    const one: u64 = 1;

    pub const failure = 63; //std.math.maxInt(u16);
    pub const success = failure - 1;
    pub const empty = StateSet{ .xs = 0 };

    pub fn add(self: *StateSet, i: u16) void {
        //std.debug.assert(i < 64);
        self.xs |= (one << @intCast(u6, i));
    }

    pub fn addSet(self: *StateSet, other: StateSet) void {
        self.xs |= other.xs;
    }

    pub fn has(self: StateSet, i: u16) bool {
        //std.debug.assert(i < 64);
        return (self.xs & (one << @intCast(u6, i))) != 0;
    }

    pub fn eql(self: StateSet, other: StateSet) bool {
        return std.meta.eql(self, other);
    }

    pub fn format(self: StateSet, comptime fmt: []const u8, options: std.fmt.FormatOptions, writer: anytype) !void {
        try writer.print("{{", .{});
        var first = true;
        var i: u16 = 0;
        while (i < 64) : (i += 1) {
            if (self.has(i)) {
                if (!first) {
                    try writer.print(",", .{});
                }
                try writer.print("{}", .{i});
                first = false;
            }
        }
        try writer.print("}}", .{});
    }
};

const State = struct {
    const branch_count = 2;

    next_state: [branch_count]u16,
    char_bitset: [branch_count]u32,
    // TODO
    epsilon_states: StateSet,

    const branches: [branch_count]usize = comptime {
        // TODO: This feels absurdly complex, is there something in the stdlib for this?
        var bs: [branch_count]usize = [_]usize{0} ** branch_count;
        var b = 0;
        while (b < branch_count) {
            bs[b] = b;
            b += 1;
        }
        return bs;
    };
};

const Nx = struct {
    allocator: *std.mem.Allocator,
    states: std.ArrayList(State),
    expression: []u8,
    ignore_whitespace: bool,
    ignore_punctuation: bool,
    letters_bitset: u32,
    fuzz: usize,
    next_state_sets: []StateSet,

    const CompileError = error{
        OutOfMemory,
        UnmatchedParentheses,
        UnmatchedSquareBrackets,
        InvalidCharacterClass,
    };

    pub fn init(expression: []const u8, fuzz: usize, allocator: *std.mem.Allocator) !Nx {
        const expr = try std.mem.dupe(allocator, u8, expression);
        errdefer allocator.free(expr);

        var next_state_sets = try allocator.alloc(StateSet, fuzz + 1);
        errdefer allocator.free(next_state_sets);

        var nx = Nx{
            .allocator = allocator,
            .states = std.ArrayList(State).init(allocator),
            .expression = expr,
            .ignore_whitespace = false,
            .ignore_punctuation = false,
            .letters_bitset = Char.letters_bitset,
            .fuzz = fuzz,
            .next_state_sets = next_state_sets,
        };
        errdefer nx.states.deinit();

        const len = try nx.compile_subexpression(expression);
        if (len != expression.len) {
            return error.UnmatchedParentheses;
        }

        // Calculate epsilon transitions
        for (nx.states.items) |*s, i| {
            var next_ss = s.epsilon_states;
            for (State.branches) |j| {
                if ((Char.epsilon.toBitset() & s.char_bitset[j]) != 0) {
                    next_ss.add(s.next_state[j]);
                }
            }
            while (true) {
                var ss = next_ss;
                for (nx.states.items) |s2, si| {
                    if (!next_ss.has(@intCast(u16, si))) {
                        continue;
                    }
                    for (State.branches) |j| {
                        if ((Char.epsilon.toBitset() & s2.char_bitset[j]) != 0) {
                            ss.add(s2.next_state[j]);
                        }
                    }
                }
                if (ss.eql(next_ss)) {
                    break;
                }
                next_ss.addSet(ss);
            }
            s.epsilon_states = next_ss;
        }
        for (nx.states.items) |*s, i| {
            for (State.branches) |j| {
                if (Char.epsilon.toBitset() == s.char_bitset[j]) {
                    s.char_bitset[j] = 0;
                    s.next_state[j] = 0;
                }
            }
        }

        return nx;
    }

    pub fn deinit(self: *Nx) void {
        self.allocator.free(self.expression);
        self.allocator.free(self.next_state_sets);
        self.states.deinit();
    }

    pub fn printNfa(self: Nx) !void {
        var list = std.ArrayList(u8).init(self.allocator);
        defer list.deinit();
        var writer = list.writer();

        _ = try writer.print("NX NFA: {} states\n", .{self.states.items.len});
        for (self.states.items) |s, i| {
            _ = try writer.print("    {:3}: ", .{i});
            for (State.branches) |j| {
                if (s.char_bitset[j] == 0) {
                    // These two cases are just to catch potentially-invalid representations
                    if (j + 1 < State.branch_count and s.char_bitset[j + 1] != 0) {
                        _ = try writer.print("(missing {})    ", .{j});
                    }
                    // 0 is technically a valid state; this just catches _most_ errors
                    if (s.next_state[j] != 0) {
                        _ = try writer.print("(null) -> {}    ", .{s.next_state[j]});
                    }
                    continue;
                }
                try Char.writeBitset(writer, s.char_bitset[j]);
                try writer.print(" -> ", .{});
                if (s.next_state[j] > StateSet.success) {
                    _ = try writer.print("!!!{}", .{s.next_state[j]});
                } else if (s.next_state[j] == StateSet.success) {
                    _ = try writer.print("MATCH", .{});
                } else {
                    _ = try writer.print("{:3}", .{s.next_state[j]});
                }
                _ = try writer.print("     ", .{});
            }
            if (!s.epsilon_states.eql(StateSet.empty)) {
                _ = try writer.print("* -> {}", .{s.epsilon_states});
            }
            _ = try writer.print("\n", .{});
        }
        _ = try writer.print("\n", .{});
        log("{s}", .{list.items});
    }

    fn addState(self: *Nx) !*State {
        var state = try self.states.addOne();
        state.next_state = [_]u16{0} ** State.branch_count;
        state.char_bitset = [_]u32{0} ** State.branch_count;
        state.epsilon_states = StateSet.empty;
        return state;
    }

    fn compile_subexpression(self: *Nx, subexpression: []const u8) CompileError!usize {
        var previous_initial_state: u16 = StateSet.failure;
        var subexpression_initial_state: u16 = @intCast(u16, self.states.items.len);
        var subexpression_final_state: u16 = StateSet.failure;

        var i: usize = 0;
        while (i < subexpression.len) {
            const c = subexpression[i];
            var nc: Char = Char.fromU8(c);
            switch (c) {
                '\\', '^', '$', ' ' => {},
                ')' => { // End of parethetical expression
                    if (subexpression_final_state != StateSet.failure) {
                        log("Subexpression {}\n", .{subexpression_final_state});
                        self.states.items[subexpression_final_state].next_state[0] = @intCast(u16, self.states.items.len);
                    }
                    return i + 1;
                },
                '(' => { // Start of new parethetical expression
                    previous_initial_state = @intCast(u16, self.states.items.len);
                    const sub_len = try self.compile_subexpression(subexpression[i + 1 ..]);
                    if (subexpression[i + sub_len] != ')') {
                        return error.UnmatchedParentheses;
                    }
                    i += sub_len;
                },
                'A'...'Z', 'a'...'z' => { // "Normal" letters
                    var s: *State = try self.addState();
                    s.next_state[0] = @intCast(u16, self.states.items.len);
                    s.char_bitset[0] = nc.toBitset();

                    previous_initial_state = @intCast(u16, self.states.items.len - 1);
                },
                '_' => { // Explicit space
                    var s: *State = try self.addState();
                    s.next_state[0] = @intCast(u16, self.states.items.len);
                    s.char_bitset[0] = Char.space.toBitset();

                    // XXX: This allows spaces to absorb an arbitrary number of spaces
                    // effectively replacing each "_" with "_+"
                    // This means we don't need to trim spaces from words when doing matches
                    s.next_state[1] = @intCast(u16, self.states.items.len - 1);
                    s.char_bitset[1] = Char.space.toBitset();

                    previous_initial_state = @intCast(u16, self.states.items.len - 1);
                },
                '\'', '-' => { // Explicit punctuation
                    var s: *State = try self.addState();
                    s.next_state[0] = @intCast(u16, self.states.items.len);
                    s.char_bitset[0] = Char.punct.toBitset();

                    previous_initial_state = @intCast(u16, self.states.items.len - 1);
                },
                '.' => { // Match any 1 character
                    var s: *State = try self.addState();
                    s.next_state[0] = @intCast(u16, self.states.items.len);
                    s.char_bitset[0] = Char.letters_bitset;
                    previous_initial_state = @intCast(u16, self.states.items.len - 1);
                },
                '[' => { // Character class
                    i += 1;
                    var inverse: bool = false;
                    if (subexpression[i] == '^') {
                        inverse = true;
                        i += 1;
                    }

                    var s: *State = try self.addState();
                    s.next_state[0] = @intCast(u16, self.states.items.len);
                    s.char_bitset[0] = 0;

                    // TODO: support "[a-z]" syntax
                    while (i < subexpression.len and subexpression[i] != ']') {
                        const sc = subexpression[i];
                        const sn = Char.fromU8(sc);
                        s.char_bitset[0] |= switch (sn) {
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
                        s.char_bitset[0] ^= self.letters_bitset;
                    }
                    previous_initial_state = @intCast(u16, self.states.items.len - 1);
                },
                // TODO: *, +, ?, |, {,
                else => {
                    log("Invalid character in nx expression: '{c}'\n", .{c});
                    // raise ...
                },
            }

            i += 1;
        }

        // End of (full) expression
        var s: *State = try self.addState();
        s.next_state[0] = StateSet.success;
        s.char_bitset[0] = Char.end.toBitset();

        if (subexpression_final_state != StateSet.failure) {
            log("Subexpression {}\n", .{subexpression_final_state});
            self.states.items[subexpression_final_state].next_state[0] = @intCast(u16, self.states.items.len);
        }

        return i;
    }

    pub fn match(self: *Nx, input_text: []const u8, n_errors: usize) !?usize {
        var input_chars: []Char = try self.allocator.alloc(Char, input_text.len + 1);
        defer self.allocator.free(input_chars);
        Char.translate(input_text, input_chars);

        // `epsilon_states` are accounted for *after* "normal" states in `nx_match_transition`
        // Therefore it is important to include them here for correctness
        var ss = StateSet.empty;
        ss.add(0);
        ss.addSet(self.states.items[0].epsilon_states);

        return self.matchFuzzy(input_chars, ss, n_errors);
    }

    pub fn matchPartial(self: *Nx, input: []const Char, initial_state: u16, state_sets: []StateSet) void {
        // Start with an initial `state_sets` containing only `initial_state` with 0 fuzz
        std.debug.assert(state_sets.len >= self.fuzz + 1);
        var i: usize = 0;
        while (i < self.fuzz + 1) : (i += 1) {
            state_sets[i] = StateSet.empty;
        }
        state_sets[0].add(initial_state);

        // Add all `epsilon_states`, which are the states reachable from `initial_state`
        // without consuming a character from `buffer`
        state_sets[0].addSet(self.states.items[initial_state].epsilon_states);

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
                self.next_state_sets[i+1].addSet(state_sets[i]);

                // Change
                var change_set = self.matchTransition(self.letters_bitset, state_sets[i]);
                self.next_state_sets[i+1].addSet(change_set);

                // Insertion
                var insertion_set = self.matchTransition(c.toBitset(), change_set);
                self.next_state_sets[i+1].addSet(insertion_set);
            }

            // Shift next_state_sets into state_sets
            std.mem.copy(StateSet, state_sets, self.next_state_sets);
            
            // We can terminate early if there are no possible valid states
            for (state_sets) |s| {
                if (!s.eql(StateSet.empty)) {
                    break;
                }
            } else {
                return;
            }
        }
    }

    /// Perform a fuzzy match against an NX NFA.
    /// From a given `initial_state_set`, return the number of changes required for `input` to match.
    /// Returns `null` if the number of errors would exceed `n_errors`, or `0` on an exact match.
    /// Can be used for exact matches by setting `n_errors` to `0`.
    pub fn matchFuzzy(self: *Nx, input: []const Char, initial_state_set: StateSet, n_errors: usize) ?usize {
        // If the initial `state_set` is already a match, we're done!
        if (initial_state_set.has(StateSet.success)) {
            return 0;
        }

        var state_set = initial_state_set;

        // Keep track of which states are reachable with *exactly* 1 error, initially empty
        var error_state_set = StateSet.empty;

        // Iterate over the characters in `buffer` (exactly 1 character per iteration)
        for (input) |c, i| {
            var next_state_set = self.matchTransition(c.toBitset(), state_set);
            var next_error_set = StateSet.empty;

            if (next_state_set.has(StateSet.success)) {
                std.debug.assert(c == .end);
                return 0;
            }

            if (n_errors > 0) {
                next_error_set = self.matchTransition(c.toBitset(), error_state_set);

                if (next_error_set.has(StateSet.success)) {
                    std.debug.assert(c == .end);
                    return 1;
                }

                if (c != .end) {
                    // Deletion
                    next_error_set.addSet(state_set);
                    // Change
                    const es = self.matchTransition(self.letters_bitset, state_set);
                    next_error_set.addSet(es);
                }

                // XXX: handle two inserts in a row
                // Insertion
                var es = self.matchTransition(self.letters_bitset, state_set);
                es = self.matchTransition(c.toBitset(), es);
                next_error_set.addSet(es);
            }

            if (next_state_set.eql(StateSet.empty)) {
                if (n_errors > 0) {
                    const rc = self.matchFuzzy(input[i+1..], next_error_set, n_errors - 1);
                    if (rc) |errors| {
                        return errors + 1;
                    }
                }

                return null;
            }

            if (c == .end) {
                log("Buffer is end: next_state_set = {}", .{next_state_set});
                unreachable;
            }

            state_set = next_state_set;
            error_state_set = next_error_set;
        }

        unreachable;
    }

    fn matchTransition(self: *Nx, char_bitset: u32, start_states: StateSet) StateSet {
        var end_states = StateSet.empty;

        if (start_states.eql(StateSet.empty)) {
            return end_states;
        }

        for (self.states.items) |state, si| {
            if (!start_states.has(@intCast(u16, si))) {
                continue;
            }

            for (State.branches) |j| {
                if ((char_bitset & state.char_bitset[j]) != 0) {
                    end_states.add(state.next_state[j]);
                }
            }
        }

        for (self.states.items) |state, si| {
            if (!end_states.has(@intCast(u16, si))) {
                continue;
            }
            end_states.addSet(state.epsilon_states);
        }

        return end_states;
    }
};

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer {
        const leaked = gpa.deinit();
    }

    const x = Char.fromU8('d');
    //var nx = try Nx.init("e.p[^aeiou][aeiou](ss)iontest", &gpa.allocator);
    var nx = try Nx.init("expressiontest", 2, &gpa.allocator);
    defer {
        nx.deinit();
    }
    try nx.printNfa();
    std.debug.print("Hello, {c}! bitset={x}\n", .{ x.toU8(), Char.letters_bitset });

    const input_text = "expresontest";
    var input_chars: []Char = try gpa.allocator.alloc(Char, input_text.len + 1);
    defer gpa.allocator.free(input_chars);
    Char.translate(input_text, input_chars);


    var timer = try std.time.Timer.start();
    timer.reset();
    const cycles: usize = 100000;
    var i: usize = 0; 
    while (i < cycles) {
        var ss = StateSet.empty;
        ss.add(0);
        ss.addSet(nx.states.items[0].epsilon_states);

        _ = nx.matchFuzzy(input_chars, ss, 0);
        i += 1;
    }
    var dt = timer.read();
    const m1 = try nx.match("expressiontest", 3);
    const m2 = try nx.match("ekpresiontqest", 3);
    log("Match results: {}, {}; dt={} for {} cycles ({} ns/cycle)\n", .{m1, m2, dt, cycles, dt / cycles});

    var state_sets: [3]StateSet = .{StateSet.empty} ** 3;
    nx.matchPartial(input_chars, 0, &state_sets);
    log("Partial results: {a}\n", .{state_sets});
}
