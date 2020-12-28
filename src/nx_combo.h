#include "nx.h"
#include "prelude.h"
#include "time_util.h"
#include "wordlist.h"

// nx_combo is isolated from the rest of nx_* because it depends
// on word/wordlist/wordset features.

// Match multiple words against multiple expressions at the same time
// Can be used for complex matching, like anagrams
NOODLE_EXPORT void nx_combo_multi(struct nx * const * nxs, size_t n_nxs, const struct wordset * input, size_t n_words,
                                  struct cursor * cursor, struct word_callback * cb);
