#include "nx_combo.h"

// TODO: better cache by grouping words by their transition table?
// e.g. all N letter words will have the same transition table for nx `......`
// if the # of unique transition tables is small, then this should save a lot of memory
// and could allow for faster iterations? could even intersect sets?
struct nx_combo_cache {
    // Array of: [word_index][initial_state] -> final_stateset
    struct nx_set * transitions;

    const struct wordset * wordset;
    size_t wordset_size;
};

static void nx_combo_cache_destroy(struct nx * nx) {
    free(nx->combo_cache->transitions);
    free(nx->combo_cache);
    nx->combo_cache = NULL;
}

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

    int64_t start_ns = now_ns();
    size_t unique = 0;
    bool * repeatset = NONNULL(calloc(input->words_count, sizeof(*repeatset)));
    for (size_t i = 0; i < input->words_count; i++) {
        enum nx_char wbuf[256];
        nx_char_translate(str_str(&input->words[i]->canonical), wbuf, 256);
        if (wbuf[0] == NX_CHAR_END) {
            continue;
        }
        struct nx_set * transitions = &nx->combo_cache->transitions[nx->n_states * i];
        // XXX This is an "O(n^2)ish" algorithm that probably could be done in "O(n)ish"
        // if we implement filling the whole transition table in one shot
        for (size_t k = 0; k < nx->n_states; k++) {
            struct nx_set ts = nx_match_partial(nx, wbuf, (uint16_t)k);
            // Avoid writing to the cache if empty
            // We can save some pages from being allocated if we have a large run of empty sets in a row
            if (true || !nx_set_isempty(&ts)) {
                transitions[k] = ts;
            }
        }
        for (size_t j = 0; j < i; j++) {
            if (repeatset[j]) {
                continue;
            }
            if (memcmp(transitions, &nx->combo_cache->transitions[nx->n_states * j],
                       sizeof(*transitions) * nx->n_states) == 0) {
                repeatset[i] = true;
                break;
            }
        }
        if (!repeatset[i]) {
            unique++;
        }
    }
    free(repeatset);
    LOG("Populated cache of %zu words in %ldms: %zu unique", input->words_count, (now_ns() - start_ns) / 1000000,
        unique);
}

static const struct nx_set * nx_combo_cache_get(const struct nx * nx, size_t word_index) {
    return &nx->combo_cache->transitions[nx->n_states * word_index];
}

