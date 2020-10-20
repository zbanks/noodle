#include "word.h"

static bool str_is_small(const struct str * s) { return s->small[0] != 0; }

void str_init(struct str * s, const char * c) {
    *s = (struct str){0};

    size_t len = strlen(c) + 1;
    if (len > sizeof(s->small)) {
        s->large = NONNULL(calloc(1, len));
        memcpy(s->large, c, len);
    } else {
        memcpy(s->small, c, len);
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
    str_init(&w->original, original);

    // Convert to lowercase, stripping out all non-letters
    str_init(&w->canonical, original);
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
    str_init(&w->sorted, str_str(&w->canonical));
    char * s = str_mutstr(&w->sorted);
    qsort(s, strlen(s), 1, &cmp_letter);
}

void word_term(struct word * w) {
    str_term(&w->original);
    str_term(&w->canonical);
    str_term(&w->sorted);
}

int word_canonical_cmp(const void * _x, const void * _y) {
    const struct word * x = _x;
    const struct word * y = _y;
    return strcmp(str_str(&x->canonical), str_str(&y->canonical));
}

int word_canonical_ptrcmp(const void * _x, const void * _y) {
    const struct word * const * x = _x;
    const struct word * const * y = _y;
    return strcmp(str_str(&(*x)->canonical), str_str(&(*y)->canonical));
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
