#pragma once
#include "prelude.h"

NOODLE_EXPORT const char * error_get_log(void);

#define NOODLE_PRINTF __attribute__((format(printf, 1, 2)))
NOODLE_EXPORT NOODLE_PRINTF void error_write(const char * fmt, ...);
