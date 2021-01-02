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
- `noodle_app.py` - webserver that hosts frontend for performing Noodle queries. This is the primary entry point.
- `noodle.py` - high-level Python wrapper around `libnoodle` C API
- `noodle-gdb.py` - script to aid debugging `libnoodle` programs from `gdb`

## Noodle Queries

See [Noodle Help](static/help.md).

## To Do

- Document theory of operation
    - NX basics, tradeoffs (NFA, small alphabet)
    - Fuzzy matches
    - Multi-word matches
    - Multi-NX matches
    - Sugar (anagrams)
- Expand `expand_expression()` in Python
    - Handle all qhex operations in `<...>` brackets (subanagram, transadd, etc.)
        - bank/superbank/subbank (`<...:+>`)
        - add/delete/change (`(...:+1)`)
        - substring (`(...:~)`)
- "Extract"/re-write rules for matching "inner" words, etc. ("cross-filtering" on qhex)
- "Inverse" NX expressions? ("does not match") -- is this easy with NFAs?
- Fuzzy matching around spaces is a bit weird
    - Internally `"hello world"` becomes `"_hello__world_"` and spaces are collapsed together. This makes `"helloworld"` be edit distance **2** away instead of 1.
    - Collapsing spaces also leads to `"_hell_no_"` matching `"shelling"` with edit distance 3
    - Only seems to be an issue for internal spaces; in practice it can be constrained with an additional strict filter
