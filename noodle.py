# Pythonic wrapper around the noodle C API, via the bare CFFI wrapper noodle_ffi

from noodle_ffi import lib as noodle_lib
from noodle_ffi import ffi
import itertools

__all__ = [
    "Word",
    "WordSet",
    "WordList",
    "WordSetAndBuffer",
    "WordCallback" "Nx",
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
    def new(cls, string, value=1):
        allocated_word = ffi.new("struct word *")
        noodle_lib.word_init(allocated_word, string.encode("utf-8"), value)
        return cls(allocated_word, _owned=True)

    @property
    def value(self):
        return noodle_lib.word_value(self.p)

    @property
    def canonical(self):
        return ffi_string(noodle_lib.word_canonical(self.p))

    def debug(self):
        return ffi_string(noodle_lib.word_debug(self.p))

    def __len__(self):
        return len(self.canonical)

    def __str__(self):
        return self.canonical

    def __repr__(self):
        return '<Word: {} "{}" {}>'.format(self.canonical, self.debug(), self.value)


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
    def new(cls, name="(anonymous)"):
        allocated_ws = ffi.new("struct wordset *")
        noodle_lib.wordset_init(allocated_ws, name.encode("utf-8"))
        return cls(allocated_ws, _owned=True)

    @property
    def name(self):
        return ffi_string(self.p.name)

    def __len__(self):
        return self.p.words_count

    def __getitem__(self, index):
        return Word(noodle_lib.wordset_get(self.p, index))

    def __iter__(self):
        for i in range(len(self)):
            yield self[i]

    def sort_value(self):
        noodle_lib.wordset_sort_value(self.p)

    def sort_canonical(self):
        noodle_lib.wordset_sort_canonical(self.p)

    def __repr__(self):
        return '<{}: "{}">'.format(self.__class__.__name__, self.debug())

    def debug(self):
        sample = " ".join(str(x) for x in itertools.islice(self, 20))
        suffix = "" if len(self) < 20 else "..."
        return '"{}" ({}): {}{}'.format(self.name, len(self), sample, suffix)


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
    def new(cls, name="(anonymous)"):
        allocated = ffi.new("struct wordlist *")
        noodle_lib.wordlist_init(allocated, name.encode("utf-8"))
        return cls(allocated, _owned=True)

    @classmethod
    def new_from_file(cls, filename, has_values=False):
        allocated = ffi.new("struct wordlist *")
        noodle_lib.wordlist_init_from_file(
            allocated, filename.encode("utf-8"), has_values
        )
        return cls(allocated, _owned=True)

    @property
    def name(self):
        return ffi_string(self.p.name)

    @property
    def wordset(self):
        return WordSet(ffi.addressof(self.p, "self_set"))

    def add(self, word_string, value=1):
        assert isinstance(word_string, str)
        w = noodle_lib.wordlist_add(self.p, word_string.encode("utf-8"), value)
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

    @classmethod
    def new_to_wordset(cls, cursor, buffer_list, output_set, unique=True):
        constructor = (
            noodle_lib.word_callback_create_wordset_add
            if unique
            else noodle_lib.word_callback_create_wordset_add_unique
        )
        wcb = constructor(cursor.p, buffer_list.p, output_set.p)
        assert wcb, ValueError
        return cls(wcb)

    @classmethod
    def new_print(cls, cursor, limit=None):
        if limit is None:
            limit = 0
        wcb = noodle_lib.word_callback_create_print(cursor.p, limit)
        assert wcb, ValueError
        return cls(wcb)


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


def nx_combo_multi(
    nxs, input_wordset, n_words=2, cursor=None, output_name=None, output=None
):
    assert all(isinstance(nx, Nx) for nx in nxs)
    assert input_wordset
    assert n_words <= 5, "Maximum number of words in combo_multi is 5"

    if isinstance(input_wordset, WordList):
        input_wordset = input_wordset.wordset
    if cursor is None:
        cursor = Cursor.new(now_ns() + 1e9, 1e5)
    if output_name is None:
        output_name = "results of nx combo {}".format(n_words)
    if output is None:
        output = WordSetAndBuffer(name=output_name)

    callback = WordCallback.new_to_wordset(cursor, output.wordlist, output, unique=True)
    nxps = ffi.new("struct nx *[]", [nx.p for nx in nxs])

    noodle_lib.nx_combo_multi(
        nxps, len(nxs), input_wordset.p, n_words, cursor.p, callback.p
    )
    return output


class Cursor:
    __slots__ = ["p"]

    def __init__(self, pointer):
        assert pointer
        self.p = pointer

    @classmethod
    def new(cls, *args, **kwargs):
        cursor = ffi.new("struct cursor *")
        noodle_lib.cursor_init(cursor)
        pycursor = cls(cursor)
        pycursor.set_deadline(*args, **kwargs)
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


class WordSetAndBuffer(WordSet):
    __slots__ = ["p", "wordlist"]

    def __init__(self, name="(anonymous)"):
        allocated_ws = ffi.new("struct wordset *")
        noodle_lib.wordset_init(allocated_ws, name.encode("utf-8"))

        self.p = allocated_ws
        self.wordlist = WordList.new()

    def __del__(self):
        noodle_lib.wordset_term(self.p)


def error_get_log():
    return ffi_string(noodle_lib.error_get_log())


def now_ns():
    return noodle_lib.now_ns()


def test():
    w = Word.new("Hello, world!")
    print("word:", str(w), repr(w))

    wl = WordList.new_from_file("/usr/share/dict/words")
    wl.add("Hello, world!", 2000)
    wl.wordset.sort_value()
    print(wl.debug())
    print("error log:", repr(error_get_log()))


if __name__ == "__main__":
    test()