// TODO: Checking `now_ns()` constantly in `cursor_update_input(...)` adds a ~15% overhead.
// This can be avoided by running without a deadline_ns
static bool nx_combo_match_iter(const struct nx * nx, const struct wordset * input, const struct word ** stems,
                                const struct nx_set * stem_ss, struct cursor * cursor, size_t n_words,
                                size_t word_index, struct word_callback * cb) {
    for (size_t i = cursor->input_index_list[word_index]; i < input->words_count; i++) {
        cursor->input_index_list[word_index] = i;

        // Check if we've exceeded a deadline
        if (!cursor_update_input(cursor, (word_index == 0) ? i : cursor->input_index)) {
            return false;
        }
        const struct nx_set * transitions = nx_combo_cache_get(nx, i);
        struct nx_set end_ss = {0};
        for (size_t k = 0; k < nx->n_states; k++) {
            if (!nx_set_test(stem_ss, k)) {
                continue;
            }
            const struct nx_set * s = &transitions[k];
            nx_set_orequal(&end_ss, s);
        }

        if (nx_set_isempty(&end_ss)) {
            continue;
        }
        // cursor->input_index_list[word_index] = i * nx->n_states + k;
        stems[word_index] = input->words[i];
        // TODO: I don't like that this yields multi-words before single words,
        // but going in this order is important for making the cursor work
        if (n_words > word_index + 1) {
            bool rc = nx_combo_match_iter(nx, input, stems, &end_ss, cursor, n_words, word_index + 1, cb);
            if (!rc) {
                return false;
            }
            cursor->input_index_list[word_index + 1] = 0;
        }
        if (nx_set_test(&end_ss, nx->n_states - 1)) {
            struct word wp;
            word_tuple_init(&wp, stems, word_index + 1);
            cb->callback(cb, &wp);
        }
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
    nx_combo_match_iter(nx, input, stems, &start_ss, cursor, n_words, 0, cb);
}

void nx_combo_match(struct nx * nx, const struct wordset * input, size_t n_words, struct cursor * cursor,
                    struct wordset * output, struct wordlist * buffer) {
    struct word_callback * cb = NONNULL(word_callback_create_wordset_add(cursor, buffer, output));
    nx_combo_cache_create(nx, input);
    nx_combo_apply(nx, input, n_words, cursor, cb);
    free(cb);
}

static bool nx_combo_multi_iter(struct nx * const * nxs, size_t n_nxs, const struct wordset * input,
                                const struct word ** stems, const struct nx_set * stem_sss, struct cursor * cursor,
                                size_t n_words, size_t word_index, struct word_callback * cb) {
    for (size_t i = cursor->input_index_list[word_index]; i < input->words_count; i++) {
        cursor->input_index_list[word_index] = i;

        // Check if we've exceeded a deadline
        if (!cursor_update_input(cursor, (word_index == 0) ? i : cursor->input_index)) {
            return false;
        }

        struct nx_set end_sss[n_nxs];
        bool no_match = false;
        bool all_end_match = true;
        for (size_t n = 0; n < n_nxs; n++) {
            end_sss[n] = (struct nx_set){0};
            const struct nx_set * transitions = nx_combo_cache_get(nxs[n], i);

            // This code based on https://lemire.me/blog/2018/02/21/iterating-over-set-bits-quickly/
            for (size_t ki = 0; ki < NX_SET_ARRAYLEN; ki++) {
                uint64_t ks = stem_sss[n].xs[ki];
                while (ks != 0) {
                    size_t r = (size_t)__builtin_ctzl(ks);
                    uint64_t t = ks & -ks;
                    ks ^= t;

                    ASSERT(ki * 64 + r < nxs[n]->n_states);
                    nx_set_orequal(&end_sss[n], &transitions[ki * 64 + r]);
                }
            }

            if (nx_set_isempty(&end_sss[n])) {
                no_match = true;
                break;
            }
            if (!nx_set_test(&end_sss[n], nxs[n]->n_states - 1)) {
                all_end_match = false;
            }
        }
        if (no_match) {
            continue;
        }

        stems[word_index] = input->words[i];
        // TODO: I don't like that this yields multi-words before single words,
        // but going in this order is important for making the cursor work
        if (n_words > word_index + 1) {
            bool rc = nx_combo_multi_iter(nxs, n_nxs, input, stems, end_sss, cursor, n_words, word_index + 1, cb);
            if (!rc) {
                return false;
            }
            cursor->input_index_list[word_index + 1] = 0;
        }
        if (all_end_match) {
            struct word wp;
            word_tuple_init(&wp, stems, word_index + 1);
            cb->callback(cb, &wp);
        }
    }
    if (word_index == 0) {
        cursor_update_input(cursor, input->words_count);
    }
    return true;
}

void nx_combo_multi(struct nx * const * nxs, size_t n_nxs, const struct wordset * input, size_t n_words,
                    struct cursor * cursor, struct word_callback * cb) {
    cursor->total_input_items = input->words_count;
    ASSERT(n_words + 1 <= CURSOR_LIST_MAX);

    int64_t start_ns = now_ns();
    // struct nx_set *sss = NONNULL(calloc(n_nxs * (n_words + 1), sizeof(*sss)));
    struct nx_set sss[n_nxs];
    for (size_t i = 0; i < n_nxs; i++) {
        nx_combo_cache_create(nxs[i], input);

        sss[i] = (struct nx_set){0};
        nx_set_add(&sss[i], 0);
    }
    if (cursor->output_index == 0) {
        LOG("Constructed %zu caches in %ld ms", n_nxs, (now_ns() - start_ns) / 1000000);
    }

    const struct word * stems[n_words];
    nx_combo_multi_iter(nxs, n_nxs, input, stems, sss, cursor, n_words, 0, cb);
}
