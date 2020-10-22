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
    _FILTER_TYPE_MAX,
};

extern const char * const filter_type_names[];

struct filter;

struct filter * filter_create(enum filter_type type, size_t n_arg, const char * str_arg);
struct filter * filter_parse(const char * spec);
void filter_apply(struct filter * f, struct wordset * input, struct wordset * output);
void filter_destroy(struct filter * f);
