#pragma once
#include "prelude.h"
#include "word.h"

struct wordset {
    const struct word ** words;
    size_t words_count;
    size_t words_capacity;
    struct anatree * anatree;
    bool is_canonically_sorted;
    char name[64];
};

void wordset_init(struct wordset * ws, const char * name);
void wordset_add(struct wordset * ws, const struct word * w);
void wordset_sort_value(struct wordset * ws);
void wordset_sort_canonical(struct wordset * ws);
void wordset_term(struct wordset * ws);
const struct word * wordset_get(const struct wordset * ws, size_t i);
const struct word * wordset_find(const struct wordset * ws, const struct str * s);
void wordset_print(struct wordset * ws);

#define WORDLIST_CHUNK_SIZE ((size_t)256)
struct wordlist {
    struct word ** chunks;
    size_t insert_index;
    struct wordset self_set;
};

void wordlist_init(struct wordlist * wl, const char * name);
int wordlist_init_from_file(struct wordlist * wl, const char * filename, bool has_weight);
void wordlist_add(struct wordlist * wl, const char * s, int v);
const struct word * wordlist_ensure_owned(struct wordlist * wl, const struct word * w);
void wordlist_term(struct wordlist * wl);
