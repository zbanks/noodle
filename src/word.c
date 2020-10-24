#include "word.h"

static bool str_is_small(const struct str * s) { return s->small[0] != 0; }

char * str_init(struct str * s, const char * c, size_t len) {
    *s = (struct str){0};

    if (len + 1 > sizeof(s->small)) {
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

void str_init_copy(struct str * dst, const struct str *src) {
    if (str_is_small(src)) {
        *dst = *src;
    } else {
        const char *c = str_str(src);
        str_init(dst, c, strlen(c));
    }
}

void str_term(struct str * s) {
    if (s != NULL && !str_is_small(s) && s->large != NULL) {
        cfree(s->large);
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

static bool is_lower(char c) { return c >= 'a' && c <= 'z'; }

static bool is_upper(char c) { return c >= 'A' && c <= 'Z'; }

static int cmp(int x, int y) {
    if (x == y) {
        return 0;
    } else if (x < y) {
        return -1;
    } else {
        return 1;
    }
}

static int cmp_letter(const void * _x, const void * _y) {
    const char * x = _x;
    const char * y = _y;
    return cmp(*x, *y);
}

void word_init(struct word * w, const char * original, int value) {
    w->value = value;
    str_init(&w->original, original, strlen(original));

    // Convert to lowercase, stripping out all non-letters
    char * c = str_init(&w->canonical, original, strlen(original));
    for (; *c != '\0'; c++) {
        if (is_lower(*c)) {
            continue;
        }
        if (is_upper(*c)) {
            *c ^= 0x20;
            continue;
        }
        // Delete the letter
        memmove(c, c + 1, strlen(c));
        c--;
    }

    // Create sorted representation
    char * s = str_init(&w->sorted, str_str(&w->canonical), strlen(original));
    qsort(s, strlen(s), 1, &cmp_letter);
}

void word_init_copy(struct word * dst, const struct word * src) {
    *dst = (struct word) {
        .is_tuple = src->is_tuple,
    };
    str_init_copy(&dst->canonical, &src->canonical);
    if (src->is_tuple) {
        memcpy(dst->tuple_words, src->tuple_words, sizeof(src->tuple_words));
    } else {
        dst->value = src->value;
        str_init_copy(&dst->original, &src->original);
        str_init_copy(&dst->sorted, &src->sorted);
    }
}

void word_term(struct word * w) {
    str_term(&w->original);
    str_term(&w->canonical);
    str_term(&w->sorted);
}

int word_value_cmp(const void * _x, const void * _y) {
    const struct word * x = _x;
    const struct word * y = _y;
    return cmp(y->value, x->value);
}

int word_value_ptrcmp(const void * _x, const void * _y) {
    const struct word * const * x = _x;
    const struct word * const * y = _y;
    return cmp((*y)->value, (*x)->value);
}

void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words) {
    NONNULL(w);
    NONNULL(tuple_words);
    ASSERT(n_tuple_words <= WORD_TUPLE_N);

    *w = (struct word){ .is_tuple = true };

    size_t total_len = 0;
    for (size_t i = 0; i < n_tuple_words; i++) {
        w->tuple_words[i] = tuple_words[i];
        total_len += strlen(str_str(&tuple_words[i]->canonical));
    }
    char * s = str_init(&w->canonical, NULL, total_len);
    for (size_t i = 0; i < n_tuple_words; i++) {
        const char * w = str_str(&tuple_words[i]->canonical);
        size_t n = strlen(w);
        memcpy(s, w, n);
        s += n;
    }
}

const char * word_debug(const struct word * w) {
    static char buffer[2048];
    if (w == NULL) {
        return "\"\"";
    } else if (w->is_tuple) {
        char * b = buffer;
        char * e = &buffer[sizeof(buffer)];
        *b++ = '[';
        for (size_t i = 0; i < WORD_TUPLE_N; i++) {
            if (w->tuple_words[i] == NULL) {
                break;
            }
            if (b > e) {
                break;
            }
            const char * c = str_str(&w->tuple_words[i]->canonical);
            b += snprintf(b, (size_t)(e - b), "%s ", c);
        }
        *--b = ']';
    } else {
        snprintf(buffer, sizeof(buffer), "%s [\"%s\" %d]",
                str_str(&w->canonical), str_str(&w->original), w->value);
    }
    return buffer;
}
