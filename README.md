# Noodle

Tools for solving wordplay puzzles

Heavily inspired by https://tools.qhex.org

## Building & Running

```
$ make PYTHON=python3
$ python3 noodle_app.py
```

This launches the Noodle server bound to http://localhost:8080

## Contents

- `listgen/` - scripts for building word lists (e.g. processing Wikipedia dumps)
- `src/` - primary `libnoodle` source code, this is the primary "engine"
- `static/` - static files for the web frontend
    - `nx.h` - _"Noodle Expressions"_, an expression language based on _regular expressions_. The `nx` engine can do fuzzy matches & multi-word matches.
    - `anatree.h` - Data structure conducive to solving anagram problems, but is currently unused.
- `noodle_app.py` - webserver that hosts frontend for performing Noodle queries. This is the primary entry point.
- `noodle.py` - high-level Python wrapper around `libnoodle` C API
- `noodle-gdb.py` - script to aid debugging `libnoodle` programs from `gdb`

## Noodle Queries

See [Noodle Help](static/help.md).
