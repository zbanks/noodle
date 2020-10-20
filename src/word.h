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

void str_init(struct str * s, const char * c);
void str_term(struct str * s);
const char * str_str(const struct str * s);

struct word {
    struct str canonical;
    struct str original;
    struct str sorted;
    int value;
};

#define PRIWORD "[%s \"%s\" %d %s]"
#define PRIWORDF(w) str_str(&(w).canonical), str_str(&(w).original), (w).value, str_str(&(w).sorted)

void word_init(struct word * w, const char * original, int value);
void word_term(struct word * w);

int word_canonical_cmp(const void * x, const void * y);
int word_canonical_ptrcmp(const void * x, const void * y);
int word_value_cmp(const void * x, const void * y);
int word_value_ptrcmp(const void * x, const void * y);
