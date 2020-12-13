#include "nx.h"
#include "prelude.h"
#include "time_util.h"
#include "wordlist.h"

// nx_combo is isolated from the rest of nx_* because it depends
// on word/wordlist/wordset features.

NOODLE_EXPORT void nx_combo_match(struct nx * nx, const struct wordset * input, size_t n_words, struct cursor * cursor,
                                  struct wordset * output, struct wordlist * buffer);

NOODLE_EXPORT void nx_combo_apply(struct nx * nx, const struct wordset * input, size_t n_words, struct cursor * cursor,
                                  struct word_callback * cb);
