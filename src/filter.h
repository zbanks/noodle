#pragma once
#include "prelude.h"
#include "time_util.h"
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
    FILTER_NXN,
    _FILTER_TYPE_MAX,
};

struct filter;

NOODLE_EXPORT struct filter * filter_create(enum filter_type type, size_t arg_n, const char * arg_str);
NOODLE_EXPORT struct filter * filter_parse(const char * spec);
NOODLE_EXPORT void filter_chain_apply(struct filter * const * fs, size_t n_fs, struct wordset * input,
                                      struct cursor * cursor, struct wordset * output, struct wordlist * buffer);
NOODLE_EXPORT void filter_destroy(struct filter * f);
NOODLE_EXPORT const char * filter_debug(struct filter * f);
