#include "nx_combo.h"

struct nx_combo_cache {
    struct cache_class {
        size_t n_words;
        const struct word ** words;

        struct nx_set nonnull_transitions;

        // Array of initial_state -> final_stateset
        struct nx_set * transitions;
    } * classes;
    size_t n_classes;

    struct cache_class ** word_classes;
    struct wordset nonnull_wordset;

    const struct wordset * wordset;
    size_t wordset_progress;
};

void nx_combo_cache_destroy(struct nx_combo_cache * cache) {
    if (cache == NULL) {
        return;
    }
    for (size_t j = 0; j < cache->n_classes; j++) {
        free(cache->classes[j].words);
        free(cache->classes[j].transitions);
    }
    free(cache->classes);
    free(cache->word_classes);
    wordset_term(&cache->nonnull_wordset);
    free(cache);
}

static void nx_combo_cache_create(struct nx * nx, const struct wordset * input, struct cursor * cursor) {
    NONNULL(nx);
    NONNULL(input);

    int64_t start_ns = now_ns();

    ASSERT(nx->fuzz <= NX_FUZZ_MAX);
    size_t transitions_size = nx->n_states * (nx->fuzz + 1) * sizeof(struct nx_set);
    struct nx_combo_cache * cache = nx->combo_cache;

    if (cache == NULL) {
        cache = NONNULL(calloc(1, sizeof(*cache)));
        nx->combo_cache = cache;
        cache->wordset = input;
        cache->wordset_progress = 0;
        wordset_init(&cache->nonnull_wordset);

        cache->classes = NONNULL(calloc(input->words_count + 1, sizeof(*cache->classes)));
        cache->word_classes = NONNULL(calloc(input->words_count, sizeof(*cache->word_classes)));

        // The first class is always the "empty" class (complete no-match)
        cache->classes[0].transitions = NONNULL(calloc(1, transitions_size));
        cache->n_classes++;
    } else if (cache->wordset_progress >= input->words_count) {
        ASSERT(cache->wordset_progress == input->words_count);
        return;
    }

    for (size_t i = cache->wordset_progress; i < input->words_count; i++) {
        enum nx_char wbuf[256];
        nx_char_translate(nx, word_str(input->words[i]), wbuf, 256);
        ASSERT(wbuf[0] != NX_CHAR_END);

        // XXX This is an "O(n^2)ish" algorithm that probably could be done in "O(n)ish"
        // if we implement filling the whole transition table in one shot?
        struct nx_set transitions[nx->n_states * (nx->fuzz + 1)];
        memset(transitions, 0, sizeof(transitions));
        for (size_t k = 0; k < nx->n_states; k++) {
            nx_match_partial(nx, wbuf, (uint16_t)k, &transitions[k * (nx->fuzz + 1)]); // XXX
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

            // XXX
            for (size_t k = 0; k < nx->n_states; k++) {
                if (!nx_set_isempty(&transitions[k * (nx->fuzz + 1)])) {
                    nx_set_add(&cache->word_classes[i]->nonnull_transitions, k);
                }
            }

            if (cache->n_classes < 20) {
                LOG("%zu: nonnull: %s: %s", cache->n_classes - 1,
                    nx_set_debug(&cache->word_classes[i]->nonnull_transitions), word_str(input->words[i]));
            } else if (cache->n_classes == 20) {
                LOG("%zu: nonnull: %s: %s (...and so on)", cache->n_classes - 1,
                    nx_set_debug(&cache->word_classes[i]->nonnull_transitions), word_str(input->words[i]));
            }
        }
        if (cache->word_classes[i] != &cache->classes[0]) {
            wordset_add(&cache->nonnull_wordset, input->words[i]);
        }

        cache->wordset_progress = i + 1;
        if (!cursor_update_input(cursor, cursor->input_index)) {
            return;
        }
    }
    ASSERT(cache->wordset_progress == input->words_count);

    // This part should be ~fast to do at once
    size_t n_words_cumulative = 0;
    for (size_t j = 0; j < cache->n_classes; j++) {
        struct cache_class * class = &cache->classes[j];
        class->words = NONNULL(calloc(class->n_words, sizeof(*class->words)));

        n_words_cumulative += class->n_words;
        class->n_words = 0;
    }
    ASSERT(n_words_cumulative == input->words_count);
    for (size_t i = 0; i < input->words_count; i++) {
        struct cache_class * class = cache->word_classes[i];
        class->words[class->n_words++] = input->words[i];
    }

    free(cache->classes[0].transitions);
    cache->classes[0].transitions = NULL;

    LOG("Populated cache of %zu words in " PRIlong "ms: %zu classes, %zu non-null", input->words_count,
        (now_ns() - start_ns) / 1000000, cache->n_classes, cache->nonnull_wordset.words_count);
}

