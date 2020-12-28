#include "nx_combo.h"

struct nx_combo_cache {
    struct cache_class {
        size_t n_words;
        size_t n_words_cumulative;
        const struct word ** words;

        struct nx_set nonnull_transitions;

        // Array of initial_state -> final_stateset
        struct nx_set * transitions;
    } * classes;
    size_t n_classes;

    struct cache_class ** word_classes;
    struct wordset nonnull_wordset;

    const struct wordset * wordset;
    size_t wordset_size;
};

static void nx_combo_cache_destroy(struct nx_combo_cache * cache) {
    for (size_t j = 0; j < cache->n_classes; j++) {
        free(cache->classes[j].words);
        free(cache->classes[j].transitions);
    }
    free(cache->classes);
    free(cache->word_classes);
    wordset_term(&cache->nonnull_wordset);
    free(cache);
}

static void nx_combo_cache_create(struct nx * nx, const struct wordset * input) {
    NONNULL(nx);
    NONNULL(input);

    if (nx->combo_cache != NULL) {
        if (nx->combo_cache->wordset == input && nx->combo_cache->wordset_size == input->words_count) {
            return;
        }
        nx_combo_cache_destroy(nx->combo_cache);
        nx->combo_cache = NULL;
    }
    struct nx_combo_cache * cache = NONNULL(calloc(1, sizeof(*nx->combo_cache)));
    nx->combo_cache = cache;
    cache->wordset = input;
    cache->wordset_size = input->words_count;
    wordset_init(&cache->nonnull_wordset);

    int64_t start_ns = now_ns();
    size_t transitions_size = nx->n_states * sizeof(struct nx_set);
    cache->classes = NONNULL(calloc(input->words_count + 1, sizeof(*cache->classes)));
    cache->word_classes = NONNULL(calloc(input->words_count, sizeof(*cache->word_classes)));

    // The first class is always the "empty" class (complete no-match)
    cache->classes[0].transitions = NONNULL(calloc(1, transitions_size));
    cache->n_classes++;

    for (size_t i = 0; i < input->words_count; i++) {
        enum nx_char wbuf[256];
        nx_char_translate(word_cstr(input->words[i]), wbuf, 256);
        ASSERT(wbuf[0] == NX_CHAR_SPACE);
        ASSERT(wbuf[1] != NX_CHAR_END);
        ASSERT(wbuf[2] != NX_CHAR_END);

        // XXX This is an "O(n^2)ish" algorithm that probably could be done in "O(n)ish"
        // if we implement filling the whole transition table in one shot
        struct nx_set transitions[nx->n_states];
        for (size_t k = 0; k < nx->n_states; k++) {
            // Use `&wbuf[1]` to skip initial space
            transitions[k] = nx_match_partial(nx, &wbuf[1], (uint16_t)k);
        }

        for (size_t j = 0; j < cache->n_classes; j++) {
            if (memcmp(transitions, cache->classes[j].transitions, transitions_size) == 0) {
                cache->word_classes[i] = &cache->classes[j];
                cache->classes[j].n_words++;
                break;
            }
        }
        if (cache->word_classes[i] == NULL) {
            cache->word_classes[i] = &cache->classes[cache->n_classes++];
            cache->word_classes[i]->transitions = NONNULL(calloc(1, transitions_size));
            memcpy(cache->word_classes[i]->transitions, transitions, transitions_size);
            cache->word_classes[i]->n_words++;

            for (size_t k = 0; k < nx->n_states; k++) {
                if (!nx_set_isempty(&transitions[k])) {
                    nx_set_add(&cache->word_classes[i]->nonnull_transitions, k);
                }
            }

            if (cache->n_classes < 20) {
                LOG("%zu: nonnull: %s: %s", cache->n_classes - 1,
                    nx_set_debug(&cache->word_classes[i]->nonnull_transitions), word_debug(input->words[i]));
            } else if (cache->n_classes == 20) {
                LOG("%zu: nonnull: %s: %s (...and so on)", cache->n_classes - 1,
                    nx_set_debug(&cache->word_classes[i]->nonnull_transitions), word_debug(input->words[i]));
            }
        }
        if (cache->word_classes[i] != &cache->classes[0]) {
            wordset_add(&cache->nonnull_wordset, input->words[i]);
        }
    }
    cache->classes = NONNULL(realloc(cache->classes, cache->n_classes * sizeof(*cache->classes)));
    size_t n_words_cumulative = 0;
    for (size_t j = 0; j < cache->n_classes; j++) {
        struct cache_class * class = &cache->classes[j];
        class->words = NONNULL(calloc(class->n_words, sizeof(*class->words)));

        n_words_cumulative += class->n_words;
        class->n_words_cumulative = n_words_cumulative;
        class->n_words = 0;
    }
    ASSERT(n_words_cumulative == input->words_count);
    for (size_t i = 0; i < input->words_count; i++) {
        struct cache_class * class = cache->word_classes[i];
        class->words[class->n_words++] = input->words[i];
    }

    free(cache->classes[0].transitions);
    cache->classes[0].transitions = NULL;

    LOG("Populated cache of %zu words in %ldms: %zu classes, %zu non-null", input->words_count,
        (now_ns() - start_ns) / 1000000, cache->n_classes, cache->nonnull_wordset.words_count);
}

