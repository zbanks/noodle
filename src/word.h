#pragma once
#include "prelude.h"

struct str {
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

char * str_init(struct str * s, const char * c, size_t len);
void str_init_copy(struct str * dst, const struct str * src);
char * str_init_buffer(struct str * s, size_t len);
void str_term(struct str * s);

const char * str_str(const struct str * s);
unsigned char str_flags(const struct str * s);
void str_flags_set(struct str * s, unsigned char flags);

int str_ptrcmp(const void * x, const void * y);

// `struct word` used to cache extra translations, etc, of the base string
// (It is now a transparent wrapper)
struct word {
    struct str str;
};

NOODLE_EXPORT void word_init(struct word * w, const char * original);
NOODLE_EXPORT void word_init_copy(struct word * w_dst, const struct word * w_src);
NOODLE_EXPORT void word_term(struct word * w);
NOODLE_EXPORT const char * word_debug(const struct word * w);
NOODLE_EXPORT const char * word_cstr(const struct word * w);

void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words);
