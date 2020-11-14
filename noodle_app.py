#!/usr/bin/env python

import noodle_ffi

w = noodle_ffi.ffi.new("struct word *")
noodle_ffi.lib.word_init(w, b"Hello, world!", 10)
print("word:", noodle_ffi.ffi.string(noodle_ffi.lib.word_debug(w)))

wl = noodle_ffi.ffi.new("struct wordlist *")
noodle_ffi.lib.wordlist_init_from_file(wl, b"/usr/share/dict/words", False)
# noodle_ffi.lib.wordlist_init_from_file(wl, b"consolidated.txt", False)

ws = noodle_ffi.ffi.addressof(wl, "self_set")
noodle_ffi.lib.wordset_sort_canonical(ws)
noodle_ffi.lib.wordset_print(ws)
