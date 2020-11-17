#!/usr/bin/env python3
#
# Build noodle.cpython-*.so using cffi
#

import cffi
import re

# This is a crude parser that depends on "src/libnoodle.h" only directly including other noodle headers
# and the remaining headers to be marked with NOODLE_EXPORT & be formatted with clang-format
#
# If libnoodle were to become a "real" shared library, all of the relevant decls would need to be
# directly specifed in libnoodle.h anyways and this step could be simplified/removed

include_files = []
with open("src/libnoodle.h") as f:
    for line in f:
        if "#include" not in line:
            continue
        include_files.append(line.split('"')[-2])

cdefs = ""
for filename in include_files:
    with open("src/{}".format(filename)) as f:
        exporting = False
        for line in f:
            line = line.replace("NOODLE_PRINTF", "")
            if "NOODLE_EXPORT" in line:
                cdefs += line.replace("NOODLE_EXPORT", "")
                exporting = True
            elif (
                any(line.startswith(k) for k in ("struct", "enum", "union"))
                and "{" in line
            ):
                assert line.strip().endswith("{"), (filename, line)
                cdefs += line
                exporting = True
            elif line.startswith("#define"):
                replace = re.sub(
                    r"(#define +[A-Z0-9_]+) +\(\([a-z_]+\)([0-9.xflu]+)\)",
                    r"\1 \2",
                    line,
                )
                if replace != line:
                    cdefs += replace
                exporting = False
            elif exporting and (line.startswith(" " * 4) or not line.strip()):
                # Multi-line decls will start with at least 4 spaces after being clang-format'd
                cdefs += line
            elif exporting and line.startswith("}"):
                cdefs += line
                exporting = False
            else:
                exporting = False

# Build the FFI library & set RPATH to search for libnoodle.so in the same folder (ORIGIN)
ffi = cffi.FFI()
ffi.cdef(cdefs)
ffi.set_source(
    "noodle_ffi",
    '#include "../src/libnoodle.h"',
    libraries=["noodle"],
    extra_link_args=["-L..", "-Wl,-rpath,$ORIGIN"],
)

ffi.compile(verbose=True, tmpdir="build")
