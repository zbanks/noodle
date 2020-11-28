#include "filter.h"
#include "nx.h"
#include "nx_combo.h"
#include <regex.h>

static bool difference_size_less_than(const char * superset, const char * subset, size_t max_size) {
    ASSERT(superset != NULL && subset != NULL);
    size_t size = 0;
    while (*subset != '\0') {
        if (*superset == '\0') {
            // The rest of subset is not in superset
            return false;
        }
        if (*superset == *subset) {
            superset++;
            subset++;
        } else if (*superset > *subset) {
            // There is a letter in subset not in superset
            return false;
        } else {
            // There is a letter in superset not in subset
            ASSERT(*superset < *subset);
            superset++;
            size++;
            if (size > max_size) {
                return false;
            }
        }
    }
    if (size + strlen(superset) > max_size) {
        return false;
    }
    return true;
}

struct apply_state {
    const struct wordset * ws;
    const struct filter * const * filters;
    size_t n_filters;
    struct cursor * cursor;
    void * callback_cookie;
    void (*callback)(const struct word * w, void * cookie);
};
static void filter_iterate(const struct apply_state * state, size_t filter_index, const struct word * w);

struct filter;
struct filter_vtbl {
    enum filter_type type;
    const char * name;
    int (*init)(struct filter * f);
    void (*term)(struct filter * f);
    void (*apply)(const struct filter * f, const struct word * w, const struct apply_state * state, size_t index);
    void (*iterate)(const struct filter * f, const struct apply_state * state);
};

struct filter {
    const struct filter_vtbl * vtbl;
    char * arg_str;
    size_t arg_n;
    char name[64];

    // Internal
    regex_t preg;
    struct nx * nx;
    struct word w;
    struct word * wb;
};

//

int filter_regex_init(struct filter * f) {
    if (f->arg_str[0] == '\0') {
        LOG("%s filter requires a string argument", f->vtbl->name);
        return -1;
    }
    if (f->arg_n != -1ul) {
        LOG("%s filter does not take a numeric argument", f->vtbl->name);
        return -1;
    }

    const char * regex = f->arg_str;
    char regex_modified[1024];
    if (strlen(regex) + 1 > sizeof(regex_modified)) {
        return -1;
    }
    snprintf(regex_modified, sizeof(regex_modified) - 1, "^%s$", regex);

    f->wb = NONNULL(calloc(1, sizeof(*f->wb)));
    return regcomp(&f->preg, regex_modified, REG_EXTENDED | REG_ICASE);
}

void filter_regex_term(struct filter * f) {
    regfree(&f->preg);
    free(f->wb);
}

#define filter_regex_iterate NULL
void filter_regex_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                        size_t index) {
    if (regexec(&f->preg, str_str(&w->canonical), 0, NULL, 0) == 0) {
        filter_iterate(state, index, w);
    }
}

//

int filter_anagram_init(struct filter * f) {
    if (f->arg_str[0] == '\0') {
        LOG("%s filter requires a string argument", f->vtbl->name);
        return -1;
    }
    if (f->vtbl->type == FILTER_TRANSADD || f->vtbl->type == FILTER_TRANSDELETE) {
        if (f->arg_n == -1ul) {
            LOG("%s filter requires a numeric argument", f->vtbl->name);
            return -1;
        }
    } else {
        if (f->arg_n != -1ul) {
            LOG("%s filter does not take a numeric argument", f->vtbl->name);
            return -1;
        }
    }
    word_init(&f->w, f->arg_str, 0);
    return 0;
}

void filter_anagram_term(struct filter * f) { word_term(&f->w); }

#define filter_anagram_iterate NULL
void filter_anagram_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                          size_t index) {
    if (strcmp(str_str(&w->sorted), str_str(&f->w.sorted)) == 0) {
        filter_iterate(state, index, w);
    }
}

#define filter_subanagram_init filter_anagram_init
#define filter_subanagram_term filter_anagram_term
#define filter_subanagram_iterate NULL

void filter_subanagram_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                             size_t index) {
    if (difference_size_less_than(str_str(&f->w.sorted), str_str(&w->sorted), -1ul)) {
        filter_iterate(state, index, w);
    }
}

#define filter_superanagram_init filter_anagram_init
#define filter_superanagram_term filter_anagram_term
#define filter_superanagram_iterate NULL

