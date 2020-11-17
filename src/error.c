#include "error.h"
#include <stdarg.h>

#define BUFFER_SIZE ((size_t)(32 * 1024))

static char buffers[2][BUFFER_SIZE];
static bool active_buffer;
static size_t buffer_bytes;

const char * error_get_log() {
    active_buffer = !active_buffer;
    buffers[!active_buffer][buffer_bytes] = '\0';
    buffer_bytes = 0;
    return buffers[!active_buffer];
}

void error_write(const char * fmt, ...) {
    va_list ap;

    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);

    va_start(ap, fmt);
    ssize_t rc = vsnprintf(&buffers[active_buffer][buffer_bytes], BUFFER_SIZE - buffer_bytes, fmt, ap);
    va_end(ap);

    if (rc > 0) {
        buffer_bytes += (size_t)rc;
        if (buffer_bytes >= BUFFER_SIZE) {
            buffer_bytes = BUFFER_SIZE;
        }
    }
}
