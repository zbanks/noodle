#pragma once
#include "prelude.h"
#include "wordlist.h"

int filter_regex(const char * regex, const struct wordset * src, struct wordset * dst);
void filter_anagram(const char * letters, const struct wordset * src, struct wordset * dst);
void filter_subanagram(const char * letters, const struct wordset * src, struct wordset * dst);
void filter_superanagram(const char * letters, const struct wordset * src, struct wordset * dst);
void filter_transadd(size_t n, const char * letters, const struct wordset * src, struct wordset * dst);
void filter_transdelete(size_t n, const char * letters, const struct wordset * src, struct wordset * dst);
void filter_bank(const char * letters, const struct wordset * src, struct wordset * dst);

enum filter_type {
    FILTER_REGEX,
    FILTER_ANAGRAM,
    FILTER_SUBANAGRAM,
    FILTER_SUPERANAGRAM,
    FILTER_TRANSADD,
    FILTER_TRANSDELETE,
    FILTER_BANK,
    _FILTER_TYPE_MAX,
};
extern const char * const filter_type_names[];

struct filter {
    enum filter_type type;
    size_t arg_n;
    char * arg_str;
    struct wordset output;
    char name[64];
};

void filter_init(struct filter * f, enum filter_type type, size_t n, const char * str);
int filter_parse(struct filter * f, const char * spec);
int filter_apply(struct filter * f, struct wordset * input);
void filter_term(struct filter * f);
