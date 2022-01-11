<!-- generated from `pandoc noodle_help.md` -->
## Quick Reference

- `a` or `A`  -- **the letter** `a` itself (case-insensitive)
- `'` (apostrophe) -- **any punctuation**, such as an apostrophe, dash, or period.
- `_` (underscore) -- **space** (word boundary)
- `.` -- **any letter**
- `[abc]` or `[a-c]` -- **one of** `a`, `b`, or `c`
- `[^abc]` -- **one letter except** `a`, `b`, or `c`
- `*`  -- **zero or more** copies of the previous symbol
- `+`  -- **one or more** copies of the previous symbol
- `?`  -- **one or zero** copies of the previous symbol
- `{3}`  -- **exactly three** copies of the previous symbol
- `{3,}`  -- **at least three** copies of the previous symbol
- `{,5}`  -- **at most five** copies of the previous symbol
- `{3,5}`  -- **between three and five** (inclusive) copies of the previous symbol
- `(abcd)` -- **group** of `abcd`, usually used with `*`, `+`, `?`, or `{…}`.
- `(ab|cd|ef)` -- **either** `ab`, `cd`, or `ef`
- `<ate>` -- **anagram** of `ate`: `ate`, `eat`, `eta`, `tea`
- `<ate+>` -- **superanagram** of `ate`: `abate`, `acute`, `fated`, `neat`, …
- `<ate+3>` -- **transadd** of `3` to `ate`: `abated`, `advent`, `basket`, …
- `<ate->` -- **subanagram** of `ate`: `ate`, `at`, `a`, `eat`, …
- `<ate-1>` -- **transdelete** of `1` to `ate`: `at`, `Ta`
- `(ate:-)` -- **subset** of `ate`: `ate`, `at`, `a`
- `(ate:+)` -- **superset** of `ate`: `abate`, `acted`, `fated`, …
- `(abcd:^)` -- **substring** of `abcd`: `a`, `cd`
- `!_` -- use **explicit spaces** for this line
- `!'` -- use **explicit punctuation** for this line
- `!1` -- use **fuzzy search** for this line, within an edit distance of 1
- `#words 4` on its own line -- lower the **phrase-length limit** to 4
- `#limit 1000` on its own line -- raise the **result limit** to 1000
- `4 5` on its own line -- **enumeration**: match 4 letters, a space, then 5 letters
- `VOWEL=[aeiou]` on its own line -- define a [**macro**](#macros) `VOWEL` to use in later lines
- `//`, `/*…*/` -- **comment**, ignore text (like in C, Javascript, etc.)
- [UI Tips](#ui-tips)

## Help

### Basics

Specify filters in the input textbox.
Each line is treated as a *noodle expression*.
You can specify multiple expressions, and only results that match *all* expressions are returned.

The query runs until one of the following:

- The input wordlist is exhausted, combining up to **10 words** into a phrase
- **300 results** are returned
- **150 seconds** have passed
- The `Stop` button is pressed

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
- `<abcd-1>` -- **transdelete** of `1` to `abcd`: rerranging *all but `N`* of the given letters
- `(abcd:-)` -- **subset** of `abcd`: contained within a *subset* of the given expression, in the same order
- `(abcd:+)` -- **superset** of `abcd`: contains the *superset* of the given expression, in the same order
- `(abcd:^)` -- **substring** of `abcd`: contained within the given expression (consecutively)

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

### Macros

Macros allow you to define a common subexpression, which can be useful when working with repeated letters from a letterbank.

Macros are defined with `NAME=expression...` syntax on their own lines.

Macros are substituted in later lines before parsing, using a naive find-replace in the order they are defined.

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

### Wordlist

The input wordlist can be downloaded [here](/wordlist).

It is based on the Debian wordlist, Wikipedia, and Wiktionary.

## Learn More

Noodle is open-source and released under the MIT license.

Visit [GitHub](https://github.com/zbanks/noodle) to fork the code or submit bugs. There is also a command-line version available for running offline/locally.

### Similar Tools

- [Nutrimatic](https://nutrimatic.org/)
    - Better at ordering results & constructing realistic phrases
- [qhex](https://tools.qhex.org/) *Word Play* tool
    - Extensive wordlist, supports "cross-filtering" for matching derivative words
- [Qat](https://www.quinapalus.com/qat.html)
    - Semantic search (e.g. "synonym to..."), complex "equation solver"

<!-- end help -->