void filter_superanagram_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                               size_t index) {
    if (difference_size_less_than(str_str(&w->sorted), str_str(&f->w.sorted), -1ul)) {
        filter_iterate(state, index, w);
    }
}

#define filter_transdelete_init filter_anagram_init
#define filter_transdelete_term filter_anagram_term
#define filter_transdelete_iterate NULL

void filter_transdelete_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                              size_t index) {
    const char * x = str_str(&w->sorted);
    const char * y = str_str(&f->w.sorted);
    if (strlen(x) + f->arg_n != strlen(y)) {
        return;
    }
    if (difference_size_less_than(y, x, f->arg_n)) {
        filter_iterate(state, index, w);
    }
}

#define filter_transadd_init filter_anagram_init
#define filter_transadd_term filter_anagram_term
#define filter_transadd_iterate NULL

void filter_transadd_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                           size_t index) {
    const char * x = str_str(&w->sorted);
    const char * y = str_str(&f->w.sorted);
    if (strlen(x) != strlen(y) + f->arg_n) {
        return;
    }
    if (difference_size_less_than(x, y, f->arg_n)) {
        filter_iterate(state, index, w);
    }
}

#define filter_bank_init NULL
#define filter_bank_term NULL
#define filter_bank_iterate NULL

void filter_bank_apply(const struct filter * f, const struct word * w, const struct apply_state * state, size_t index) {
    const char * s = str_str(&w->sorted);
    for (; *s != '\0'; s++) {
        if (strchr(f->arg_str, *s) == NULL) {
            break;
        }
    }
    if (*s == '\0') {
        filter_iterate(state, index, w);
    }
}

#define filter_extract_init filter_regex_init
#define filter_extract_term filter_regex_term
#define filter_extract_iterate NULL

void filter_extract_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                          size_t index) {
    const char * s = str_str(&w->canonical);

    regmatch_t matches[2];
    if (regexec(&f->preg, s, 2, matches, 0) == 0) {
        static char buffer[1024];
        size_t len = (size_t)(matches[1].rm_eo - matches[1].rm_so);
        if (len == 0 || len >= sizeof(buffer)) {
            return;
        }
        memcpy(buffer, &s[matches[1].rm_so], len);
        buffer[len] = '\0';
        // XXX should use str_init_buffer instead?
        struct str s = {._padding = {(char)0xFF}, .large = buffer};
        const struct word * found_word = wordset_find(state->ws, &s);
        if (found_word != NULL) {
            filter_iterate(state, index, found_word);
        }
    }
}

#define filter_extractq_init filter_regex_init
#define filter_extractq_term filter_extract_term
#define filter_extractq_iterate NULL

void filter_extractq_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                           size_t index) {
    const char * s = str_str(&w->canonical);

    regmatch_t matches[2];
    if (regexec(&f->preg, s, 2, matches, 0) == 0) {
        static char buffer[1024];
        size_t len = (size_t)(matches[1].rm_eo - matches[1].rm_so);
        if (len == 0 || len >= sizeof(buffer)) {
            return;
        }
        memcpy(buffer, &s[matches[1].rm_so], len);
        buffer[len] = '\0';

        word_term(f->wb);
        word_init(f->wb, buffer, word_value(w));
        filter_iterate(state, index, f->wb);
    }
}

int filter_nx_init(struct filter * f) {
    if (f->arg_str[0] == '\0') {
        LOG("%s filter requires a string argument", f->vtbl->name);
        return -1;
    }
    if (f->arg_n == -1ul) {
        f->arg_n = f->vtbl->type == FILTER_NX ? 0 : 2;
    }
    if (f->vtbl->type == FILTER_NXN) {
        if (f->arg_n > WORD_TUPLE_N) {
            LOG("Max number of words for nxn is %zu", WORD_TUPLE_N);
            return -1;
        }
    }

    f->nx = nx_compile(f->arg_str);
    if (f->nx == NULL) {
        return -1;
    }
    return 0;
}

void filter_nx_term(struct filter * f) { nx_destroy(f->nx); }

#define filter_nx_iterate NULL
void filter_nx_apply(const struct filter * f, const struct word * w, const struct apply_state * state, size_t index) {
    if (nx_match(f->nx, str_str(&w->canonical), f->arg_n) >= 0) {
        filter_iterate(state, index, w);
    }
}

#define filter_nxn_init filter_nx_init
#define filter_nxn_term filter_nx_term

