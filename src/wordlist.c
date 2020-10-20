#include "wordlist.h"

void wordset_init(struct wordset * ws, const char * name) {
    *ws = (struct wordset){0};
    snprintf(ws->name, sizeof(ws->name) - 1, "%s", name);
    ws->words_count = 0;
    ws->words_capacity = 32;
    ws->words = NONNULL(calloc(ws->words_capacity, sizeof(*ws->words)));
}

void wordset_add(struct wordset * ws, const struct word * w) {
    if (ws->words_count >= ws->words_capacity) {
        ASSERT(ws->words_capacity > 0);
        ws->words_capacity *= 2;
        ASSERT(ws->words_count < ws->words_capacity);

        ws->words = NONNULL(realloc(ws->words, ws->words_capacity * sizeof(*ws->words)));
    }

    ws->words[ws->words_count++] = w;
}

void wordset_sort_value(struct wordset * ws) {
    qsort(ws->words, ws->words_count, sizeof(*ws->words), &word_value_ptrcmp);
}

void wordset_sort_canonical(struct wordset * ws) {
    qsort(ws->words, ws->words_count, sizeof(*ws->words), &word_canonical_ptrcmp);
}

void wordset_term(struct wordset * ws) { free(ws->words); }

const struct word * wordset_get(struct wordset * ws, size_t i) {
    if (i >= ws->words_count) {
        return NULL;
    }
    return ws->words[i];
}

void wordlist_init(struct wordlist * wl, const char * name) {
    *wl = (struct wordlist){0};
    wl->chunks = NULL;
    wl->insert_index = 0;
    wordset_init(&wl->self_set, name);
}

int wordlist_init_from_file(struct wordlist * wl, const char * filename) {
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
        wordlist_add(wl, line, (int)((int)rc * 100000 + (int)i));
        i++;
    }
    free(line);
    fclose(f);
    return 0;
}

void wordlist_add(struct wordlist * wl, const char * s, int v) {
    size_t i = wl->insert_index / WORDLIST_CHUNK_SIZE;
    size_t j = wl->insert_index % WORDLIST_CHUNK_SIZE;
    if (j == 0) {
        wl->chunks = NONNULL(realloc(wl->chunks, (i + 1) * sizeof(*wl->chunks)));
        wl->chunks[i] = NONNULL(calloc(WORDLIST_CHUNK_SIZE, sizeof(**wl->chunks)));
    }
    wl->insert_index++;

    struct word * w = &wl->chunks[i][j];
    word_init(w, s, v);
    wordset_add(&wl->self_set, w);
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
