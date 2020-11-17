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
    w->is_tuple = false;
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
    (void)s;
    (void)cmp_letter;
    qsort(s, strlen(s), 1, &cmp_letter);
}

void word_init_copy(struct word * dst, const struct word * src) {
    *dst = (struct word){
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

int word_value(const struct word * w) {
    if (w == NULL) {
        return 0;
    } else if (!w->is_tuple) {
        return w->value;
    } else {
        // Take max over all words, recursively
        int value = word_value(w->tuple_words[0]);
        for (size_t i = 1; i < WORD_TUPLE_N; i++) {
            value = MAX(value, word_value(w->tuple_words[i]));
        }
        return value;
    }
}

int word_value_cmp(const void * _x, const void * _y) {
    const struct word * x = _x;
    const struct word * y = _y;
    return cmp(word_value(y), word_value(x));
}

int word_value_ptrcmp(const void * _x, const void * _y) {
    const struct word * const * x = _x;
    const struct word * const * y = _y;
    return cmp(word_value(*y), word_value(*x));
}

void word_tuple_init(struct word * w, const struct word * const * tuple_words, size_t n_tuple_words) {
    NONNULL(w);
    NONNULL(tuple_words);
    ASSERT(n_tuple_words <= WORD_TUPLE_N);

    *w = (struct word){.is_tuple = true};

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

ssize_t word_debug_iter(const struct word * w, char * buf) {
    if (w == NULL) {
        return sprintf(buf, "\"\"");
    } else if (w->is_tuple) {
        if (w->tuple_words[0] == NULL) {
            sprintf(buf, "[]");
        }

        ssize_t rc = 0;
        char * b = buf;
        *b++ = '[';
        rc++;
        for (size_t i = 0; i < WORD_TUPLE_N; i++) {
            if (w->tuple_words[i] == NULL) {
                break;
            }
            ssize_t k = word_debug_iter(w->tuple_words[i], b);
            rc += k;
            b += k;
            *b++ = ' ';
            rc++;
        }
        b--;
        *b++ = ']';
        *b++ = '\0';
        return rc;
    } else {
        // return sprintf(buf, "%s [\"%s\" %d]", str_str(&w->canonical), str_str(&w->original), w->value);
        return sprintf(buf, "%s", str_str(&w->original));
    }
}

const char * word_debug(const struct word * w) {
    static char buffer[2048];
    word_debug_iter(w, buffer);
    return buffer;
}

const char * word_canonical(const struct word * w) { return str_str(&w->canonical); }
