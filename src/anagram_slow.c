#include "anagram_slow.h"
#include "bag_util.h"

#define MAX_LENGTH ((size_t)256)
void anagram_slow_iterate(const struct wordset * words, const char * letters, size_t depth, const struct word ** stack,
                          struct cursor * cursor, struct word_callback * cb) {
    ASSERT(depth < WORD_TUPLE_N);

    while (true) {
        size_t i = cursor->input_index_list[depth];
        if (i >= words->words_count) {
            return;
        }

        const char * candidate = str_str(&words->words[i]->sorted);

        char buffer[MAX_LENGTH];
        if (bag_subtract_into(letters, candidate, buffer)) {
            stack[depth] = words->words[i];
            if (buffer[0] == '\0') {
                struct word wp;
                word_tuple_init(&wp, stack, depth + 1);
                cb->callback(cb, &wp);
            } else if (depth + 1 < WORD_TUPLE_N) {
                anagram_slow_iterate(words, buffer, depth + 1, stack, cursor, cb);
            }
        }

        cursor->input_index_list[depth]++;
        cursor->input_index_list[depth + 1] = 0;

        // Check if we've exceeded a deadline
        if (!cursor_update_input(cursor, cursor->input_index_list[0])) {
            return;
        }
    }
}

void anagram_slow(const struct wordset * words, const char * sorted, struct cursor * cursor,
                  struct word_callback * cb) {
    if (strlen(sorted) + 1 >= MAX_LENGTH) {
        LOG("Input string is too long (%zu >= %zu)", strlen(sorted) + 1, MAX_LENGTH);
        return;
    }
    const struct word * stack[WORD_TUPLE_N] = {0};
    anagram_slow_iterate(words, sorted, 0, stack, cursor, cb);
}
