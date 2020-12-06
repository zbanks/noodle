#include "nx_combo.h"

// This algorithm is pretty naive, and does a lot of re-calculation
// It could probably be drastically improved by adding a caching layer
// around `nx_match_partial(...)` keyed off of `wbuf` & `k`?

// TODO: Checking `now_ns()` constantly in `cursor_update_input(...)` adds a ~15% overhead.
// This can be avoided by running without a deadline_ns
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
            }
            if (nx_set_test(&end_ss, nx->n_states - 1)) {
                struct word wp;
                word_tuple_init(&wp, stems, word_index + 1);
                cb->callback(cb, &wp);
                // LOG("Match: %s", word_debug(&wp));
            }
        }
        first_k = 0;
        cursor->input_index_list[word_index + 1] = 0;
    }
    if (word_index == 0) {
        cursor_update_input(cursor, input->words_count);
    }
    return true;
}

void nx_combo_apply(const struct nx * nx, const struct wordset * input, size_t n_words, struct cursor * cursor,
                    struct word_callback * cb) {
    cursor->total_input_items = input->words_count;
    ASSERT(n_words + 1 <= CURSOR_LIST_MAX);

    struct nx_set start_ss = {0};
    nx_set_add(&start_ss, 0);
    ASSERT(n_words <= WORD_TUPLE_N);
    const struct word * stems[n_words];
    nx_combo_match_iter(nx, input, stems, start_ss, cursor, n_words, 0, cb);
}

void nx_combo_match(const struct nx * nx, const struct wordset * input, size_t n_words, struct cursor * cursor,
                    struct wordset * output, struct wordlist * buffer) {
    struct word_callback * cb = NONNULL(word_callback_create_wordset_add(cursor, buffer, output));
    nx_combo_apply(nx, input, n_words, cursor, cb);
    free(cb);
}
