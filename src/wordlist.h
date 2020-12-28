#pragma once
#include "prelude.h"
#include "time_util.h"
#include "word.h"

struct wordset {
    const struct word ** words;
    size_t words_count;
    size_t words_capacity;
};

NOODLE_EXPORT void wordset_init(struct wordset * ws);
NOODLE_EXPORT void wordset_term(struct wordset * ws);

NOODLE_EXPORT void wordset_print(const struct wordset * ws);
NOODLE_EXPORT const struct word * wordset_get(const struct wordset * ws, size_t i);

void wordset_add(struct wordset * ws, const struct word * w);
const struct word * wordset_find(const struct wordset * ws, const struct word * s);

#define WORDLIST_CHUNK_SIZE ((size_t)256)
struct wordlist {
    struct word ** chunks;
    size_t insert_index;
    struct wordset self_set;
};

NOODLE_EXPORT void wordlist_init(struct wordlist * wl);
NOODLE_EXPORT int wordlist_init_from_file(struct wordlist * wl, const char * filename);
NOODLE_EXPORT void wordlist_term(struct wordlist * wl);
NOODLE_EXPORT const struct word * wordlist_add(struct wordlist * wl, const char * s);

const struct word * wordlist_ensure_owned(struct wordlist * wl, const struct word * w);

// TODO: Maybe unify callbacks & cursors? It's weird they have to passed in together
struct word_callback {
    void (*callback)(struct word_callback * cb, const struct word * w);
    struct cursor * cursor;
};

NOODLE_EXPORT void word_callback_destroy(struct word_callback * wcb);

// Print each word to the log
NOODLE_EXPORT struct word_callback * word_callback_create_print(struct cursor * cursor, size_t limit);
// Add each word to an `output` wordset, using wordlist `buffer` to ensure it is owned
NOODLE_EXPORT struct word_callback * word_callback_create_wordset_add(struct cursor * cursor, struct wordlist * buffer,
                                                                      struct wordset * output);
// This has an O(n^2) component, which is fine for small n (~a few thousand)
// Disable clang-formatting to avoid breaking the super-naive parser in build_cffi.py
// clang-format off
NOODLE_EXPORT struct word_callback * word_callback_create_wordset_add_unique(
        struct cursor * cursor, struct wordlist * buffer, struct wordset * output);
// clang-format on
