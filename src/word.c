#include "word.h"

static bool str_is_small(const struct str * s) { return s->small[0] != 0; }

void str_init(struct str * s, const char * c, size_t len) {
    *s = (struct str){0};

    if (len + 1 > sizeof(s->small)) {
        s->large = NONNULL(calloc(1, len + 1));
        if (c != NULL) {
            memcpy(s->large, c, len);
        }
    } else {
        if (c != NULL) {
            memcpy(s->small, c, len);
        }
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

static char * str_mutstr(struct str * s) {
    if (str_is_small(s)) {
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
    str_init(&w->canonical, original, strlen(original));
    for (char * c = str_mutstr(&w->canonical); *c != '\0'; c++) {
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
    str_init(&w->sorted, str_str(&w->canonical), strlen(original));
    char * s = str_mutstr(&w->sorted);
    qsort(s, strlen(s), 1, &cmp_letter);
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

void wordtuple_init(struct wordtuple * wt, const struct word * const * words, size_t n_words) {
    NONNULL(wt);
    NONNULL(words);
    ASSERT(n_words <= WORDTUPLE_N);

    *wt = (struct wordtuple){0};

    size_t total_len = 0;
    for (size_t i = 0; i < n_words; i++) {
        wt->words[i] = words[i];
        total_len += strlen(str_str(&words[i]->canonical));
    }
    str_init(&wt->canonical, NULL, total_len);
    char * s = str_mutstr(&wt->canonical);
    for (size_t i = 0; i < n_words; i++) {
        const char * w = str_str(&words[i]->canonical);
        size_t n = strlen(w);
        memcpy(s, w, n);
        s += n;
    }
}

void wordtuple_term(struct wordtuple * wt) { str_term(&wt->canonical); }

const char * wordtuple_original(struct wordtuple * wt) {
    static char buffer[2048];
    char * b = buffer;
    char * e = &buffer[sizeof(buffer)];
    for (size_t i = 0; i < WORDTUPLE_N; i++) {
        if (wt->words[i] == NULL) {
            break;
        }
        if (b > e) {
            break;
        }
        const char * c = str_str(&wt->words[i]->canonical);
        b += snprintf(b, (size_t)(e - b), "%s ", c);
    }
    return buffer;
}