static void nxn_callback(const struct word * w, void * cookie) {
    const struct apply_state * state = cookie;
    filter_iterate(state, 1, w);
}

void filter_nxn_apply(const struct filter * f, const struct word * w, const struct apply_state * state, size_t index) {
    if (nx_match(f->nx, str_str(&w->canonical), 0) >= 0) {
        filter_iterate(state, index, w);
    }
}

void filter_nxn_iterate(const struct filter * f, const struct apply_state * state) {
    nx_combo_apply(f->nx, state->ws, f->arg_n, state->cursor, nxn_callback, (void *)state);
}

int filter_score_init(struct filter * f) {
    if (f->arg_str[0] != '\0') {
        LOG("%s filter does not take a string argument", f->vtbl->name);
        return -1;
    }
    if (f->arg_n == -1ul) {
        LOG("%s filter requires a numeric argument", f->vtbl->name);
        return -1;
    }
    return 0;
}

#define filter_score_term NULL
#define filter_score_iterate NULL

void filter_score_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                        size_t index) {
    if (word_value(w) >= (int)f->arg_n) {
        filter_iterate(state, index, w);
    }
}

//

#define FILTERS                                                                                                        \
    X(REGEX, regex)                                                                                                    \
    X(ANAGRAM, anagram)                                                                                                \
    X(SUBANAGRAM, subanagram)                                                                                          \
    X(SUPERANAGRAM, superanagram)                                                                                      \
    X(TRANSADD, transadd)                                                                                              \
    X(TRANSDELETE, transdelete)                                                                                        \
    X(BANK, bank)                                                                                                      \
    X(EXTRACT, extract)                                                                                                \
    X(EXTRACTQ, extractq)                                                                                              \
    X(NX, nx)                                                                                                          \
    X(NXN, nxn)                                                                                                        \
    X(SCORE, score)

const struct filter_vtbl filter_vtbls[] = {
#define X(N, n)                                                                                                        \
    [CONCAT(FILTER_, N)] = (struct filter_vtbl){                                                                       \
        .name = STRINGIFY(n),                                                                                          \
        .type = CONCAT(FILTER_, N),                                                                                    \
        .init = CONCAT(CONCAT(filter_, n), _init),                                                                     \
        .term = CONCAT(CONCAT(filter_, n), _term),                                                                     \
        .apply = CONCAT(CONCAT(filter_, n), _apply),                                                                   \
        .iterate = CONCAT(CONCAT(filter_, n), _iterate),                                                               \
    },
    FILTERS
#undef X
};
_Static_assert(sizeof(filter_vtbls) / sizeof(*filter_vtbls) == _FILTER_TYPE_MAX, "Missing filter_vtbl");

struct filter * filter_create(enum filter_type type, size_t n, const char * str) {
    ASSERT(str != NULL);
    if (type < 0 || type >= _FILTER_TYPE_MAX) {
        return NULL;
    }

    struct filter * f = NONNULL(calloc(1, sizeof(*f)));
    f->vtbl = &filter_vtbls[type];
    f->arg_n = n;
    f->arg_str = NONNULL(strdup(str));

    if (f->arg_n != -1ul) {
        snprintf(f->name, sizeof(f->name), "%s %zu: %s", f->vtbl->name, f->arg_n, f->arg_str);
    } else {
        snprintf(f->name, sizeof(f->name), "%s: %s", f->vtbl->name, f->arg_str);
    }

    int rc = 0;
    if (f->vtbl->init != NULL) {
        rc = f->vtbl->init(f);
    }
    if (rc != 0) {
        free(f->arg_str);
        free(f);
        return NULL;
    }

    return f;
}

struct filter * filter_parse(const char * spec) {
    ASSERT(spec != NULL);

    regex_t preg;
    const char * regex = "^\\s*([a-z]+)\\s*([0-9]*)\\s*:\\s*(\\S*)\\s*$";
    ASSERT(regcomp(&preg, regex, REG_EXTENDED | REG_ICASE) == 0);

    regmatch_t matches[4];
    int rc = regexec(&preg, spec, 4, matches, 0);
    regfree(&preg);

    if (rc != 0) {
        LOG("filter specification '%s' does not fit expected form: /%s/", spec, regex);
        return NULL;
    }

    size_t size = strlen(spec) + 1;
    char * buffer = NONNULL(calloc(1, size));

