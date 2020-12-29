Specify filters in the input textbox. Each line is treated as a *noodle expression*.

The query runs until the input wordlist is exhausted, 300 results are returned, or 15 seconds have passed.

<!-- If `N` is provided, perform a fuzzy match with up to `N` edits. (NB: consecutive inserts are not allowed) -->

Noodle supports the following regex syntax: `[...]`, `[^...]`, `.`, `*`, `+`, `?`, `(...)`, `|`, `{...}`.

Noodle has additional support for anagram-like constriants with angle bracket syntax: `<...>`

- `<abcd>` -- **anagram** of `abcd`: rearranging the given letters
- `<abcd+>` -- **superanagram** of `abcd`: rearranging *at least* the given letters
- `<abcd+3>` -- **transadd** of `3` to `abcd`: rearranging *all* of the given letters *plus* `N` wildcards
- `<abcd->` -- **subanagram** of `abcd`: rearranging *at most* the given letters
- `<abcd-1>` -- **transdelete** of `3` to `abcd`: rerranging *all but `N`* of the given letters

<!--
TODO:
- `<abcd:~>` -- **bank**
- `<abcd:+>` -- **superbank**
- `<abcd:->` -- **subbank**
- `(abcd:?)` -- **substring**
- `(abcd:+2)` -- **add**
- `(abcd:-2)` -- **delete**
- `(abcd:~2)` -- **change**
-->

Matches are case-insentive.

Spaces are completely ignored in the expression. By default, spaces are ignored in the matched word or phrase.

An "`_`" in the expression matches an space character in a word, and enables explicit space matching across the entire expression.

An "`-`" in the expression matches any symbol in the word (hyphen, apostrophe, etc.).