static int nx_combo_cache_compress(struct nx * nx, const struct wordset * new_input) {
    // Recompute the NX cache for a new input set
    //
    // `new_input` *must* be a subset of the original `cache->wordset`, in the *same order*
    //
    // NB: This only removes the words from the `word_classes` lookup, it does not
    // shrink the class word lists or remove now-unused classes.
    //
    // NB: If `|new_input| << |cache->wordset|` then it may be faster to completely wipe the
    // cache and re-build it? (But this would just make fast queries faster; not help with slow ones)

    NONNULL(nx);
    NONNULL(new_input);

    struct nx_combo_cache * cache = nx->combo_cache;
    ASSERT(cache->wordset->words_count == cache->wordset_size);
    if (cache->wordset == new_input) {
        return 0;
    }

    struct cache_class ** new_word_classes = NONNULL(calloc(new_input->words_count, sizeof(*new_word_classes)));

    size_t j = 0;
    for (size_t i = 0; i < new_input->words_count; i++) {
        const struct word * nw = wordset_get(new_input, i);
        const struct word * w;
        while (1) {
            w = wordset_get(cache->wordset, j);
            if (w == NULL) {
                // XXX: This isn't really recoverable?
                LOG("Error: word %s was not in original wordset", word_debug(nw));
                free(new_word_classes);
                return -1;
            }
            if (w == nw) {
                break;
            }
            j++;
        }
        NONNULL(w);

        new_word_classes[i] = cache->word_classes[j];
        j++;
    }

    free(cache->word_classes);
    cache->word_classes = new_word_classes;
    cache->wordset = new_input;
    cache->wordset_size = new_input->words_count;
    return 0;
}

static const struct cache_class * nx_combo_cache_get(const struct nx * nx, size_t word_index) {
    return nx->combo_cache->word_classes[word_index];
}

static bool nx_combo_multi_iter(struct nx * const * nxs, size_t n_nxs, const struct wordset * input,
                                const struct word ** stems, const struct nx_set * stem_sss, struct cursor * cursor,
                                size_t n_words, size_t word_index, struct word_callback * cb) {
    for (size_t i = cursor->input_index_list[word_index]; i < cursor->total_input_items; i++) {
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
            const struct cache_class * class = nx_combo_cache_get(nxs[n], i);
            const struct nx_set * transitions = class->transitions;
            if (transitions == NULL) {
                no_match = true;
                break;
            }

            // Unclear if this optimization helps
            if (!nx_set_intersect(&class->nonnull_transitions, &stem_sss[n])) {
                no_match = true;
                break;
            }

            // This code based on https://lemire.me/blog/2018/02/21/iterating-over-set-bits-quickly/
            for (size_t ki = 0; ki < (nxs[n]->n_states + 63) / 64; ki++) {
                uint64_t ks = stem_sss[n].xs[ki];
                while (ks != 0) {
                    size_t r = (size_t)__builtin_ctzl(ks);
                    uint64_t t = ks & -ks;
                    ks ^= t;

                    size_t idx = ki * 64 + r;
                    if (idx >= nxs[n]->n_states) {
                        break;
                    }
                    nx_set_orequal(&end_sss[n], &transitions[idx]);
                }
            }

            // This branch should be impossible, after the earlier nonnull_transitions test
            if (nx_set_isempty(&end_sss[n])) {
                ASSERT(0);
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
        // Skip words that don't advance the NFAs?
        // The idea being if a user queries for `a.*b` they don't care about every word combination
        // that can fill in the `.*` portion -- but in pratice it is hard to interpret, maybe should be disabled?
        if (memcmp(end_sss, stem_sss, sizeof(end_sss)) == 0) {
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
        cursor_update_input(cursor, cursor->total_input_items);
    }
    return true;
}

void nx_combo_multi(struct nx * const * nxs, size_t n_nxs, const struct wordset * input, size_t n_words,
                    struct cursor * cursor, struct word_callback * cb) {
    ASSERT(nxs != NULL);
    ASSERT(n_nxs > 0);
    ASSERT(input != NULL);
    ASSERT(n_words + 1 <= CURSOR_LIST_MAX);
    ASSERT(cursor != NULL);
    ASSERT(cb != NULL);

    cursor->total_input_items = input->words_count;

    for (size_t i = 0; i < n_nxs; i++) {
        if (nxs[i]->combo_cache == NULL) {
            // XXX this doesn't validate that the input wordset hasn't changed
            nx_combo_cache_create(nxs[i], input);
        }
        ASSERT(nxs[i]->combo_cache != NULL);
        input = &nxs[i]->combo_cache->nonnull_wordset;

        // Building the cache can be slow, check if we exceeded the time limit
        if (!cursor_update_input(cursor, cursor->input_index)) {
            return;
        }
    }
    cursor->total_input_items = input->words_count;

    struct nx_set sss[n_nxs];
    for (size_t i = 0; i < n_nxs; i++) {
        nx_combo_cache_compress(nxs[i], input);

        enum nx_char space[2] = {NX_CHAR_SPACE, NX_CHAR_END};
        sss[i] = nx_match_partial(nxs[i], space, 0);
    }

    const struct word * stems[n_words];
    nx_combo_multi_iter(nxs, n_nxs, input, stems, sss, cursor, n_words, 0, cb);
}
