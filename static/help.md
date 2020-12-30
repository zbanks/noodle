<!-- generated from `pandoc help.md` -->

Specify filters in the input textbox. Each line is treated as a *noodle expression*.

The query runs until the input wordlist is exhausted, 300 results are returned, or 15 seconds have passed.

<!-- If `N` is provided, perform a fuzzy match with up to `N` edits. (NB: consecutive inserts are not allowed) -->

Noodle supports the following regex syntax: `[...]`, `[^...]`, `.`, `*`, `+`, `?`, `(...)`, `|`, `{...}`.

Before matching, words are converted to lowercase and stripped of whitespace and non-alphabetical symbols (punctuation, numbers).

To explicitly match spaces, include "`!_`" at the end of the expression. When enabled, the matched phrase is surrounded by spaces, and space characters can be explicitly matched with the "`_`" character.

To explicitly match other symbols, include "`!'`" at the end of the expression. When enabled, these symbols can be matched with the "`'`" character.

Regardless of setting, and unlike normal regular expressions, the period ("`.`") is only equivalent to "`[a-z]`". To match *any* symbol, use "`[a-z'_]`".

Noodle has additional support for anagram-like constriants with angle bracket syntax: `<...>`

- `<abcd>` -- **anagram** of `abcd`: rearranging the given letters
- `<abcd+>` -- **superanagram** of `abcd`: rearranging *at least* the given letters
- `<abcd+3>` -- **transadd** of `3` to `abcd`: rearranging *all* of the given letters *plus* `N` wildcards
- `<abcd->` -- **subanagram** of `abcd`: rearranging *at most* the given letters
- `<abcd-1>` -- **transdelete** of `3` to `abcd`: rerranging *all but `N`* of the given letters
- `(abcd:?)` -- **partial** of `abcd`: contained within a subset of the given letters, in the same order

<!--
TODO:
- `<abcd:~>` -- **bank**
- `<abcd:+>` -- **superbank**
- `<abcd:->` -- **subbank**
- `(abcd:+2)` -- **add**
- `(abcd:-2)` -- **delete**
- `(abcd:~2)` -- **change**
- `(abcd:~)` -- **substring**
-->
<!-- end help -->
