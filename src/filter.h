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
    FILTER_SCORE,
    _FILTER_TYPE_MAX,
};

struct filter;

// Create a filter object from raw values
NOODLE_EXPORT struct filter * filter_create(enum filter_type type, size_t arg_n, const char * arg_str);
// Create a filter object from a string specification
NOODLE_EXPORT struct filter * filter_parse(const char * specification);
// Destroy a filter object
NOODLE_EXPORT void filter_destroy(struct filter * f);
// Return a string representation of the filter
NOODLE_EXPORT const char * filter_debug(struct filter * f);

// Apply a series of filters to an input wordset, calling `callback` with each result
NOODLE_EXPORT void filter_chain_apply(const struct filter * const * filters, size_t n_filters, struct wordset * input,
                                      struct cursor * cursor, struct word_callback * cb);

// Shortcut to using `filter_chain_apply` with `word_callback_wordset_add_state`
NOODLE_EXPORT void filter_chain_to_wordset(const struct filter * const * filters, size_t n_filters,
                                           struct wordset * input, struct cursor * cursor, struct wordset * output,
                                           struct wordlist * buffer);
