Noodle Architecture
===================

Noodle is a tool for searching for words and phrases that match complex constraints.

Constraints are based on regular expressions, but have extensions for common wordplay operations (like anagramming) and approximate matches (fuzzy matching).

**Note:** This document is still a work-in-progress; and is not complete. 

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

