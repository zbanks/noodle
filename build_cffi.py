#!/usr/bin/env python3

import cffi

ffi = cffi.FFI()
ffi.cdef(
    """
void nx_test(void);
"""
)

ffi.set_source(
    "noodle",
    '#include "../src/libnoodle.h"',
    libraries=["noodle"],
    extra_link_args=["-L..", "-Wl,-rpath,$ORIGIN"],
)

ffi.compile(verbose=True, tmpdir="build")
