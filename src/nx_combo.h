#include "nx.h"
#include "prelude.h"
#include "wordlist.h"

// nx_combo is isolated from the rest of nx_* because it depends
// on word/wordlist/wordset features.

NOODLE_EXPORT int nx_combo_match(const struct nx * nx, const struct wordset * input, size_t n_words, struct wordset * output,
                   struct wordlist * buffer);
