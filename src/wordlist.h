#pragma once
#include "cursor.h"
#include "prelude.h"
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
