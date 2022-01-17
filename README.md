# Noodle

Tools for solving wordplay puzzles. 
Heavily inspired by [tools.qhex.org](https://tools.qhex.org), [Nutrimatic](https://nutrimatic.org), and [Qat](https://www.quinapalus.com/qat.html).

Noodle has the following key features that differentiate it from basic regex/anagram tools:

- Evaluate **complex anagram constraints**
    - "(anagram of `pasta`, except one letter) followed by `led`": `stapled`.
- Generate **phrases** from the input wordlist which match the query
    - `a..mong.u.s.ntencewithmul.iplewords`: `a humongous sentence with multiple words`
- **"Fuzzy" search**, to find phrases which *almost* match the constraints
    - "phrases within edit distance 2 of `breadfast`": `breakfast(s)`, `broadcast`, `bead east`...
- **Sorted results** where matches with common/long words are prioritized
- **Responsive**, showing matches as they are found, even on long queries
- A **simple GET API**, allowing it to be easily integrated into Google Sheets or bots


## Building & Running

### Command-line Application

```
$ cargo install --path noodle-cli
```

This installs a `noodle` binary to your path.

```
> noodle --help
noodle 0.1.0

USAGE:
    noodle [OPTIONS] <query>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -n, --count <count>                    Number of results to return
    -i, --input <input>                    Input wordlist file [default: /usr/share/dict/words]
    -m, --phrase-length <phrase-length>    Maximum number of words to combine to make a matching phrase [default: 10]

ARGS:
    <query>    Noodle query string
```

### Web Application

```
$ cargo run --release --bin noodle-webapp
```

This launches the Noodle server bound to http://localhost:8082


### Deploy to fly.io

```
$ flyctl launch
```

This will create & launch a [fly.io](https://fly.io/docs/introduction/) application (named `noodle`).

(Although fly.io is a bit overkill as a hosting provider, they have a generous & easy-to-use free tier.)


## Noodle Queries

See [Noodle Help](noodle_help.md).

## To Do

- Document theory of operation (See `architecture.md`)
    - NX basics, tradeoffs (NFA, small alphabet)
    - Fuzzy matches
    - Multi-word matches
    - Multi-NX matches
    - Sugar (anagrams)
- Select wordlist
- Use wordlist from Wikipedia, like Nutrimatic, with rough frequency guides
- Custom wordlist in web interface
- Mix & match wordlists ("one word from this list of 100 words, plus any other 3 words")
- Pre/post filters (regex)
- "Extract"/re-write rules for matching "inner" words, etc. ("cross-filtering" on qhex)
- "Inverse" NX expressions? ("does not match") -- (this is hard with NFAs)
- Fuzzy matching + anagrams are weak; add post filter
- Python library
- Other ways of sorting the output in the UI (e.g. by length, alphabetical, etc.)
- Add support for `(...:-n)`/`(...:+n)` syntax
- More powerful macro/preprocessing language?
    - "Length of macro" would help with certain repetitive lookups
- Feedback form in web UI
- Make single-word searches faster by not building full transition tables
- Heuristically re-sort constraints from most-to-least constraining (for speed)

## License

Released under the [MIT License](LICENSE).

Copyright (c) 2020-2021 Zach Banks

