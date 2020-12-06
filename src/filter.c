#include "filter.h"
#include "anagram_slow.h"
#include "bag_util.h"
#include "nx.h"
#include "nx_combo.h"
#include <regex.h>

struct filter;
struct filter_vtable;
struct apply_state;

// A filter represents a function that can match a word or phrase
//
// Filters have a string representation that is written by a user
// and parsed. (ex: "transadd 3: blah")
// These are usually in the form "TYPE [N]: [STR]"
//
// Some filters can also transform words (ex: extract).
// Some filters can produce phrases from a set of words from an
// `iterate` method (ex: anagram, nxn).
struct filter {
    // Methods & constants for the given filter type
    const struct filter_vtable * vtable;

    // Numeric & string arguments to the filter ("TYPE [N]: [STR]")
    // `arg_str` is NULL if absent; `arg_n` is -1 if absent
    char * arg_str;
    size_t arg_n;

    // Human-readable string representation of the filter
    char name[64];

    // Internal objects, used differently by each filter type
    // These must be init/term'd by the corresponding init/term function
    regex_t preg;
    struct nx * nx;
    struct word w;
    struct word * wb;
};

struct filter_vtable {
    // Filter type constants
    enum filter_type type;
    const char * name;

    // Initialize the filter, perform any pre-computation
    // `f` is already populated with `vtable`, `arg_str`, `arg_n`, and `name`
    int (*init)(struct filter * f);

    // Tear down the filter, clean up any internal objects
    // The `f` and `arg_str` pointers do not need to be free'd here
    void (*term)(struct filter * f);

    // Basic filtering operation
    // For the given input word `w`, call `filter_iterate(...)` for every output word
    // `state` and `index` are opaque values to be passed to `filter_iterate`,
    // with the exception of `state->ws`, the set of valid words
    void (*apply)(const struct filter * f, const struct word * w, const struct apply_state * state, size_t index);

    // Advanced generator / filtering operation
    // Can be NULL if not supported
    void (*iterate)(const struct filter * f, const struct apply_state * state);
};

struct apply_state {
    struct word_callback internal_cb;
    const struct wordset * ws;
    const struct filter * const * filters;
    size_t n_filters;
    struct cursor * cursor;
    struct word_callback * cb;
};

// Call inside `filter_*_apply` functions for each word `w` that
// matches the filter for the corresponding input word
//
// Pure filters typically call this at most once per `apply` with
// the input word given to `apply`.
static void filter_iterate(const struct apply_state * state, size_t filter_index, const struct word * w);

static void iterate_callback(struct word_callback * cb, const struct word * w) {
    struct apply_state * state = (void *)cb;
    filter_iterate(state, 1, w);
}

//

int filter_regex_init(struct filter * f) {
    if (f->arg_str[0] == '\0') {
        LOG("%s filter requires a string argument", f->vtable->name);
        return -1;
    }
    if (f->arg_n != -1ul) {
        LOG("%s filter does not take a numeric argument", f->vtable->name);
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
        LOG("%s filter requires a string argument", f->vtable->name);
        return -1;
    }
    if (f->vtable->type == FILTER_TRANSADD || f->vtable->type == FILTER_TRANSDELETE) {
        if (f->arg_n == -1ul) {
            LOG("%s filter requires a numeric argument", f->vtable->name);
            return -1;
        }
    } else {
        if (f->arg_n != -1ul) {
            LOG("%s filter does not take a numeric argument", f->vtable->name);
            return -1;
        }
    }
    word_init(&f->w, f->arg_str, 0);
    return 0;
}

void filter_anagram_term(struct filter * f) { word_term(&f->w); }

void filter_anagram_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                          size_t index) {
    if (strcmp(str_str(&w->sorted), str_str(&f->w.sorted)) == 0) {
        filter_iterate(state, index, w);
    }
}

