#pragma once

#include "prelude.h"
#include "time_util.h"
#include "word.h"
#include "wordlist.h"

// This is the most naive O(n^k) algorithm
// It's meant as a placeholder until the anatree datastructure can do partial lookups
NOODLE_EXPORT void anagram_slow(const struct wordset * words, const char * sorted, struct cursor * cursor,
                                struct word_callback * cb);
