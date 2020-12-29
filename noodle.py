# Pythonic wrapper around the noodle C API, via the bare CFFI wrapper noodle_ffi

from noodle_ffi import lib as noodle_lib
from noodle_ffi import ffi
import itertools

__all__ = [
    "Word",
    "WordSet",
    "WordList",
    "WordSetAndBuffer",
    "Nx",
    "Cursor",
    "error_get_log",
    "now_ns",
    "nx_combo_multi",
]

SIZE_T_MAX = (1 << 64) - 1


def ffi_string(char_p):
    return ffi.string(char_p).decode("utf-8")


class Word:
    __slots__ = ["p", "_owned"]

    def __init__(self, pointer, _owned=False):
        assert pointer
        self.p = pointer
        self._owned = _owned

    def __del__(self):
        if self._owned:
            noodle_lib.word_term(self.p)

    @classmethod
    def new(cls, string):
        allocated_word = ffi.new("struct word *")
        noodle_lib.word_init(allocated_word, string.encode("utf-8"))
        return cls(allocated_word, _owned=True)

    def __str__(self):
        return ffi_string(noodle_lib.word_str(self.p))

    def debug(self):
        return str(self)

    def __len__(self):
        return len(str(self))

    def __repr__(self):
        return '<{}: "{}">'.format(self.__class__.__name__, self.debug())


class WordSet:
    __slots__ = ["p", "_owned"]

    def __init__(self, pointer, _owned=False):
        assert pointer
        self.p = pointer
        self._owned = _owned

    def __del__(self):
        if self._owned:
            noodle_lib.wordset_term(self.p)

    @classmethod
    def new(cls):
        allocated_ws = ffi.new("struct wordset *")
        noodle_lib.wordset_init(allocated_ws)
        return cls(allocated_ws, _owned=True)

    def __len__(self):
        return self.p.words_count

    def __getitem__(self, index):
        return Word(noodle_lib.wordset_get(self.p, index))

    def __iter__(self):
        for i in range(len(self)):
            yield self[i]

    def __repr__(self):
        return '<{}: "{}">'.format(self.__class__.__name__, self.debug())

    def debug(self):
        sample = " ".join(str(x) for x in itertools.islice(self, 20))
        suffix = "" if len(self) < 20 else "..."
        return "Wordset {}: {}{}".format(len(self), sample, suffix)


class WordList:
    __slots__ = ["p", "_owned"]

    def __init__(self, pointer, _owned=False):
        assert pointer
        self.p = pointer
        self._owned = _owned

    def __del__(self):
        if self._owned:
            noodle_lib.wordlist_term(self.p)

    @classmethod
    def new(cls):
        allocated = ffi.new("struct wordlist *")
        noodle_lib.wordlist_init(allocated)
        return cls(allocated, _owned=True)

    @classmethod
    def new_from_file(cls, filename):
        allocated = ffi.new("struct wordlist *")
        noodle_lib.wordlist_init_from_file(allocated, filename.encode("utf-8"))
        return cls(allocated, _owned=True)

    @property
    def wordset(self):
        return WordSet(ffi.addressof(self.p, "self_set"))

    def add(self, word_string):
        assert isinstance(word_string, str)
        w = noodle_lib.wordlist_add(self.p, word_string.encode("utf-8"))
        return Word(w)

    def __repr__(self):
        return '<{}: "{}">'.format(self.__class__.__name__, self.debug())

    def debug(self):
        return self.wordset.debug()


class WordCallback:
    __slots__ = ["p"]

    def __init__(self, pointer):
        assert pointer
        self.p = pointer

    def __del__(self):
        noodle_lib.word_callback_destroy(self.p)


class Nx:
    __slots__ = ["p"]

    def __init__(self, pointer):
        assert pointer
        self.p = pointer

    def __del__(self):
        noodle_lib.nx_destroy(self.p)

    @classmethod
    def new(cls, expr):
        n = noodle_lib.nx_compile(expr.encode("utf-8"))
        assert n, ValueError
        return cls(n)

    def __str__(self):
        return self.debug()

    def __repr__(self):
        return self.debug()

    def debug(self):
        return ffi_string(self.p.expression)

    def match(self, test_string, n_errors=0):
        # Returns the number of errors. 0 is an exact match. None if the errors was greater than the threshold
        rc = noodle_lib.nx_match(self.p, test_string.encode("utf-8"), n_errors)
        if rc < 0:
            return None
        return rc


class WordSetAndBuffer(WordSet):
    __slots__ = ["p", "wordlist"]

    def __init__(self):
        allocated_ws = ffi.new("struct wordset *")
        noodle_lib.wordset_init(allocated_ws)

        self.p = allocated_ws
        self.wordlist = WordList.new()

    def __del__(self):
        noodle_lib.wordset_term(self.p)


class Cursor:
    __slots__ = ["p"]

    def __init__(self, pointer, *args, **kwargs):
        assert pointer
        self.p = pointer
        self.set_deadline(*args, **kwargs)

    @classmethod
    def new(cls, *args, **kwargs):
        cursor = ffi.new("struct cursor *")
        noodle_lib.cursor_init(cursor)
        pycursor = cls(cursor, *args, **kwargs)
        return pycursor

    @classmethod
    def new_print(cls, limit=None, **kwargs):
        if limit is None:
            limit = 0
        cursor = ffi.new("struct cursor *")
        noodle_lib.cursor_init_print(cursor, limit)
        pycursor = cls(cursor, **kwargs)
        return pycursor

    @classmethod
    def new_to_wordset(cls, buffer_list, output_set, unique=True, **kwargs):
        cursor = ffi.new("struct cursor *")
        noodle_lib.cursor_init_wordset(cursor, buffer_list.p, output_set.p, unique)
        pycursor = cls(cursor, **kwargs)
        return pycursor

    def __str__(self):
        return self.debug()

    def set_deadline(self, deadline_ns=None, deadline_output_index=None):
        if deadline_ns is None:
            deadline_ns = self.p.deadline_ns
        if deadline_output_index is None:
            deadline_output_index = self.p.deadline_output_index
        noodle_lib.cursor_set_deadline(
            self.p, int(deadline_ns), int(deadline_output_index)
        )

    def debug(self):
        return ffi_string(noodle_lib.cursor_debug(self.p))

    def is_done(self):
        return (self.p.input_index == self.p.total_input_items) or (
            self.p.output_index == self.p.deadline_output_index
        )


def nx_combo_multi(nxs, input_wordset, n_words=2, cursor=None, output=None):
    assert all(isinstance(nx, Nx) for nx in nxs)
    assert input_wordset
    assert n_words <= 10, "Maximum number of words in combo_multi is 10"

    if isinstance(input_wordset, WordList):
        input_wordset = input_wordset.wordset
    if output is None:
        output = WordSetAndBuffer()
    if cursor is None:
        cursor = Cursor.new_to_wordset(
            output.wordlist,
            output,
            unique=True,
            deadline_ns=now_ns() + 1e9,
            deadline_output_index=1e5,
        )

    nxps = ffi.new("struct nx *[]", [nx.p for nx in nxs])

    noodle_lib.nx_combo_multi(nxps, len(nxs), input_wordset.p, n_words, cursor.p)
    return output


def error_get_log():
    return ffi_string(noodle_lib.error_get_log())


def now_ns():
    return noodle_lib.now_ns()


def test():
    w = Word.new("Hello, world!")
    print("word:", str(w), repr(w))

    wl = WordList.new_from_file("/usr/share/dict/words")
    wl.add("Hello, world!")
    print(wl.debug())
    print("error log:", repr(error_get_log()))


if __name__ == "__main__":
    test()
