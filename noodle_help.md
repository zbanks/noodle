<!-- generated from `pandoc noodle_help.md` -->

### Basics

Specify filters in the input textbox.
Each line is treated as a *noodle expression*.
You can specify multiple expressions, and only results that match *all* expressions are returned.

The query runs until one of the following:

- The input wordlist is exhausted, combining up to **10 words** into a phrase
- **300 results** are returned
- **300 seconds** have passed
- The `Stop` button is pressed

#### Learn More

Noodle is open-source and released under the MIT license.

Visit [GitHub](https://github.com/zbanks/noodle) to fork the code or submit bugs. There is also a command-line version available for running offline/locally.

## Noodle Expressions

### Regular Expressions

Noodle supports the following regular expression syntax: `[…]`, `[^…]`, `.`, `*`, `+`, `?`, `(…)`, `|`, `{…}`.

Before matching, words are converted to lowercase and stripped of whitespace and non-alphabetical symbols (punctuation, numbers).

To explicitly match spaces, include "`!_`" at the end of the expression. When enabled, spaces can be explicitly matched with the "`_`" character.

To explicitly match other symbols, include "`!'`" at the end of the expression. When enabled, these symbols can be matched with the "`'`" character.

Regardless of setting, and unlike normal regular expressions, the period ("`.`") is only equivalent to "`[a-z]`". To match *any* symbol, use "`[a-z'_]`".

Noodle expressions do not support backreferences (e.g. "`\1`").
Additionally, because the input is pre-processed to have a limited alphabet, noodle expressions do not support escape characters, or character classes like "`[:alpha:]`".

### Anagram constraints

Noodle has additional support for anagram-like constriants with angle bracket syntax: `<...>`

- `<abcd>` -- **anagram** of `abcd`: rearranging the given letters
- `<abcd+>` -- **superanagram** of `abcd`: rearranging *at least* the given letters
- `<abcd+3>` -- **transadd** of `3` to `abcd`: rearranging *all* of the given letters *plus* `N` wildcards
- `<abcd->` -- **subanagram** of `abcd`: rearranging *at most* the given letters
- `<abcd-1>` -- **transdelete** of `3` to `abcd`: rerranging *all but `N`* of the given letters
- `(abcd:-)` -- **subset** of `abcd`: contained within a *subset* of the given letters, in the same order
- `(abcd:+)` -- **superset** of `abcd`: contains the *superset* of the given letters, in the same order

Anagram constraints are not compatible with fuzzy matching, and may result in false positives (but not false negatives!).

### Enumerations

Bare numbers are a shortcut to define an *enumeration*. 

The expression `3 3 8 7` looks for a 4-word phrase, consisting of two 3-letter words, followed by an 8-letter word, then a 7-letter word.

### Fuzzy matching

Noodle supports performing *fuzzy matching* for certain expressions.

This will find words & phrases that would match within a given [edit distance](https://en.wikipedia.org/wiki/Levenshtein_distance) of the expression.

To allow matches within edit distance 2, include "`!2`" at the end of the expression.

Fuzzy matching can make queries take much longer, so it works best when there are additional constraints.

#### Fuzzy Caveats

If there are multiple constraints with fuzzy matching, the edits between expressions may not be consistent. For example, `"hey"` will match the query `"hen !1; hay !1"` even though the edits *to get to* "hen" or "hay" are different.

Anagram-like constraints ("`<…>`") are incompatible with fuzzy matching, and may produce false positives.

### Directives

There are a few special directives for modifying how the whole query operates.
They all start with `#`:

- `#limit <N>` -- set the maximum number of results to return. (Example: "`#limit 5000`")
- `#words <N>` -- set the maximum number of words to try to combine into a phrase. "`#words 1`" completely disables phrase matching.

<!--
- `#list <default|small|...>` -- set the input wordlist to use (equivalent to the dropdown)
- `#quiet` -- do not print header/progress information.
-->

## UI Tips

### Keyboard shortcuts

- `Ctrl-Enter` -- submit query

### Google Sheets Integration

You can query Noodle directly from Google Sheets! Here's an example formula:

```
=IMPORTDATA(CONCAT("https://noodle.fly.dev/query/", ENCODEURL("yourqueryhere")))
```

You can wrap it in `TRANSPOSE(...)` to have it fill out horizontally instead of vertically.

You can separate mutli-line queries with semicolons (`;`) instead of newlines.

For `GET` requests like this, the timeout is lowered, default results limit is lowered to 15 (this can be changed with `#limit`).

<!-- end help -->
