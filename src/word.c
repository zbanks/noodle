#include "word.h"

#define STR_LARGE ((char)0xFF)
static bool str_is_small(const struct str * s) { return s->small[0] != STR_LARGE; }

char * str_init(struct str * s, const char * c, size_t len) {
    ASSERT(s != NULL);
    ASSERT(c == NULL || c[0] != STR_LARGE);

    *s = (struct str){0};

    if (len + 1 > sizeof(s->small)) {
        s->small[0] = STR_LARGE;
        s->large = NONNULL(calloc(1, len + 1));
        if (c != NULL) {
            memcpy(s->large, c, len);
        }
        return s->large;
    } else {
        if (c != NULL) {
            memcpy(s->small, c, len);
        }
        return s->small;
    }
}

void str_init_copy(struct str * dst, const struct str * src) {
    if (str_is_small(src)) {
        *dst = *src;
    } else {
        const char * c = str_str(src);
        str_init(dst, c, strlen(c));
    }
}

char * str_init_buffer(struct str * s, size_t len) {
    ASSERT(s != NULL);
    *s = (struct str){0};
    s->small[0] = STR_LARGE;
    s->large = NONNULL(calloc(1, len));
    return s->large;
}

void str_term(struct str * s) {
    if (s != NULL && !str_is_small(s) && s->large != NULL) {
        free(s->large);
    }
}

const char * str_str(const struct str * s) {
    if (s == NULL) {
        return "";
    } else if (str_is_small(s)) {
        return s->small;
    } else if (s->large != NULL) {
        return s->large;
    } else {
        return "";
    }
}

int str_cmp(const void * _x, const void * _y) {
    const struct str * x = _x;
    const struct str * y = _y;
    return strcmp(str_str(x), str_str(y));
}

int str_ptrcmp(const void * _x, const void * _y) {
    const struct str * const * x = _x;
    const struct str * const * y = _y;
    return strcmp(str_str(*x), str_str(*y));
}

void word_init(struct word * w, const char * original) { str_init(&w->str, original, strlen(original)); }

void word_init_copy(struct word * dst, const struct word * src) {
    *dst = (struct word){0};
    str_init_copy(&dst->str, &src->str);
}

void word_term(struct word * w) { str_term(&w->str); }

void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words) {
    NONNULL(w);
    NONNULL(tuple_words);

    *w = (struct word){0};

    size_t total_len = 0;
    for (size_t i = 0; i < n_tuple_words; i++) {
        total_len += strlen(word_cstr(tuple_words[i])) + 1;
    }
    char * s = str_init(&w->str, NULL, total_len);
    for (size_t i = 0; i < n_tuple_words; i++) {
        const char * w = word_cstr(tuple_words[i]);
        size_t n = strlen(w);
        memcpy(s, w, n);
        s += n;
        if (i + 1 != n_tuple_words) {
            *s++ = ' ';
        }
    }
}

const char * word_debug(const struct word * w) { return word_cstr(w); }

const char * word_cstr(const struct word * w) { return str_str(&w->str); }
