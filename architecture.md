Noodle Architecture
===================

Noodle is a tool for searching for words and phrases that match complex constraints.

"Noodle expressions" are similar to regular expressions, but have extensions for common wordplay operations (like anagramming) and approximate matches (fuzzy matching).

**Note:** This document is still a work-in-progress; and is not complete. 

Prior Art
---------

### tools.qhex.org *Word Play*

The *Word Play* utility on [tools.qhex.org](https://tools.qhex.org/) is one of my personal favorite puzzle tools (even though it is not as full-featured as other tools).

The tool is [open source](https://github.com/danyq/tools.qhex.org/blob/master/wordplay.py) as a command-line tool, but the wordlist & web UI are not.

<!--
#### Wordlist

The *Word Play* website uses a fairly large word & phrase list.
Its sources are not listed, but it appears to include at least a dictionary source, Wikipedia titles, and Urban Dictionary entries.

All words have been normalized to lowercase `a-z`, with spaces and accents removed.

Words are roughly organized so that more common words/phrases are earlier in the list.

#### Capabilities & UI

*Word Play* takes in a list of constraints, one per line, and then incrementally returns up to 500 results that match all of the constraints.

Each constraint can either be a regular expression, or an "operation" such as `anagram` or `transadd 3`.

There is also a "cross-filtering" option, where you can *insert* matches from an initial set of constraints *into* a second set of constraints.This can be used to solve some puzzles that require finding a *pair* of related words, but the capability is somewhat limited because the first match is inserted verbatim.

The left side of the page contains examples and usage information.
The page keeps track of history, and maintains a dropdown of past queries.
The same page layout & input textbox are used across multiple tools on the site.

#### Implementation

*Word Play* is implemented in straightforward Python.
Each input constraint is translated into a set of regular expressions, and then each word in the wordlist is evaluated for matches.

This strategy is simple, and is more than fast enough for human use. Although the strategy is algorithmically straightforward, it remains performant by only evaluating single-entry matches from the wordlist.

As a CLI tool, it is very easy to understand, modify, and extend.
However, without the web interface & wordlist are not readily available, which makes it harder to re-deploy or share with others.
-->

### Nutrimatic

[*Nutrimatic*](https://nutrimatic.org/) is a search tool based on Wikipedia n-grams.

The tool is fully [open source](https://github.com/egnor/nutrimatic) under GPLv3.


### Qat

[*Qat*](https://www.quinapalus.com/qat.html) is a very powerful constraint/search tool. 

The tool is free to use online, supports both English & French, but is not open-source.

Qat's author also maintains [*Qxw*](https://www.quinapalus.com/qxw.html), an very good open-source (GPL) tool for constructing crosswords.
In theory this tool could also be useful for *solving* puzzles, but I've never used it that way.


### Other tools

There are also other, simpler, tools that are often used when solving wordplay puzzles.
However, they are less directly comparable to *Noodle*.

- [Anagram Server](https://wordsmith.org/anagram/)
- [Regex Dictionary Search](https://www.visca.com/regexdict/)
- [NPL Dictionary Search](http://wiki.puzzlers.org/dokuwiki/doku.php?id=solving:wordlists:dictionary_search)
- [Crossword Solver](https://www.wordplays.com/crossword-solver/) (Crossword clue search)

High-Level Overview
-------------------

### NFAs

A key part of Noodle's architecture are [Nondeterministic Finite Automata](https://en.wikipedia.org/wiki/Nondeterministic_finite_automaton) (*NFA*s).
These are a type of state machine that are commonly used to implement [regular expressions](https://en.wikipedia.org/wiki/Regular_expression). 

Unlike simpler [*Deterministic* Fniite Automata](https://en.wikipedia.org/wiki/Deterministic_finite_automaton) (*DFA*s), each state can have *multiple* transitions for the same input.
This means that evaluating an NFA requires working with *sets of reachable states*, whereas evaluating a DFA only requires working with a single state at a time.

All DFAs are technically NFAs. Conversely, [you can convert](https://en.wikipedia.org/wiki/Powerset_construction) an NFA with `n` states into a DFA with `O(2^n)` states. 

// For a given number of states, NFAs are more complex than DFAs, but are more closely related 
[Thompson's construction](https://en.wikipedia.org/wiki/Thompson%27s_construction) is a straightforward algorithm to build an NFA corresponding to a regular expression.

Internally, Noodle uses an [NFA with ε-moves](https://en.wikipedia.org/wiki/Nondeterministic_finite_automaton#NFA_with_ε-moves) representation.
Each NFA has a set of initial states, a success state, and intermediate states. Transitions between states can either match and consume exactly 1 character from the input, or be an ε-move which can always be taken and consumes no input. Unlike [DFAs](https://en.wikipedia.org/wiki/Deterministic_finite_automaton), evaluating an NFA requires tracking the *set* of reachable states from a given input string. A input string is considered "matching" if there is *at least one* path from any initial state to the success state.

NFAs are more complex and, in general, slower to evaluate than DFAs. Many regular expression libraries try to convert NFA representations to DFAs to improve evaluation speed.

<!-- describe fuzz; expression::Expression -->

### 

*Noodle*'s goal is to find words and phrases that match a *Noodle Expression* query.

The query is parsed, and each expression is translated into one or more NFAs, as described above. 
"Simple" expressions that are analogous to regular expressions can be represented as a single NFA. More complex constraints, like anagram segments (`<...>`), are represented by multiple NFAs.

Finding *single words* that exactly match the query is straightforward, and Noodle acts like a typical regular expression engine.
Each string in the input wordlist is run through the query's NFAs, and it's emitted as a match if the string matches *all* the NFAs.

<!-- This logic is also extended to handle *fuzzy matches*... -->

When Noodle searches for *phrase* matches, it takes advantage of the NFA internals to make the search more efficient.
A naïve search for a `k`-word phrase from a wordlist with `n` words could be done by checking all `n^k` sequences of words



<!--
*Noodle* finds words and phrases that match a set of *Noodle Expressions*.

Internally, each expression is conceptually translated into one or more [regular expressions](https://en.wikipedia.org/wiki/Regular_expression). Each of these expressions are compiled into a [Nondeterministic Finite Automaton (*NFA*) with ε-moves](https://en.wikipedia.org/wiki/Nondeterministic_finite_automaton#NFA_with_ε-moves).

These NFAs have an initial state, a success state, and intermediate states. Transitions between states can either match and consume exactly 1 character from the input, or be an ε-move which can always be taken and consumes no input. Unlike [DFAs](https://en.wikipedia.org/wiki/Deterministic_finite_automaton), evaluating an NFA requires tracking the *set* of reachable states from a given input string. A input string is considered "matching" if there is *at least one* path from the initial state to the success state.

Finding *single words* that match the query is straightforward: after compiling the query into a set of NFAs, each word in the wordlist is evaluated against each of these NFAs: if it matches all of them, it is emitted as a match.
-->




Process
-------

- User input: High-level *Query*
    - Queries provide syntactic sugar, and are easy to compile down into a set of *Expressions*
    - 1 Query with an anagram constraint would be turned into multiple Expressions
- Regex-like Low-level *Expressions*
    - Based on POSIX Extended Regular Expressions, but does not support backreferences
    - Adds support fuzzy matching (Levenshtein edit distance)
    - Represented internally as an NFA
        - Each state in the NFA has (up to) two types of transitions:
            - Epsilon transitions (that do not consume a character), to a *set* of next states
            - Character transitions (that match & consume 1 character), with a *set* of characters that can transition to *one* next state
        - The transitive closure over epsilon transitions is pre-computed.
        - This form guarantees that each input character can be consumed in `O(1)` time (follow the character transition, followed by epsilon transitions)
        - Fuzzy matches are implemented by tracking set of states reachable within a given number of edits
            - Since we are tracking state sets for fuzzy matching, there's less benefit from transforming the NFA into a DFA
- Wordlist
    - Reduced alphabet: only considers letters A-Z (case-insensitive), spaces, and punctuation
        - Any non-letter, non-space character is translated into "punctuation"
- Compute a "transition table" for each word in the wordlist, for each Expressions in the Query
    - For each `(Expression, word)` compute the transition table `[from_state, to_state] -> min_edit_distance`
        - If `min_edit_distance` is more than the allowed fuzz, use `infty`
        - This transition table is currently represented as an array of bitsets: `[from_state][edit_distance][to_state] -> true/false`
    - If the transition table is all `infty`, the word isn't useful and can be ignored for the rest of the query processing
    - This step is roughly `O(n_words * word_len * n_states^3 * (max_fuzz + 1) * O(bitset))` for each Expression
        - `O(bitset)` is `O(1)` if `n_states` is compile-time assumed to be small, e.g. `<= 64`. Otherwise, it is `O(n_states)`
        - `n_words` goes down for each successive Expression processed as words get pruned
        - Computation can be reused between words with shared stems
        - In pratice, `max_fuzz` has a super-linear effect on the runtime, because it ~quadratically increases the number of reachable states.
- Use the transition table to follow an [iterative-deepening depth-first search (IDDFS)](https://en.wikipedia.org/wiki/Iterative_deepening_depth-first_search), up to a given maximum depth (maximum number of words)
    - Find a series of words which can connect the initial state to the success state (within the maximum edit distance) across every Expression
    - Do not include words that do not advance the state of any Expression
    - This step is roughly `O((n_words * n_expressions * (max_fuzz + 1) * n_states * O(bitset)) ^ max_words)`

