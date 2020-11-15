#pragma once
#include "prelude.h"

NOODLE_EXPORT int64_t now_ns();

#define CURSOR_LIST_MAX ((size_t)16)

struct cursor;
struct cursor {
    size_t input_index;
    size_t output_index;
    size_t input_index_list[CURSOR_LIST_MAX];

    size_t total_input_items;
    int64_t initialize_ns;

    int64_t deadline_ns;
    size_t deadline_output_index;
};

NOODLE_EXPORT void cursor_init(struct cursor * c);
NOODLE_EXPORT void cursor_set_deadline(struct cursor * c, int64_t deadline_ns, size_t deadline_output_index);
NOODLE_EXPORT const char * cursor_debug(const struct cursor * c);

bool cursor_update_input(struct cursor * c, size_t input_index);
bool cursor_update_output(struct cursor * c, size_t output_index);
