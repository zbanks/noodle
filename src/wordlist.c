#include "wordlist.h"
#include <search.h>

void wordset_init(struct wordset * ws, const char * name) {
    *ws = (struct wordset){0};
    snprintf(ws->name, sizeof(ws->name) - 1, "%s", name);
    ws->words_count = 0;
    ws->words_capacity = 32;
    ws->words = NONNULL(calloc(ws->words_capacity, sizeof(*ws->words)));
}

void wordset_add(struct wordset * ws, const struct word * w) {
    ASSERT(w->owned);

    if (ws->words_count >= ws->words_capacity) {
        ASSERT(ws->words_capacity > 0);
        ws->words_capacity *= 2;
        ASSERT(ws->words_count < ws->words_capacity);

        ws->words = NONNULL(realloc(ws->words, ws->words_capacity * sizeof(*ws->words)));
    }

    ws->words[ws->words_count++] = w;
    ws->is_canonically_sorted = false;
}

void wordset_sort_value(struct wordset * ws) {
    qsort(ws->words, ws->words_count, sizeof(*ws->words), &word_value_ptrcmp);
    ws->is_canonically_sorted = false;
}

void wordset_sort_canonical(struct wordset * ws) {
    qsort(ws->words, ws->words_count, sizeof(*ws->words), &str_ptrcmp);
    ws->is_canonically_sorted = true;
}

void wordset_term(struct wordset * ws) { free(ws->words); }

const struct word * wordset_get(const struct wordset * ws, size_t i) {
    if (i >= ws->words_count) {
        return NULL;
    }
    return ws->words[i];
}

const struct word * wordset_find(const struct wordset * ws, const struct str * s) {
    struct word ** w;
    if (ws->is_canonically_sorted) {
        w = bsearch(&s, ws->words, ws->words_count, sizeof(*ws->words), str_ptrcmp);
    } else {
        size_t count = ws->words_count;
        w = lfind(&s, ws->words, &count, sizeof(*ws->words), str_ptrcmp);
    }
    if (w != NULL) {
        return *w;
    }
    return NULL;
}

void wordset_print(struct wordset * ws) {
    LOG("Wordset \"%s\" (%zu):", ws->name, ws->words_count);
    for (size_t i = 0; i < 20 && i < ws->words_count; i++) {
        LOG("  - %s", word_debug(ws->words[i]));
    }
}

//

void wordlist_init(struct wordlist * wl, const char * name) {
    *wl = (struct wordlist){0};
    wl->chunks = NULL;
    wl->insert_index = 0;
    wordset_init(&wl->self_set, name);
}

int wordlist_init_from_file(struct wordlist * wl, const char * filename, bool has_weight) {
    FILE * f = fopen(filename, "r");
    if (f == NULL) {
        PLOG("unable to open %s", filename);
        return -1;
    }

    wordlist_init(wl, filename);

    char * line = NULL;
    size_t len = 0;
    ssize_t rc;
    size_t i = 0;
    while ((rc = getline(&line, &len, f)) != -1) {
        line[rc - 1] = '\0';
        if (has_weight) {
            char * word = strchr(line, ' ');
            *word++ = '\0';
            if (strlen(word) > 20) {
                continue;
            }
            wordlist_add(wl, word, (int)strtoul(line, NULL, 10));
        } else {
            // XXX: Filter out 1-letter words, except a & I
            if (strlen(line) == 1 && line[0] != 'a' && line[0] != 'I')
                continue;
            wordlist_add(wl, line, 1000);
        }
        i++;
    }
    free(line);
    fclose(f);
    return 0;
}

static struct word * wordlist_alloc(struct wordlist * wl) {
    size_t i = wl->insert_index / WORDLIST_CHUNK_SIZE;
    size_t j = wl->insert_index % WORDLIST_CHUNK_SIZE;
    if (j == 0) {
        wl->chunks = NONNULL(realloc(wl->chunks, (i + 1) * sizeof(*wl->chunks)));
        wl->chunks[i] = NONNULL(calloc(WORDLIST_CHUNK_SIZE, sizeof(**wl->chunks)));
    }
    wl->insert_index++;

