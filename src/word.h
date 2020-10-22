#pragma once
#include "prelude.h"

struct str {
    union {
        char small[16];
        struct {
            char _padding[8];
            char * large;
        };
    };
};

void str_init(struct str * s, const char * c, size_t len);
void str_term(struct str * s);
const char * str_str(const struct str * s);
int str_cmp(const void * x, const void * y);
int str_ptrcmp(const void * x, const void * y);

struct word {
    struct str canonical;
    struct str original;
    struct str sorted;
    int value;
};
_Static_assert(offsetof(struct word, canonical) == 0, "canonical must be the first element in struct word");

#define PRIWORD "[%s \"%s\" %d %s]"
#define PRIWORDF(w) str_str(&(w).canonical), str_str(&(w).original), (w).value, str_str(&(w).sorted)

void word_init(struct word * w, const char * original, int value);
void word_term(struct word * w);

int word_value_cmp(const void * x, const void * y);
int word_value_ptrcmp(const void * x, const void * y);

#define WORDTUPLE_N ((size_t)5)
struct wordtuple {
    struct str canonical;
    const struct word * words[WORDTUPLE_N];
};
_Static_assert(sizeof(struct wordtuple) == sizeof(struct word),
               "struct wordtuple and struct word must be the same size; adjust WORDTUPLE_N");

void wordtuple_init(struct wordtuple * wt, const struct word * const * words, size_t n_words);
void wordtuple_term(struct wordtuple * wt);
const char * wordtuple_original(struct wordtuple * wt);
