#include "word.h"

#define WORD_LARGE ((char)0xFF)
static bool word_is_small(const struct word * w) { return w->small[0] != WORD_LARGE; }

char * word_init(struct word * w, const char * c, size_t len) {
    ASSERT(w != NULL);
    ASSERT(c == NULL || c[0] != WORD_LARGE);

    memset(w, 0, sizeof(*w));

    if (len + 1 > sizeof(w->small)) {
        w->small[0] = WORD_LARGE;
        w->large = NONNULL(calloc(1, len + 1));
        if (c != NULL) {
            memcpy(w->large, c, len);
        }
        return w->large;
    } else {
        if (c != NULL) {
            memcpy(w->small, c, len);
        }
        return w->small;
    }
}

void word_init_copy(struct word * dst, const struct word * src) {
    if (word_is_small(src)) {
        *dst = *src;
    } else {
        const char * c = word_str(src);
        word_init(dst, c, strlen(c));
    }
}

void word_term(struct word * w) {
    if (w != NULL && !word_is_small(w) && w->large != NULL) {
        free(w->large);
    }
}

const char * word_str(const struct word * w) {
    if (w == NULL) {
        return "";
    } else if (word_is_small(w)) {
        return w->small;
    } else if (w->large != NULL) {
        return w->large;
    } else {
        return "";
    }
}

unsigned char word_flags(const struct word * w) {
    if (w == NULL) {
        return 0;
    } else if (word_is_small(w)) {
        return w->small_flags;
    } else if (w->large != NULL) {
        return w->large_flags;
    } else {
        return 0;
    }
}

void word_flags_set(struct word * w, unsigned char flags) {
    if (w == NULL) {
        return;
    } else if (word_is_small(w)) {
        w->small_flags = flags;
    } else if (w->large != NULL) {
        w->large_flags = flags;
    } else {
        return;
    }
}

int word_ptrcmp(const void * _x, const void * _y) {
    const struct word * const * x = _x;
    const struct word * const * y = _y;
    return strcmp(word_str(*x), word_str(*y));
}

void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words) {
    NONNULL(w);
    NONNULL(tuple_words);

    memset(w, 0, sizeof(*w));

    size_t total_len = 0;
    for (size_t i = 0; i < n_tuple_words; i++) {
        total_len += strlen(word_str(tuple_words[i])) + 1;
    }
    char * s = word_init(w, NULL, total_len);
    for (size_t i = 0; i < n_tuple_words; i++) {
        const char * si = word_str(tuple_words[i]);
        size_t n = strlen(si);
        memcpy(s, si, n);
        s += n;
        if (i + 1 != n_tuple_words) {
            *s++ = ' ';
        }
    }
}
