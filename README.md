# Noodle

Tools for solving wordplay puzzles. 
Heavily inspired by [tools.qhex.org](https://tools.qhex.org), [Nutrimatic](https://nutrimatic.org), and [Qat](https://www.quinapalus.com/qat.html).

Noodle has the following key features that differentiate it from basic regex/anagram tools:

- Evaluate **complex anagram constraints**
    - "(anagram of `pasta`, except one letter) followed by `led`": `stapled`.
- Generate **phrases** from the input wordlist which match the query
    - `a..mong.u.s.ntencewithmul.iplewords`: `a humongous sentence with multiple words`
- **Fuzzy search**, to find phrases which *almost* match the constraints
    - "phrases within edit distance 2 of `breadfast`": `breakfast(s)`, `broadcast`, `bead east`...
- **Sorted results** where matches with common/long words are prioritized
- **Responsive**, showing matches as they are found, even on long queries


## Building & Running

```
$ cargo run --release --bin noodle-app
```

This launches the Noodle server bound to http://localhost:8082


## Noodle Queries

See [Noodle Help](static/help.md). (TODO)

## To Do

- Document theory of operation (See `architecture.md`)
    - NX basics, tradeoffs (NFA, small alphabet)
    - Fuzzy matches
    - Multi-word matches
    - Multi-NX matches
    - Sugar (anagrams)
- Select wordlist
- Use wordlist from Wikipedia, like Nutrimatic, with rough frequency guides
- Pre/post filters (regex)
- "Extract"/re-write rules for matching "inner" words, etc. ("cross-filtering" on qhex)
- "Inverse" NX expressions? ("does not match") -- (this is hard with NFAs)
- Fuzzy matching + anagrams are weak; add post filter
- Python library
- CLI tool

## License

Released under the [MIT License](LICENSE).

Copyright (c) 2020-2021 Zach Banks

