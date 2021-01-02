<!-- generated from `pandoc help.md` -->

### Basics

Specify filters in the input textbox.
Each line is treated as a *noodle expression*.
You can specify multiple expressions, and only results that match *all* expressions are returned.

The query runs until one of the following:

- The input wordlist is exhausted, combining up to **10 words** into a phrase
- **300 results** are returned
- **15 seconds** have passed
- The `Stop` button is pressed

### Regex features

Noodle supports the following regualar expression syntax: `[...]`, `[^...]`, `.`, `*`, `+`, `?`, `(...)`, `|`, `{...}`.

Before matching, words are converted to lowercase and stripped of whitespace and non-alphabetical symbols (punctuation, numbers).

To explicitly match spaces, include "`!_`" at the end of the expression. When enabled, the matched phrase is surrounded by spaces, and space characters can be explicitly matched with the "`_`" character.

To explicitly match other symbols, include "`!'`" at the end of the expression. When enabled, these symbols can be matched with the "`'`" character.

Regardless of setting, and unlike normal regular expressions, the period ("`.`") is only equivalent to "`[a-z]`". To match *any* symbol, use "`[a-z'_]`".

### Anagram constraints

Noodle has additional support for anagram-like constriants with angle bracket syntax: `<...>`

- `<abcd>` -- **anagram** of `abcd`: rearranging the given letters
- `<abcd+>` -- **superanagram** of `abcd`: rearranging *at least* the given letters
- `<abcd+3>` -- **transadd** of `3` to `abcd`: rearranging *all* of the given letters *plus* `N` wildcards
- `<abcd->` -- **subanagram** of `abcd`: rearranging *at most* the given letters
- `<abcd-1>` -- **transdelete** of `3` to `abcd`: rerranging *all but `N`* of the given letters
- `(abcd:?)` -- **partial** of `abcd`: contained within a subset of the given letters, in the same order

Anagram constraints are not compatible with fuzzy matching

### Fuzzy matching

Noodle supports performing *fuzzy matching* for certain expressions.

This will find words & phrases that would match within a given [edit distance](https://en.wikipedia.org/wiki/Levenshtein_distance) of the expression.

To allow matches within edit distance 2, include "`!2`" at the end of the expression.

Fuzzy matching can make queries take much longer, so it works best when there are additional constraints.

#### Fuzzy Caveats

Fuzzy matching will treat internal spaces in phrases as *two* spaces, leading to a few false positives.

If there are multiple constraints with fuzzy matching, the edits between expressions may not be consistent. For example, `"hey"` will match the query `"hen !1; hay !1"` even though the edits *to get to* "hen" or "hay" are different.

Anagram-like constraints ("`<...>`") may produce false positives when combined with fuzzy matching.

### Keyboard shortcuts

- `Ctrl-Enter` -- submit query

<!-- end help -->
