#pragma once

#include "prelude.h"

struct nx;

struct nx * nx_compile(const char * expression);
void nx_destroy(struct nx * nx);

int nx_match(const struct nx * nx, const char * input, size_t n_errors);

void nx_test(void);
