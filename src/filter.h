#pragma once
#include "prelude.h"
#include "wordlist.h"

enum filter_type {
    FILTER_REGEX,
    FILTER_ANAGRAM,
    FILTER_SUBANAGRAM,
    FILTER_SUPERANAGRAM,
    FILTER_TRANSADD,
    FILTER_TRANSDELETE,
    FILTER_BANK,
    FILTER_EXTRACT,
    FILTER_EXTRACTQ,
    FILTER_NX,
    _FILTER_TYPE_MAX,
};

extern const char * const filter_type_names[];

struct filter;

NOODLE_EXPORT struct filter * filter_create(enum filter_type type, size_t n_arg, const char * str_arg);
NOODLE_EXPORT struct filter * filter_parse(const char * spec);
NOODLE_EXPORT void filter_chain_apply(struct filter * const * fs, size_t n_fs, struct wordset * input, struct wordset * output,
                        struct wordlist * buffer);
NOODLE_EXPORT void filter_destroy(struct filter * f);
