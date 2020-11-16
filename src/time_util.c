#include "time_util.h"
#include <time.h>

int64_t now_ns() {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1000000000 + t.tv_nsec;
}

void cursor_init(struct cursor * c) {
    *c = (struct cursor){
        .input_index = 0,
        .output_index = 0,

        .total_input_items = 0,
        .initialize_ns = now_ns(),

        .deadline_ns = 0,
        .deadline_output_index = 0,
    };
}

void cursor_set_deadline(struct cursor * c, int64_t deadline_ns, size_t deadline_output_index) {
    c->deadline_ns = deadline_ns;
    c->deadline_output_index = deadline_output_index;
}

const char * cursor_debug(const struct cursor * c) {
    static char buffer[2048];
    int64_t now = now_ns();
    snprintf(buffer, sizeof(buffer), "%zu/%zu (%0.2lf%%) input; %zu output; in %0.0lfms", c->input_index,
             c->total_input_items, 100.0 * (double)c->input_index / (double)c->total_input_items, c->output_index,
             (double)(now - c->initialize_ns) / 1e6);
    return buffer;
}

bool cursor_update_input(struct cursor * c, size_t input_index) {
    ASSERT(input_index <= c->total_input_items);
    c->input_index = input_index;
    if (c->input_index >= c->total_input_items) {
        return false;
    }
    if (c->deadline_output_index != 0 && c->output_index >= c->deadline_output_index) {
        return false;
    }
    if (c->deadline_ns != 0 && now_ns() > c->deadline_ns) {
        return false;
    }
    return true;
}

bool cursor_update_output(struct cursor * c, size_t output_index) {
    c->output_index = output_index;
    if (c->deadline_output_index != 0 && c->output_index >= c->deadline_output_index) {
        return false;
    }
    return true;
}