    return &wl->chunks[i][j];
}

const struct word * wordlist_add(struct wordlist * wl, const char * s, int v) {
    struct word * w = wordlist_alloc(wl);
    word_init(w, s, v);
    w->owned = true;
    wordset_add(&wl->self_set, w);
    return w;
}

const struct word * wordlist_ensure_owned(struct wordlist * wl, const struct word * src) {
    if (src->owned) {
        // XXX This assertion only checks the "top" layer; theoretically we should
        // recurse down to the lower layers; but in general we should never end up
        // with an owned word being formed from a tuple of un-owned words!
        for (size_t i = 0; src->is_tuple && i < WORD_TUPLE_N; i++) {
            ASSERT(src->tuple_words[i]->owned);
        }
        return src;
    }

    struct word * w = wordlist_alloc(wl);
    word_init_copy(w, src);

    w->owned = true;
    wordset_add(&wl->self_set, w);

    for (size_t i = 0; w->is_tuple && i < WORD_TUPLE_N; i++) {
        if (w->tuple_words[i] == NULL) {
            break;
        }
        if (!w->tuple_words[i]->owned) {
            w->tuple_words[i] = wordlist_ensure_owned(wl, w->tuple_words[i]);
        }
    }
    return w;
}

void wordlist_term(struct wordlist * wl) {
    wordset_term(&wl->self_set);

    if (wl->insert_index == 0) {
        return;
    }
    wl->insert_index--;

    for (size_t i = 0; i <= wl->insert_index / WORDLIST_CHUNK_SIZE; i++) {
        for (size_t j = 0; j < WORDLIST_CHUNK_SIZE; j++) {
            if (i * WORDLIST_CHUNK_SIZE + j >= wl->insert_index) {
                break;
            }
            word_term(&wl->chunks[i][j]);
        }
        free(wl->chunks[i]);
    }
    free(wl->chunks);
}

//

void word_callback_destroy(struct word_callback * wcb) { free(wcb); }

struct word_callback_print {
    struct word_callback cb;
    size_t limit;
    size_t count;
};

static void word_callback_print(struct word_callback * cb, const struct word * w) {
    struct word_callback_print * state = (void *)cb;
    if (state->limit != 0 && state->count >= state->limit) {
        return;
    }
    state->count++;
    cursor_update_output(state->cb.cursor, state->count);
    LOG("- %s", word_debug(w));
}

struct word_callback * word_callback_create_print(struct cursor * cursor, size_t limit) {
    struct word_callback_print * state = NONNULL(calloc(1, sizeof(*state)));
    state->cb.callback = word_callback_print;
    state->cb.cursor = cursor;
    state->limit = limit;
    state->count = 0;
    return &state->cb;
}

struct word_callback_wordset {
    struct word_callback cb;
    struct wordlist * buffer;
    struct wordset * output;
    bool unique;
};

static void word_callback_wordset(struct word_callback * cb, const struct word * w) {
    struct word_callback_wordset * state = (void *)cb;
    if (state->unique && wordset_find(state->output, &w->canonical) != NULL) {
        return;
    }
    w = wordlist_ensure_owned(state->buffer, w);
    wordset_add(state->output, w);
    cursor_update_output(state->cb.cursor, state->output->words_count);
}

struct word_callback * word_callback_create_wordset_add(struct cursor * cursor, struct wordlist * buffer,
                                                        struct wordset * output) {
    struct word_callback_wordset * state = NONNULL(calloc(1, sizeof(*state)));
    state->cb.callback = word_callback_wordset;
    state->cb.cursor = cursor;
    state->buffer = buffer;
    state->output = output;
    state->unique = false;
    return &state->cb;
}

struct word_callback * word_callback_create_wordset_add_unique(struct cursor * cursor, struct wordlist * buffer,
                                                               struct wordset * output) {
    struct word_callback_wordset * state = NONNULL(calloc(1, sizeof(*state)));
    state->cb.callback = word_callback_wordset;
    state->cb.cursor = cursor;
    state->buffer = buffer;
    state->output = output;
    state->unique = true;
    return &state->cb;
}
