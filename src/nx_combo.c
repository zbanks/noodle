#include "nx_combo.h"

// This algorithm is pretty naive, and does a lot of re-calculation
// It could probably be drastically improved by adding a caching layer
// around `nx_match_partial(...)` keyed off of `wbuf` & `k`?

static void nx_combo_match_iter(const struct nx * nx, const struct wordset * input, const struct word ** stems,
                                struct nx_set stem_ss, size_t n_words, size_t word_index, struct wordset * output,
                                struct wordlist * buffer) {
    for (size_t i = 0; i < input->words_count; i++) {
        enum nx_char wbuf[256];
        nx_char_translate(str_str(&input->words[i]->canonical), wbuf, 256);
        if (wbuf[0] == NX_CHAR_END) {
            continue;
        }
        for (size_t k = 0; k < nx->n_states; k++) {
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
            stems[word_index] = input->words[i];
            if (nx_set_test(&end_ss, nx->n_states - 1)) {
                struct word wp;
                word_tuple_init(&wp, stems, word_index + 1);
                const struct word * w = wordlist_ensure_owned(buffer, &wp);
                // LOG("Match: %s", word_debug(w));
                if (word_index >= 1) {
                    LOG("Match: %s", word_debug(w));
                }
                wordset_add(output, w);
                // continue;
            }
            if (n_words > word_index + 1) {
                nx_combo_match_iter(nx, input, stems, end_ss, n_words, word_index + 1, output, buffer);
            }
        }
    }
}

int nx_combo_match(const struct nx * nx, const struct wordset * input, size_t n_words, struct wordset * output,
                   struct wordlist * buffer) {
    struct nx_set start_ss = {0};
    nx_set_add(&start_ss, 0);
    ASSERT(n_words <= WORD_TUPLE_N);
    const struct word * stems[n_words];
    nx_combo_match_iter(nx, input, stems, start_ss, n_words, 0, output, buffer);
    return 0;
}
