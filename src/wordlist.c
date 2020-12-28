#include "wordlist.h"
#include <search.h>

static const unsigned char STR_FLAG_OWNED = 0x01;

void wordset_init(struct wordset * ws) {
    *ws = (struct wordset){0};
    ws->words_count = 0;
    ws->words_capacity = 32;
    ws->words = NONNULL(calloc(ws->words_capacity, sizeof(*ws->words)));
}

void wordset_add(struct wordset * ws, const struct word * w) {
    ASSERT(str_flags(&w->str) & STR_FLAG_OWNED);

    if (ws->words_count >= ws->words_capacity) {
        ASSERT(ws->words_capacity > 0);
        ws->words_capacity *= 2;
        ASSERT(ws->words_count < ws->words_capacity);

        ws->words = NONNULL(realloc(ws->words, ws->words_capacity * sizeof(*ws->words)));
    }

    ws->words[ws->words_count++] = w;
}

void wordset_term(struct wordset * ws) { free(ws->words); }

const struct word * wordset_get(const struct wordset * ws, size_t i) {
    if (i >= ws->words_count) {
        return NULL;
    }
    return ws->words[i];
}

const struct word * wordset_find(const struct wordset * ws, const struct str * s) {
    size_t count = ws->words_count;
    struct word ** w = lfind(&s, ws->words, &count, sizeof(*ws->words), str_ptrcmp);
    if (w != NULL) {
        return *w;
    }
    return NULL;
}

void wordset_print(const struct wordset * ws) {
    LOG("Wordset %zu:", ws->words_count);
    for (size_t i = 0; i < 50 && i < ws->words_count; i++) {
        LOG("  - %s", word_debug(ws->words[i]));
    }
}

//

void wordlist_init(struct wordlist * wl) {
    *wl = (struct wordlist){0};
    wl->chunks = NULL;
    wl->insert_index = 0;
    wordset_init(&wl->self_set);
}

int wordlist_init_from_file(struct wordlist * wl, const char * filename) {
    FILE * f = fopen(filename, "r");
    if (f == NULL) {
        PLOG("unable to open %s", filename);
        return -1;
    }

    wordlist_init(wl);

    char * line = NULL;
    size_t len = 0;
    ssize_t rc;
    size_t i = 0;
    while ((rc = getline(&line, &len, f)) != -1) {
        line[rc - 1] = '\0';
        // XXX: Filter out 1-letter words, except a & I
        if (strlen(line) == 1 && line[0] != 'a' && line[0] != 'I')
            continue;
        wordlist_add(wl, line);
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

const struct word * wordlist_add(struct wordlist * wl, const char * s) {
    struct word * w = wordlist_alloc(wl);
    word_init(w, s);
    str_flags_set(&w->str, STR_FLAG_OWNED);

    wordset_add(&wl->self_set, w);
    return w;
}

const struct word * wordlist_ensure_owned(struct wordlist * wl, const struct word * src) {
    if (str_flags(&src->str) & STR_FLAG_OWNED) {
        return src;
    }

    struct word * w = wordlist_alloc(wl);
    word_init_copy(w, src);
    str_flags_set(&w->str, STR_FLAG_OWNED);

    wordset_add(&wl->self_set, w);
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
    if (state->unique && wordset_find(state->output, &w->str) != NULL) {
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