void filter_anagram_iterate(const struct filter * f, const struct apply_state * state) {
    anagram_slow(state->ws, str_str(&f->w.sorted), state->cursor, (void *)&state->internal_cb);
}

#define filter_subanagram_init filter_anagram_init
#define filter_subanagram_term filter_anagram_term
#define filter_subanagram_iterate NULL

void filter_subanagram_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                             size_t index) {
    if (bag_difference_size_less_than(str_str(&f->w.sorted), str_str(&w->sorted), -1ul)) {
        filter_iterate(state, index, w);
    }
}

#define filter_superanagram_init filter_anagram_init
#define filter_superanagram_term filter_anagram_term
#define filter_superanagram_iterate NULL

void filter_superanagram_apply(const struct filter * f, const struct word * w, const struct apply_state * state,
                               size_t index) {
    if (bag_difference_size_less_than(str_str(&w->sorted), str_str(&f->w.sorted), -1ul)) {
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
    if (bag_difference_size_less_than(y, x, f->arg_n)) {
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
    if (bag_difference_size_less_than(x, y, f->arg_n)) {
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
        LOG("%s filter requires a string argument", f->vtable->name);
        return -1;
    }
    if (f->arg_n == -1ul) {
        f->arg_n = f->vtable->type == FILTER_NX ? 0 : 2;
    }
    if (f->vtable->type == FILTER_NXN) {
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

void filter_nxn_apply(const struct filter * f, const struct word * w, const struct apply_state * state, size_t index) {
    if (nx_match(f->nx, str_str(&w->canonical), 0) >= 0) {
        filter_iterate(state, index, w);
    }
}

void filter_nxn_iterate(const struct filter * f, const struct apply_state * state) {
    nx_combo_apply(f->nx, state->ws, f->arg_n, state->cursor, (void *)&state->internal_cb);
}

int filter_score_init(struct filter * f) {
    if (f->arg_str[0] != '\0') {
        LOG("%s filter does not take a string argument", f->vtable->name);
        return -1;
    }
    if (f->arg_n == -1ul) {
        LOG("%s filter requires a numeric argument", f->vtable->name);
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

const struct filter_vtable filter_vtables[] = {
#define X(N, n)                                                                                                        \
    [CONCAT(FILTER_, N)] = (struct filter_vtable){                                                                     \
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
_Static_assert(sizeof(filter_vtables) / sizeof(*filter_vtables) == _FILTER_TYPE_MAX, "Missing filter_vtable");

struct filter * filter_create(enum filter_type type, size_t n, const char * str) {
    ASSERT(str != NULL);
    if (type < 0 || type >= _FILTER_TYPE_MAX) {
        return NULL;
    }

    struct filter * f = NONNULL(calloc(1, sizeof(*f)));
    f->vtable = &filter_vtables[type];
    f->arg_n = n;
    f->arg_str = NONNULL(strdup(str));

    if (f->arg_n != -1ul) {
        snprintf(f->name, sizeof(f->name), "%s %zu: %s", f->vtable->name, f->arg_n, f->arg_str);
    } else {
        snprintf(f->name, sizeof(f->name), "%s: %s", f->vtable->name, f->arg_str);
    }

    int rc = 0;
    if (f->vtable->init != NULL) {
        rc = f->vtable->init(f);
    }
    if (rc != 0) {
        free(f->arg_str);
        free(f);
        return NULL;
    }

    return f;
}

struct filter * filter_parse(const char * specification) {
    ASSERT(specification != NULL);

    regex_t preg;
    const char * regex = "^\\s*([a-z]+)\\s*([0-9]*)\\s*:\\s*(\\S*)\\s*$";
    ASSERT(regcomp(&preg, regex, REG_EXTENDED | REG_ICASE) == 0);

    regmatch_t matches[4];
    int rc = regexec(&preg, specification, 4, matches, 0);
    regfree(&preg);

    if (rc != 0) {
        LOG("filter specification '%s' does not fit expected form: /%s/", specification, regex);
        return NULL;
    }

    size_t size = strlen(specification) + 1;
    char * buffer = NONNULL(calloc(1, size));

    memset(buffer, 0, size);
    memcpy(buffer, &specification[matches[1].rm_so], (size_t)(matches[1].rm_eo - matches[1].rm_so));
    enum filter_type type = _FILTER_TYPE_MAX;
    for (size_t i = 0; i < _FILTER_TYPE_MAX; i++) {
        if (strcmp(NONNULL(filter_vtables[i].name), buffer) == 0) {
            type = i;
            break;
        }
    }
    if (type == _FILTER_TYPE_MAX) {
        LOG("Invalid filter type '%s'", buffer);
        goto fail;
    }

    memset(buffer, 0, size);
    memcpy(buffer, &specification[matches[2].rm_so], (size_t)(matches[2].rm_eo - matches[2].rm_so));
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
    memcpy(buffer, &specification[matches[3].rm_so], (size_t)(matches[3].rm_eo - matches[3].rm_so));

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
    if (f->vtable->term != NULL) {
        f->vtable->term(f);
    }
    free(f->arg_str);
    free(f);
}

static void filter_iterate(const struct apply_state * state, size_t filter_index, const struct word * w) {
    if (filter_index >= state->n_filters) {
        state->cb->callback(state->cb, w);
    } else {
        const struct filter * filter = state->filters[filter_index];
        filter->vtable->apply(filter, w, state, filter_index + 1);
    }
}

void filter_chain_apply(const struct filter * const * filters, size_t n_filters, struct wordset * input,
                        struct cursor * cursor, struct word_callback * cb) {
    ASSERT(filters != NULL);
    ASSERT(n_filters > 0);
    ASSERT(input != NULL);
    ASSERT(cursor != NULL);
    ASSERT(cb != NULL);
    for (size_t i = 0; i < n_filters; i++) {
        ASSERT(filters[i]->vtable != NULL);
        if (i == 0) {
            ASSERT(filters[i]->vtable->apply != NULL || filters[i]->vtable->iterate != NULL);
        } else {
            ASSERT(filters[i]->vtable->apply != NULL);
        }
    }

    struct apply_state state = {
        .internal_cb = {iterate_callback},
        .filters = filters,
        .n_filters = n_filters,
        .ws = input,
        .cursor = cursor,
        .cb = cb,
    };

    cursor->total_input_items = input->words_count;
    if (filters[0]->vtable->iterate != NULL) {
        filters[0]->vtable->iterate(filters[0], &state);
    } else {
        for (size_t i = cursor->input_index; cursor_update_input(cursor, i); i++) {
            const struct word * w = input->words[i];
            filter_iterate(&state, 0, w);
        }
    }
}

void filter_chain_to_wordset(const struct filter * const * filters, size_t n_filters, struct wordset * input,
                             struct cursor * cursor, struct wordset * output, struct wordlist * buffer) {
    ASSERT(filters != NULL);
    ASSERT(input != NULL);
    ASSERT(cursor != NULL);
    ASSERT(output != NULL);
    ASSERT(buffer != NULL);
    ASSERT(input != output);

    struct word_callback * cb = word_callback_create_wordset_add(cursor, buffer, output);
    filter_chain_apply(filters, n_filters, input, cursor, cb);
    free(cb);
}

const char * filter_debug(struct filter * f) {
    static char buffer[2048];
    if (f->arg_str == NULL && f->arg_n == -1ul) {
        return f->vtable->name;
    } else if (f->arg_n == -1ul) {
        snprintf(buffer, sizeof(buffer), "%s: %s", f->vtable->name, f->arg_str);
    } else if (f->arg_str == NULL) {
        snprintf(buffer, sizeof(buffer), "%s %zu:", f->vtable->name, f->arg_n);
    } else {
        snprintf(buffer, sizeof(buffer), "%s %zu: %s", f->vtable->name, f->arg_n, f->arg_str);
    }
    return buffer;
}