    memset(buffer, 0, size);
    memcpy(buffer, &spec[matches[1].rm_so], (size_t)(matches[1].rm_eo - matches[1].rm_so));
    enum filter_type type = _FILTER_TYPE_MAX;
    for (size_t i = 0; i < _FILTER_TYPE_MAX; i++) {
        if (strcmp(NONNULL(filter_vtbls[i].name), buffer) == 0) {
            type = i;
            break;
        }
    }
    if (type == _FILTER_TYPE_MAX) {
        LOG("Invalid filter type '%s'", buffer);
        goto fail;
    }

    memset(buffer, 0, size);
    memcpy(buffer, &spec[matches[2].rm_so], (size_t)(matches[2].rm_eo - matches[2].rm_so));
    size_t n = -1ul;
    if (*buffer != '\0') {
        errno = 0;
        n = strtoul(buffer, NULL, 10);
        if (errno != 0) {
            LOG("Invalid n argument '%s'", buffer);
            goto fail;
        }
    }

    memset(buffer, 0, size);
    memcpy(buffer, &spec[matches[3].rm_so], (size_t)(matches[3].rm_eo - matches[3].rm_so));

    struct filter * f = filter_create(type, n, buffer);
    free(buffer);
    return f;

fail:
    free(buffer);
    return NULL;
}

void filter_destroy(struct filter * f) {
    if (f == NULL) {
        return;
    }
    if (f->vtbl->term != NULL) {
        f->vtbl->term(f);
    }
    free(f->arg_str);
    free(f);
}

static void filter_iterate(const struct apply_state * state, size_t filter_index, const struct word * w) {
    if (filter_index >= state->n_filters) {
        cursor_update_output(state->cursor, state->cursor->output_index + 1);
        state->callback(w, state->callback_cookie);
    } else {
        const struct filter * filter = state->filters[filter_index];
        filter->vtbl->apply(filter, w, state, filter_index + 1);
    }
}

void filter_chain_apply(const struct filter * const * fs, size_t n_fs, struct wordset * input, struct cursor * cursor,
                        void (*callback)(const struct word * w, void * cookie), void * cookie) {
    ASSERT(fs != NULL);
    ASSERT(n_fs > 0);
    ASSERT(input != NULL);
    ASSERT(cursor != NULL);
    ASSERT(callback != NULL);
    for (size_t i = 0; i < n_fs; i++) {
        ASSERT(fs[i]->vtbl != NULL);
        if (i == 0) {
            ASSERT(fs[i]->vtbl->apply != NULL || fs[i]->vtbl->iterate != NULL);
        } else {
            ASSERT(fs[i]->vtbl->apply != NULL);
        }
    }

    struct apply_state state = {
        .filters = fs,
        .n_filters = n_fs,
        .ws = input,
        .cursor = cursor,
        .callback_cookie = cookie,
        .callback = callback,
    };

    cursor->total_input_items = input->words_count;
    if (fs[0]->vtbl->iterate != NULL) {
        fs[0]->vtbl->iterate(fs[0], &state);
    } else {
        for (size_t i = cursor->input_index; cursor_update_input(cursor, i); i++) {
            const struct word * w = input->words[i];
            filter_iterate(&state, 0, w);
        }
    }
}

void filter_chain_to_wordset(const struct filter * const * fs, size_t n_fs, struct wordset * input,
                             struct cursor * cursor, struct wordset * output, struct wordlist * buffer) {
    ASSERT(fs != NULL);
    ASSERT(input != NULL);
    ASSERT(cursor != NULL);
    ASSERT(output != NULL);
    ASSERT(buffer != NULL);
    ASSERT(input != output);

    struct word_callback_wordset_add_state add_state = {
        .output = output, .buffer = buffer,
    };
    filter_chain_apply(fs, n_fs, input, cursor, &word_callback_wordset_add_unique, &add_state);
}

const char * filter_debug(struct filter * f) {
    static char buffer[2048];
    if (f->arg_str == NULL && f->arg_n == -1ul) {
        return f->vtbl->name;
    } else if (f->arg_n == -1ul) {
        snprintf(buffer, sizeof(buffer), "%s: %s", f->vtbl->name, f->arg_str);
    } else if (f->arg_str == NULL) {
        snprintf(buffer, sizeof(buffer), "%s %zu:", f->vtbl->name, f->arg_n);
    } else {
        snprintf(buffer, sizeof(buffer), "%s %zu: %s", f->vtbl->name, f->arg_n, f->arg_str);
    }
    return buffer;
}
