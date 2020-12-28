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

struct word {
    struct str str;
    bool owned;
};
_Static_assert(offsetof(struct word, str) == 0, "str must be the first element in struct word");

NOODLE_EXPORT void word_init(struct word * w, const char * original);
NOODLE_EXPORT void word_init_copy(struct word * w_dst, const struct word * w_src);
NOODLE_EXPORT void word_term(struct word * w);
NOODLE_EXPORT const char * word_debug(const struct word * w);
NOODLE_EXPORT const char * word_cstr(const struct word * w);

void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words);
