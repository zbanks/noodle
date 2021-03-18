const std = @import("std");
const Char = @import("char.zig").Char;
const Word = @import("Word.zig");
const log = std.log.scoped(.Wordlist);

allocator: *std.mem.Allocator,
words: std.ArrayList(Word),
pointer_slice: []*const Word,

pub const Error = error{} || std.mem.Allocator.Error;
pub const FileError = Error || error{StreamTooLong} || std.fs.File.OpenError || std.fs.File.ReadError;

const Self = @This();

pub fn initEmpty(allocator: *std.mem.Allocator) Self {
    return .{
        .allocator = allocator,
        .words = std.ArrayList(Word).init(allocator),
        .pointer_slice = undefined,
    };
}

pub fn deinit(self: Self) void {
    self.allocator.free(self.pointer_slice);
    for (self.words.items) |word| {
        self.allocator.free(word.text);
    }
    self.words.deinit();
}

pub fn initFromFile(filename: []const u8, allocator: *std.mem.Allocator) FileError!Self {
    var self = Self.initEmpty(allocator);
    errdefer self.deinit();

    var file = try std.fs.openFileAbsolute(filename, .{});
    defer file.close();

    var reader = std.io.bufferedReader(file.reader()).reader();

    const max_length: usize = 1024;
    while (try reader.readUntilDelimiterOrEofAlloc(allocator, '\n', max_length)) |line| {
        errdefer allocator.free(line);

        // XXX: Drop single-letter "words" - this should be done on the input, not here
        if (line.len == 1 and !(line[0] == 'a' or line[0] == 'I')) {
            allocator.free(line);
            continue;
        }

        var word = try self.words.addOne();
        word.text = line;
    }
    // TODO: cleanup words on error

    self.pointer_slice = try allocator.alloc(*const Word, self.words.items.len);
    errdefer allocator.free(self.pointer_slice);

    std.sort.sort(Word, self.words.items, {}, Word.compareLengthDesc);

    for (self.words.items) |*word, i| {
        self.pointer_slice[i] = word;
    }

    log.debug("Created wordlist from file {s} with {} words\n", .{ filename, self.words.items.len });

    return self;
}

test "Wordlist.initFromFile" {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();

    var wordlist = try Self.initFromFile("/usr/share/dict/words", &gpa.allocator);
    defer wordlist.deinit();

    std.testing.expect(wordlist.words.items.len > 100);
}
