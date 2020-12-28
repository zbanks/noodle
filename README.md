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
- Refactor `struct word` now that we aren't pre-processing word lists (preivously done for e.g. anagrams)
    - No need to cache `canonical`/`sorted` forms
        - Could use for "ignore spaces/capitalization/punctuation" rules?
    - If we aren't sorting, there's no need for `value` either (value is implicit from list order)
- Consolidate `struct word_callback` & `struct cursor`
- Building the `combo_cache` should poll the cursor, it can take a long time
- `nx_combo_multi(...)` sort outputs by word length
    - Makes it so the user doesn't need to configure word length
    - Is the easiest way to do this to repeatedly call it with `n_words` set to 1, then 2, etc.?
        - This turns an `O(n^k)` process into `O(n) + O(n^2) + O(n^3) + ... + O(n^k) = O(n^k)` process (big-O notation isn't ideal for analyzing this)
- `nx_combo_multi(...)` could theoretically take an `n_errors` parameter?