static int nx_combo_cache_compress(struct nx * nx, const struct wordset * new_input) {
    // Recompute the NX cache for a new input set
    //
    // `new_input` *must* be a subsnxs[i]->combo_cache == NULL || nxs[i
    //
    // NB: This only removes the words from the `word_classes` lookup, it does not
    // shrink the class word lists or remove now-unused classes.
    //
    // NB: If `|new_input| << |cache->wordset|` then it may be faster to completely wipe the
    // cache and re-build it? (But this would just make fast queries faster; not help with slow ones)

    NONNULL(nx);
    NONNULL(new_input);

    struct nx_combo_cache * cache = nx->combo_cache;
    ASSERT(cache->wordset->words_count == cache->wordset_progress);
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
            ASSERT(w != NULL);
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
    cache->wordset_progress = new_input->words_count;
    return 0;
}

static const struct cache_class * nx_combo_cache_get(const struct nx * nx, size_t word_index) {
    return nx->combo_cache->word_classes[word_index];
}

enum multi_return {
    MULTI_RETURN_CONTINUE,
    MULTI_RETURN_DONE,
};

static enum multi_return nx_combo_multi_iter(struct nx * const * nxs, size_t n_nxs, const struct wordset * input,
                                             const struct word ** stems, const struct nx_set (*stem_sss)[NX_FUZZ_MAX],
                                             struct cursor * cursor, size_t n_words, size_t word_index) {
    for (size_t i = cursor->input_index_list[word_index]; i < cursor->total_input_items; i++) {
        cursor->input_index_list[word_index] = i;

        // Check if we've exceeded a deadline
        if (!cursor_update_input(cursor, (word_index == 0) ? i : cursor->input_index)) {
            return MULTI_RETURN_CONTINUE;
        }

        struct nx_set end_sss[n_nxs][NX_FUZZ_MAX];
        memset(end_sss, 0, sizeof(end_sss));
        bool no_match = false;
        bool all_end_match = true;
        for (size_t n = 0; n < n_nxs; n++) {
            const struct cache_class * class = nx_combo_cache_get(nxs[n], i);
            const struct nx_set * transitions = class->transitions;
            if (transitions == NULL) {
                no_match = true;
                break;
            }

            // XXX: I don't think this is sound anymore
            // Unclear if this optimization helps
            // bool all_empty = true;
            // for (size_t fi = 0; fi <= nx->fuzz; fi++) {
            //    if (nx_set_intersect(&class->nonnull_transitions, &stem_sss[n][0])) {
            //        all_empty = false;
            //        break;
            //    }
            //}
            // if (all_empty) {
            //    no_match = true;
            //    break;
            //}

            for (size_t fi = 0; fi <= nxs[n]->fuzz; fi++) {
                // This code based on https://lemire.me/blog/2018/02/21/iterating-over-set-bits-quickly/
                for (size_t ki = 0; ki < (nxs[n]->n_states + 63) / 64; ki++) {
                    uint64_t ks = stem_sss[n][fi].xs[ki];
                    while (ks != 0) {
                        size_t r = (size_t)__builtin_ctzl(ks); // TODO: depends on sizeof(long) __EMSCRIPTEN__
                        uint64_t t = ks & -ks;
                        ks ^= t;

                        size_t idx = ki * 64 + r;
                        if (idx >= nxs[n]->n_states) {
                            break;
                        }
                        for (size_t fd = 0; fi + fd <= nxs[n]->fuzz; fd++) {
                            nx_set_orequal(&end_sss[n][fi + fd], &transitions[idx * (nxs[n]->fuzz + 1) + fd]);
                        }
                    }
                }
            }

            // TODO: // This branch should be impossible, after the earlier nonnull_transitions test
            bool all_empty = true;
            bool any_end_match = false;
            for (size_t fi = 0; fi <= nxs[n]->fuzz; fi++) {
                if (nx_set_test(&end_sss[n][fi], nxs[n]->n_states - 1)) {
                    any_end_match = true;
                    all_empty = false;
                    break;
                } else if (!nx_set_isempty(&end_sss[n][fi])) {
                    all_empty = false;
                }
            }
            if (all_empty) {
                ASSERT(!any_end_match);
                no_match = true;
                break;
            }
            if (!any_end_match) {
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

        if (n_words > word_index + 1) {
            enum multi_return rc =
                nx_combo_multi_iter(nxs, n_nxs, input, stems, end_sss, cursor, n_words, word_index + 1);
            if (rc == MULTI_RETURN_CONTINUE) {
                return rc;
            }
            cursor->input_index_list[word_index + 1] = 0;
        } else if (all_end_match) {
            struct word wp;
            word_tuple_init(&wp, stems, word_index + 1);
            cursor->callback(cursor, &wp);
        } else {
            cursor->has_partial_match = true;
        }
    }
    if (word_index == 0) {
        cursor_update_input(cursor, cursor->total_input_items);
    }
    return MULTI_RETURN_DONE;
}

void nx_combo_multi(struct nx * const * nxs, size_t n_nxs, const struct wordset * input, size_t n_words,
                    struct cursor * cursor) {
    ASSERT(nxs != NULL);
    ASSERT(n_nxs > 0);
    ASSERT(input != NULL);
    ASSERT(n_words + 1 <= CURSOR_LIST_MAX);
    ASSERT(cursor != NULL);

    if (!cursor->setup_done) {
        cursor->total_input_items = n_nxs;
        for (size_t i = 0; i < n_nxs; i++) {
            nx_combo_cache_create(nxs[i], input, cursor);

            if (!cursor_update_input(cursor, i)) {
                return;
            }

            ASSERT(nxs[i]->combo_cache != NULL);
            input = &nxs[i]->combo_cache->nonnull_wordset;
        }

        // This should be ~fast to do at once
        for (size_t i = 0; i < n_nxs; i++) {
            nx_combo_cache_compress(nxs[i], input);
        }

        cursor->setup_done = true;
        cursor->total_input_items = input->words_count;
        memset(cursor->input_index_list, 0, sizeof(cursor->input_index_list));
        cursor->word_index = 1;
        cursor->has_partial_match = false;
    } else {
        input = &nxs[n_nxs - 1]->combo_cache->nonnull_wordset;
    }

    struct nx_set sss[n_nxs][NX_FUZZ_MAX];
    memset(sss, 0, sizeof(sss));
    for (size_t i = 0; i < n_nxs; i++) {
        enum nx_char end[1] = {NX_CHAR_END};
        nx_match_partial(nxs[i], end, 0, sss[i]);
    }

    while (1) {
        const struct word * stems[n_words];
        enum multi_return rc = nx_combo_multi_iter(nxs, n_nxs, input, stems, sss, cursor, cursor->word_index, 0);

        if (rc == MULTI_RETURN_DONE && cursor->has_partial_match && cursor->word_index < n_words) {
            cursor->total_input_items = input->words_count;
            memset(cursor->input_index_list, 0, sizeof(cursor->input_index_list));
            cursor->word_index++;
            cursor->has_partial_match = false;
        } else {
            return;
        }
    }
}
