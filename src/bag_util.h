#pragma once

#include "prelude.h"

// Returns true if `subset` is a subset of the characters in `superset`,
// and `|superset - subset| <= max_size`
// All strings represent bags of characters, so must be sorted
bool bag_difference_size_less_than(const char * superset, const char * subset, size_t max_size);

// Returns true if all of the characters in `subset` are in `superset`,
// with the characters only in `superset` copied into `output`.
// The state of `output` is undefined if this funciton returns false.
// All strings represent bags of characters, so must be sorted
bool bag_subtract_into(const char * superset, const char * subset, char * output);
