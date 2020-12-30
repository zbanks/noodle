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
- `nx_combo_multi(...)` sort outputs by word length in what I'd call a "`O(n) + O(n^2) + O(n^3) + ... + O(n^k)`" process
    - Although this *is* equivalent to `O(n^k)` in big-O, we may be able to get better constant factors by caching the intermediate state?
- `nx_combo_multi(...)` could theoretically take an `n_errors` parameter?
- Clean up explicit space/other handling
    - Flag for explicit other/punctuation
    - Convert accented letters to bare (probably in list generation?)
- Expand `expand_expression()` in Python
    - Handle all qhex operations in `<...>` brackets (subanagram, transadd, etc.)
        - bank/superbank/subbank (`<...:+>`)
        - add/delete/change (`(...:+1)`)
        - substring (`(...:~)`)
    - Set per-NX flags
    - Handle enumertations (bare "`5 1 3-1" rewrites to "`_....._._...-._`")
- "Extract"/re-write rules for matching "inner" words, etc. ("cross-filtering" on qhex)
- "Inverse" NX expressions? ("does not match") -- is this easy with NFAs?
