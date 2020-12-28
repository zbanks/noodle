#pragma once
#include "prelude.h"

struct word {
    union {
        struct {
            char small[15];
            unsigned char small_flags;
        };
        struct {
            char _padding[7];
            unsigned char large_flags;
            char * large;
        };
    };
};

NOODLE_EXPORT char * word_init(struct word * s, const char * c, size_t len);
NOODLE_EXPORT void word_init_copy(struct word * dst, const struct word * src);
NOODLE_EXPORT void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words);
NOODLE_EXPORT void word_term(struct word * s);

NOODLE_EXPORT const char * word_str(const struct word * s);

unsigned char word_flags(const struct word * s);
void word_flags_set(struct word * s, unsigned char flags);
int word_ptrcmp(const void * x, const void * y);
