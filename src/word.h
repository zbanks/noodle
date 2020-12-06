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

char * str_init(struct str * s, const char * c, size_t len);
void str_init_copy(struct str * dst, const struct str * src);
char * str_init_buffer(struct str * s, size_t len);
void str_term(struct str * s);
const char * str_str(const struct str * s);
int str_cmp(const void * x, const void * y);
int str_ptrcmp(const void * x, const void * y);

#define WORD_TUPLE_N ((size_t)5)
struct word {
    struct str canonical;
    bool is_tuple;
    bool owned;
    union {
        struct {
            int value;
            struct str original;
            struct str sorted;
        };
        const struct word * tuple_words[WORD_TUPLE_N];
    };
};
_Static_assert(offsetof(struct word, canonical) == 0, "canonical must be the first element in struct word");
_Static_assert(sizeof(struct word) <= 4 * sizeof(struct str), "struct word padding/packing is unexpectedly large");

NOODLE_EXPORT void word_init(struct word * w, const char * original, int value);
NOODLE_EXPORT void word_init_copy(struct word * w_dst, const struct word * w_src);
NOODLE_EXPORT void word_term(struct word * w);
NOODLE_EXPORT int word_value(const struct word * w);
NOODLE_EXPORT const char * word_debug(const struct word * w);
NOODLE_EXPORT const char * word_canonical(const struct word * w);
NOODLE_EXPORT const char * word_sorted(const struct word * w);

int word_value_cmp(const void * x, const void * y);
int word_value_ptrcmp(const void * x, const void * y);

void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words);
