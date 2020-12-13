#include "nx_combo.h"

struct nx_combo_cache {
    struct nx_set populated_starts;
    struct nx_set * transitions;

    const struct wordset * wordset;
    size_t wordset_size;
};

static void nx_combo_cache_destroy(struct nx * nx) {
    free(nx->combo_cache->transitions);
    free(nx->combo_cache);
    nx->combo_cache = NULL;
}

#define CACHE_BY_WORD

static void nx_combo_cache_create(struct nx * nx, const struct wordset * input) {
    NONNULL(nx);
    NONNULL(input);

    if (nx->combo_cache != NULL) {
        if (nx->combo_cache->wordset == input && nx->combo_cache->wordset_size == input->words_count) {
            return;
        }
        nx_combo_cache_destroy(nx);
    }
    nx->combo_cache = NONNULL(calloc(1, sizeof(*nx->combo_cache)));
    nx->combo_cache->wordset = input;
    nx->combo_cache->wordset_size = input->words_count;
    LOG("Allocating %ldMB for nx combo cache",
        (nx->n_states * input->words_count * sizeof(*nx->combo_cache->transitions)) >> 20);
    nx->combo_cache->transitions =
        NONNULL(calloc(nx->n_states * input->words_count, sizeof(*nx->combo_cache->transitions)));

#ifdef CACHE_BY_WORD
    int64_t start_ns = now_ns();
    for (size_t i = 0; i < input->words_count; i++) {
        enum nx_char wbuf[256];
        nx_char_translate(str_str(&input->words[i]->canonical), wbuf, 256);
        if (wbuf[0] == NX_CHAR_END) {
            continue;
        }
        struct nx_set * transitions = &nx->combo_cache->transitions[nx->n_states * i];
        for (size_t k = 0; k < nx->n_states; k++) {
            struct nx_set ts = nx_match_partial(nx, wbuf, (uint16_t)k);
            if (!nx_set_isempty(&ts)) {
                transitions[k] = ts;
            }
        }
    }
    LOG("Populated cache of %zu words in %ldms", input->words_count, (now_ns() - start_ns) / 1000000);
#endif
}

static const struct nx_set * nx_combo_cache_get(const struct nx * nx, const struct wordset * input, size_t index) {
#ifdef CACHE_BY_WORD
    (void)input;
    return &nx->combo_cache->transitions[nx->n_states * index];
#else
    struct nx_set * transitions = &nx->combo_cache->transitions[input->words_count * index];
    if (!nx_set_test(&nx->combo_cache->populated_starts, index)) {
        int64_t start_ns = now_ns();
        for (size_t i = 0; i < input->words_count; i++) {
            enum nx_char wbuf[256];
            nx_char_translate(str_str(&input->words[i]->canonical), wbuf, 256);
            if (wbuf[0] == NX_CHAR_END) {
                continue;
            }
            struct nx_set ts = nx_match_partial(nx, wbuf, (uint16_t)index);
            // Avoid writing to the cache if empty
            // We can save some pages from being allocated if we have a large run of empty sets in a row
            if (!nx_set_isempty(&ts)) {
                transitions[i] = ts;
            }
        }
        nx_set_add(&nx->combo_cache->populated_starts, index);
        LOG("Populated cache of %zu words for start state %zu in %ldms", input->words_count, index,
            (now_ns() - start_ns) / 1000000);
    }
    return transitions;
#endif
}

// This algorithm is pretty naive, and does a lot of re-calculation
// It could probably be drastically improved by adding a caching layer
// around `nx_match_partial(...)` keyed off of `wbuf` & `k`?

