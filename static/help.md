Specify filters in the input textbox. Each line is treated as a filter.

The filters are "chained" together, in order. The output pane shows which words matched the entire filter chain.

The query runs until the input wordlist is exhausted, 300 results are returned, or 15 seconds have past.

#### `nx [N]: <noodlex>`

Evalute noodle expression

If `N` is provided, perform a fuzzy match with up to `N` edits. (NB: consecutive inserts are not allowed)

Supported regex features: `[...]`, `[^...]`, `.`, `*`, `+`, `?`, `(...)`, `|`.

Matches are case-insentive.
Spaces are ignored both in the expression, and the matched word.
An `_` in the expression matches an space character in a word.

#### `nxn N: <noodlex>`

Evalute noodle expression, matching up to `N` words.

The `noodlex` expression follows the same syntax as the `nx` command, except multi-word search cannot be combined with fuzzy search.

**Note:** This is handled as a special case for now, and cannot be chained with any other filters.

#### `regex: <regex>`

Evaluate regular expression.
Matches are case-insentive.
The whole word must match, it is implicitly wrapped in  `^...$`.

#### `extract: <regex>`

Evalulate regular expression, but return the first capture group (it must also be a word)

#### `extractq: <regex>`

Evalulate regular expression, but return the first capture group (it does not have to be a word)

#### `anagram: <letters>`

Anagram

#### `subanagram: <letters>`

Contains at most the given letters

#### `superanagram: <letters>`

Contains at least the given letters

#### `transadd N: <letters>`

Contains the given letters, plus `N` extras

#### `transdelete N: <letters>`

Contains the given letters, except `N` of them

#### `bank: <letters>`

Only contains the given letters (but with repeats)
