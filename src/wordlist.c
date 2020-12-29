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
    ASSERT(word_flags(w) & STR_FLAG_OWNED);

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

const struct word * wordset_find(const struct wordset * ws, const struct word * s) {
    size_t count = ws->words_count;
    struct word ** w = lfind(&s, ws->words, &count, sizeof(*ws->words), word_ptrcmp);
    if (w != NULL) {
        return *w;
    }
    return NULL;
}

void wordset_print(const struct wordset * ws) {
    LOG("Wordset %zu:", ws->words_count);
    for (size_t i = 0; i < 50 && i < ws->words_count; i++) {
        LOG("  - \"%s\"", word_str(ws->words[i]));
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
    word_init(w, s, strlen(s));
    word_flags_set(w, STR_FLAG_OWNED);

    wordset_add(&wl->self_set, w);
    return w;
}

const struct word * wordlist_ensure_owned(struct wordlist * wl, const struct word * src) {
    if (word_flags(src) & STR_FLAG_OWNED) {
        return src;
    }

    struct word * w = wordlist_alloc(wl);
    word_init_copy(w, src);
    word_flags_set(w, STR_FLAG_OWNED);

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
