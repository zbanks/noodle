#pragma once

#define NOODLE_EXPORT __attribute__((visibility("default")))

#include "error.h"

#define typeof __typeof__

//#define LOG(msg, ...) fprintf(stderr, "[%s:%s:%d] " msg "\n", __FILE__, __FUNCTION__, __LINE__, ##__VA_ARGS__)
#define LOG(msg, ...) error_write("[%s:%s:%d] " msg "\n", __FILE__, __FUNCTION__, __LINE__, ##__VA_ARGS__)
#define PLOG(msg, ...) LOG(msg " (%s)", ##__VA_ARGS__, strerror(errno))

#ifdef DEBUG
#define ASSERT(x)                                                                                                      \
    ({                                                                                                                 \
        if (__builtin_expect(!(x), 0)) {                                                                               \
            LOG("Assertion failed: " STRINGIFY(x));                                                                    \
            abort();                                                                                                   \
        }                                                                                                              \
    })
#else
#define ASSERT(x) ((void)(x))
#endif

#define NONNULL(x)                                                                                                     \
    ({                                                                                                                 \
        typeof(x) _x = (x);                                                                                            \
        ASSERT((_x) != NULL);                                                                                          \
        _x;                                                                                                            \
    })

#define MIN(a, b)                                                                                                      \
    ({                                                                                                                 \
        typeof(a) _a = (a);                                                                                            \
        typeof(b) _b = (b);                                                                                            \
        (_a < _b) ? _a : _b;                                                                                           \
    })
#define MAX(a, b)                                                                                                      \
    ({                                                                                                                 \
        typeof(a) _a = (a);                                                                                            \
        typeof(b) _b = (b);                                                                                            \
        (_a > _b) ? _a : _b;                                                                                           \
    })

#define STRINGIFY(x) STRINGIFY2(x)
#define STRINGIFY2(x) #x

#define CONCAT(x, y) CONCAT2(x, y)
#define CONCAT2(x, y) x##y