// TODO: Checking `now_ns()` constantly in `cursor_update_input(...)` adds a ~15% overhead.
// This can be avoided by running without a deadline_ns
static bool nx_combo_match_iter2(const struct nx * nx, const struct wordset * input, const struct word ** stems,
                                 const struct nx_set * stem_ss, struct cursor * cursor, size_t n_words,
                                 size_t word_index, struct word_callback * cb) {
#ifndef CACHE_BY_WORD
    size_t first_k = cursor->input_index_list[word_index] / nx->n_states;
    size_t first_i = cursor->input_index_list[word_index] % nx->n_states;
    // LOG("word_index=%zu first_k=%zu first_i=%zu", word_index, first_k, first_i);
    for (size_t k = first_k; k < nx->n_states; k++) {
        if (!nx_set_test(stem_ss, k)) {
            continue;
        }
        if (!nx_set_test(&nx->head_states, k)) {
            // continue;
        }
        // LOG("word_index=%zu k=%zu n_words=%zu", word_index, k, n_words);
        cursor->input_index_list[word_index] = k * nx->n_states + first_i;

        const struct nx_set * transitions = nx_combo_cache_get(nx, input, k);
        ASSERT(transitions);

        for (size_t i = first_i; i < input->words_count; i++) {
            // XXX may need to be revised
            // Check if we've exceeded a deadline
            if (!cursor_update_input(cursor, (word_index == 0 && k == 0) ? i : cursor->input_index)) {
                return false;
            }

            const struct nx_set * end_ss = &transitions[i];
            if (nx_set_isempty(end_ss)) {
                continue;
            }
            // LOG("word: %s, end_ss: %s, k=%zu, index=%zu", word_debug(input->words[i]), nx_set_debug(end_ss), k,
            // word_index);
            cursor->input_index_list[word_index] = k * nx->n_states + i;
            stems[word_index] = input->words[i];
            // TODO: I don't like that this yields multi-words before single words,
            // but going in this order is important for making the cursor work
            if (n_words > word_index + 1) {
                bool rc = nx_combo_match_iter2(nx, input, stems, end_ss, cursor, n_words, word_index + 1, cb);
                if (!rc) {
                    return false;
                }
                cursor->input_index_list[word_index + 1] = 0;
            }
            if (nx_set_test(end_ss, nx->n_states - 1)) {
                struct word wp;
                word_tuple_init(&wp, stems, word_index + 1);
                cb->callback(cb, &wp);
                // LOG("Match: %s", word_debug(&wp));
            }
        }
        first_i = 0;
    }
#else
    size_t first_i = cursor->input_index_list[word_index] / nx->n_states;
    size_t first_k = cursor->input_index_list[word_index] % nx->n_states;
    for (size_t i = first_i; i < input->words_count; i++) {
        cursor->input_index_list[word_index] = i * nx->n_states + first_k;

        // Check if we've exceeded a deadline
        if (!cursor_update_input(cursor, (word_index == 0) ? i : cursor->input_index)) {
            return false;
        }
        const struct nx_set * transitions = nx_combo_cache_get(nx, input, i);
        for (size_t k = first_k; k < nx->n_states; k++) {
            if (!nx_set_test(stem_ss, k)) {
                continue;
            }
            if (!nx_set_test(&nx->head_states, k)) {
                // continue;
            }
            const struct nx_set * end_ss = &transitions[k];
            if (nx_set_isempty(end_ss)) {
                continue;
            }
            cursor->input_index_list[word_index] = i * nx->n_states + k;
            stems[word_index] = input->words[i];
            // TODO: I don't like that this yields multi-words before single words,
            // but going in this order is important for making the cursor work
            if (n_words > word_index + 1) {
                bool rc = nx_combo_match_iter2(nx, input, stems, end_ss, cursor, n_words, word_index + 1, cb);
                if (!rc) {
                    return false;
                }
                cursor->input_index_list[word_index + 1] = 0;
            }
            if (nx_set_test(end_ss, nx->n_states - 1)) {
                struct word wp;
                word_tuple_init(&wp, stems, word_index + 1);
                cb->callback(cb, &wp);
                // LOG("Match: %s", word_debug(&wp));
            }
        }
        first_k = 0;
    }
#endif
    if (word_index == 0) {
        cursor_update_input(cursor, input->words_count);
    }
    return true;
}

// Without cache
static bool nx_combo_match_iter(const struct nx * nx, const struct wordset * input, const struct word ** stems,
                                struct nx_set stem_ss, struct cursor * cursor, size_t n_words, size_t word_index,
                                struct word_callback * cb) {
    size_t first_i = cursor->input_index_list[word_index] / nx->n_states;
    size_t first_k = cursor->input_index_list[word_index] % nx->n_states;
    for (size_t i = first_i; i < input->words_count; i++) {
        cursor->input_index_list[word_index] = i * nx->n_states + first_k;

        // Check if we've exceeded a deadline
        if (!cursor_update_input(cursor, (word_index == 0) ? i : cursor->input_index)) {
            return false;
        }

        enum nx_char wbuf[256];
        nx_char_translate(str_str(&input->words[i]->canonical), wbuf, 256);
        if (wbuf[0] == NX_CHAR_END) {
            continue;
        }
        for (size_t k = first_k; k < nx->n_states; k++) {
            if (!nx_set_test(&stem_ss, k)) {
                continue;
            }
            if (!nx_set_test(&nx->head_states, k)) {
                continue;
            }
            struct nx_set end_ss = nx_match_partial(nx, wbuf, (uint16_t)k);
            if (nx_set_isempty(&end_ss)) {
                continue;
            }
            cursor->input_index_list[word_index] = i * nx->n_states + k;
            stems[word_index] = input->words[i];
            // TODO: I don't like that this yields multi-words before single words,
            // but going in this order is important for making the cursor work
            if (n_words > word_index + 1) {
                bool rc = nx_combo_match_iter(nx, input, stems, end_ss, cursor, n_words, word_index + 1, cb);
                if (!rc) {
                    return false;
                }
                cursor->input_index_list[word_index + 1] = 0;
            }
            if (nx_set_test(&end_ss, nx->n_states - 1)) {
                struct word wp;
                word_tuple_init(&wp, stems, word_index + 1);
                cb->callback(cb, &wp);
                // LOG("Match: %s", word_debug(&wp));
            }
        }
        first_k = 0;
    }
    if (word_index == 0) {
        cursor_update_input(cursor, input->words_count);
    }
    return true;
}

void nx_combo_apply(struct nx * nx, const struct wordset * input, size_t n_words, struct cursor * cursor,
                    struct word_callback * cb) {
    cursor->total_input_items = input->words_count;
    ASSERT(n_words + 1 <= CURSOR_LIST_MAX);
    nx_combo_cache_create(nx, input);

    struct nx_set start_ss = {0};
    nx_set_add(&start_ss, 0);
    ASSERT(n_words <= WORD_TUPLE_N);
    const struct word * stems[n_words];
    (void)nx_combo_match_iter;
    (void)nx_combo_match_iter2;
    nx_combo_match_iter2(nx, input, stems, &start_ss, cursor, n_words, 0, cb);
    nx_combo_match_iter(nx, input, stems, start_ss, cursor, n_words, 0, cb);
}

void nx_combo_match(struct nx * nx, const struct wordset * input, size_t n_words, struct cursor * cursor,
                    struct wordset * output, struct wordlist * buffer) {
    struct word_callback * cb = NONNULL(word_callback_create_wordset_add(cursor, buffer, output));
    nx_combo_cache_create(nx, input);
    nx_combo_apply(nx, input, n_words, cursor, cb);
    free(cb);
}
