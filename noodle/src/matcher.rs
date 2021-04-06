use crate::bitset::{BitSet1D, BitSet3D};
use crate::expression::Expression;
use crate::parser;
use crate::words::{Char, Word};
use indexmap::{IndexMap, IndexSet};
use std::collections::HashSet;
use std::time;

/// Evaluate a query, consisting of multiple expressions, on a given wordset.
/// Returns words and phrases that match *all* of the given expressions.
pub struct Matcher<'word> {
    cache_builders: Vec<CacheBuilder<'word>>,
    caches: Vec<Cache>,
    layers: Vec<Layer<'word>>,
    // TODO: On `new`, this is populated with the input wordlist; but later it is
    // replaced with the last `nonnull_wordlist`. Is that weird?
    wordlist: Vec<&'word Word>,
    singles_done: bool,

    max_words: usize,
    results_limit: Option<usize>,

    layer_index: usize,
    results_count: usize,
}

/// A `Layer` holds the state for 1 word slot in a search for matching phrases.
#[derive(Debug)]
struct Layer<'word> {
    // The layer represents the nth word (`word_index`) in the wordlist...
    word_index: usize,
    // ...which is populated in `stem`
    stem: Option<&'word Word>,

    // `states` is a 3D bitset: [cache][fuzz][to_state]
    //
    // It contains the reachable `to_state`s for `cache` within `fuzz` edits
    // *before* consuming the given word.
    states: BitSet3D,
}

impl<'word> Layer<'word> {
    fn new(caches_count: usize, fuzz_count: usize, states_count: usize) -> Self {
        Self {
            word_index: 0,
            stem: None,
            states: BitSet3D::new((caches_count, fuzz_count), states_count),
        }
    }
}

impl<'word> Matcher<'word> {
    pub fn from_ast(query_ast: &parser::QueryAst, wordlist: &[&'word Word]) -> Self {
        // TODO: Use `options.dictionary`
        // TODO: Use `options.quiet`
        const DEFAULT_MAX_WORDS: usize = 10;
        const DEFAULT_RESULTS_LIMIT: usize = 300;
        let max_words = query_ast.options.max_words.unwrap_or(DEFAULT_MAX_WORDS);
        let expressions: Vec<_> = query_ast
            .expressions
            .iter()
            .map(|expr| Expression::from_ast(expr))
            .collect();

        let mut matcher = Self::new(expressions, wordlist, max_words);
        matcher.results_limit = query_ast
            .options
            .results_limit
            .or(Some(DEFAULT_RESULTS_LIMIT));

        matcher
    }

    pub fn expressions<'a>(&'a self) -> impl Iterator<Item = &'a Expression> {
        self.cache_builders
            .iter()
            .map(|cb| &cb.cache.expression)
            .chain(self.caches.iter().map(|c| &c.expression))
    }

    pub fn new(expressions: Vec<Expression>, wordlist: &[&'word Word], max_words: usize) -> Self {
        assert!(!expressions.is_empty());

        let word_len_max = 1 + wordlist.iter().map(|w| w.chars.len()).max().unwrap_or(0);

        // TODO: This does a copy of the wordlist slice, which would not be great
        // if we had ~millions of words.
        let wordlist = wordlist.to_vec();
        let cache_builders = expressions
            .into_iter()
            .map(|expr| CacheBuilder::new(expr, word_len_max))
            .collect();

        Matcher {
            cache_builders,
            caches: vec![],
            singles_done: false,
            layers: vec![],
            wordlist,
            max_words,
            results_limit: None,
            layer_index: 0,
            results_count: 0,
        }
    }

    /// `RUNTIME: O(expressions * (words + fuzz * states))`
    fn next_single(&mut self) -> Option<Word> {
        assert!(!self.singles_done);

        // Check for single-word matches
        let (first_cache, remaining_caches) = self.cache_builders.split_at_mut(1);
        while let Some(word) = first_cache[0].next_single_word(&self.wordlist) {
            let mut wordlist = &first_cache[0].nonnull_wordlist;
            let mut all_match = true;
            for cache in remaining_caches.iter_mut() {
                let last_word = cache.iter(wordlist).last();
                all_match = all_match && (last_word == Some(word));
                wordlist = &cache.nonnull_wordlist;
            }
            if all_match {
                return Some(word.clone());
            }
        }

        // Process the remaining words (even though they can't be single-word matches)
        let mut wl = &first_cache[0].nonnull_wordlist;
        for cache in remaining_caches.iter_mut() {
            let _ = cache.iter(wl).count();
            wl = &cache.nonnull_wordlist;
        }

        // Empty out the wordlist, if any remaining part of this function fails, then there are
        // no phrase matches possible
        self.singles_done = true;
        self.wordlist = vec![];

        // RUNTIME: O(expressions * words)
        let nonnull_wordlist = self
            .cache_builders
            .iter()
            .last()
            .unwrap()
            .nonnull_wordlist
            .clone();
        self.caches = self
            .cache_builders
            .drain(..)
            .map(|c| c.finalize(&nonnull_wordlist))
            .collect::<Result<Vec<_>, _>>()
            .ok()?;

        // RUNTIME: O(expressions * fuzz * states)
        let fuzz_max = self
            .caches
            .iter()
            .map(|cache| cache.expression.fuzz)
            .max()
            .unwrap_or(0);
        let states_max = self
            .caches
            .iter()
            .map(|cache| cache.expression.states_len())
            .max()
            .unwrap_or(1);
        let mut layers: Vec<Layer<'word>> = (0..=self.max_words)
            .map(|_| Layer::new(self.caches.len(), fuzz_max + 1, states_max))
            .collect();

        for (i, cache) in self.caches.iter().enumerate() {
            if cache.is_dfa {
                debug_assert_eq!(cache.expression.fuzz, 0);
                *layers[0].states.slice_mut((i, 0)).as_block_mut() = 1_u32;
            } else {
                layers[0].states.slice2d_mut(i).clear();

                // TODO: Expand epsilon_states(0) out to states_max
                let mut starting_states = layers[0].states.slice_mut((i, 0));
                starting_states.union_with(cache.expression.epsilon_states(0));
            }
        }

        // TODO: This wordlist copy is also potentially more expensive than nessassary,
        // but it keeps the type of `Matcher::wordlist` simple (`Vec<_>` not `Rc<RefCell<Vec<_>>>`)
        self.wordlist = nonnull_wordlist.clone();
        self.layers = layers;
        None
    }

    /// `RUNTIME: O(words^max_words * expressions * fuzz^2 * states^2)`
    fn next_phrase(&mut self) -> Option<Vec<Word>> {
        assert!(self.singles_done);
        if self.wordlist.is_empty() {
            return None;
        }

        // RUNTIME: O(words^max_words * expressions * fuzz^2 * states^2)
        let mut result = None;
        loop {
            let mut no_match = false;
            {
                let (lower_layers, upper_layers) = self.layers.split_at_mut(self.layer_index + 1);
                let this_layer = &mut lower_layers[self.layer_index];
                let next_layer = &mut upper_layers[0];

                // RUNTIME: O(expressions * fuzz^2 * states^2)
                let mut all_end_match = true;
                let mut all_no_advance = true;
                let word_index = this_layer.word_index;
                let word = self.wordlist[word_index];
                for (c, cache) in self.caches.iter().enumerate() {
                    if cache.word_classes[word_index] == 0 {
                        no_match = true;
                        break;
                    }

                    let (table, class) = cache
                        .classes
                        .get_index(cache.word_classes[word_index])
                        .unwrap();
                    let fuzz_limit = cache.expression.fuzz + 1;

                    let mut all_empty = true;
                    let mut all_subset = true;
                    let mut any_end_match = false;
                    if cache.is_dfa {
                        debug_assert_eq!(fuzz_limit, 1);
                        let si = this_layer.states.slice((c, 0)).as_block();
                        let new_state = class.set_table[si as usize];
                        *next_layer.states.slice_mut((c, 0)).as_block_mut() = new_state as u32;

                        if cache.terminal_states.contains(&new_state) {
                            any_end_match = true;
                            all_empty = false;
                            all_subset = false;
                        } else if new_state != 0 {
                            // TODO: implement all_subset (oof)
                            all_subset = false;
                            all_empty = false;
                        }
                    } else {
                        // RUNTIME: O(fuzz^2 * states^2)
                        next_layer.states.slice2d_mut(c).clear();
                        for f in 0..fuzz_limit {
                            // RUNTIME: O(fuzz * states^2)
                            for si in this_layer.states.slice((c, f)).ones() {
                                // RUNTIME: O(fuzz * states)
                                let mut fd = 0;
                                while f + fd < fuzz_limit {
                                    // RUNTIME: O(states)
                                    next_layer
                                        .states
                                        .slice_mut((c, f + fd))
                                        .union_with(table.slice((si, fd)));
                                    fd += 1;
                                }
                            }
                        }

                        // RUNTIME: O(fuzz * states)
                        let success_index = cache.expression.states_len() - 1;
                        for f in 0..fuzz_limit {
                            let es = next_layer.states.slice((c, f));
                            if es.contains(success_index) {
                                any_end_match = true;
                                all_empty = false;
                                all_subset = false;
                            } else if !es.is_empty() {
                                if !es.is_subset(&this_layer.states.slice((c, f))) {
                                    all_subset = false;
                                }
                                all_empty = false;
                            }
                        }
                    }

                    if all_empty {
                        debug_assert!(!any_end_match);
                        no_match = true;
                        break;
                    }

                    if !all_subset {
                        all_no_advance = false;
                    }
                    if !any_end_match {
                        all_end_match = false;
                    }

                    // NB: This heuristic doesn't actually help that much
                    // RUNTIME: O(fuzz * states)
                    //next_layer.states.compact_distance_set(c);
                }

                // Unclear if this optimization is worth it (even though it does help prevent .* blowouts)
                if all_no_advance {
                    no_match = true;
                }

                if !no_match {
                    this_layer.stem = Some(word);
                    if all_end_match && self.layer_index >= 1 {
                        result = Some(
                            self.layers[0..=self.layer_index]
                                .iter()
                                .map(|layer| layer.stem.unwrap().clone())
                                .collect(),
                        );
                    }
                }
            }

            if self.advance(!no_match) || result.is_some() {
                break;
            }
        }

        result
    }

    /// Advance the phrase iterator. Return `true` if the iterator is exhausted.
    ///
    /// If there was a `partial_match`, then attempt to "descend" by incrementing
    /// `self.layer_index` to build a longer phrase.
    /// If the was not a partial match, then try the next word in the wordlist.
    /// But, if there are no words left in the wordlist, "ascend" by decrementing
    /// `self.layer_index` to try a shorter phrase.
    ///
    /// `RUNTIME: O(max_words)`
    fn advance(&mut self, mut partial_match: bool) -> bool {
        loop {
            // If there's a partial_match, try to build a longer phrase
            if partial_match {
                // If we've hit the `max_words` limit, too bad.
                // Treat it like we didn't have a partial match
                if self.layer_index + 1 >= self.max_words {
                    partial_match = false;
                    continue;
                }

                // Descend to the next layer (resetting the layer's word_index to 0)
                self.layer_index += 1;
                self.layers[self.layer_index].word_index = 0;
                break;
            } else {
                // This word didn't create a match, so try the next word
                self.layers[self.layer_index].word_index += 1;

                // Did we exhaust the whole word list at this layer?
                if self.layers[self.layer_index].word_index >= self.wordlist.len() {
                    // If there isn't a previous layer, then we're done!
                    if self.layer_index == 0 {
                        println!(
                            "Matcher done with {} result(s) (up to {} word phrases)",
                            self.results_count, self.max_words
                        );

                        // Signal that the iterator is exhausted
                        return true;
                    }

                    // Ascend back to the previous layer (perhaps recursively!)
                    self.layer_index -= 1;
                    continue;
                }
                break;
            }
        }

        // The iterator is not exhausted
        false
    }
}

impl Iterator for Matcher<'_> {
    type Item = Vec<Word>;

    fn next(&mut self) -> Option<Vec<Word>> {
        let r = if self
            .results_limit
            .map(|lim| lim < self.results_count)
            .unwrap_or(false)
        {
            None
        } else if !self.singles_done {
            self.next_single()
                .map(|w| vec![w])
                .or_else(|| self.next_phrase())
        } else {
            self.next_phrase()
        };
        if r.is_some() {
            self.results_count += 1;
        }
        r
    }
}

/// During the initial search, each word in the wordlist is fully characterized
/// in relation to an `Expression`.
///
/// The words can be sorted into "equivalency classes" based on their net transitions
/// on the expression's NFA: for each starting state, which states are reachable after
/// consuming the entire word, within a certain edit distance?
///
/// Often, we can eliminate words which have *no* reachable states within the given
/// edit limit (fuzz), regardless of starting state.
/// The reachable words are referred to as the "`nonnull_wordlist`"
#[derive(Debug)]
struct Cache {
    expression: Expression,
    // The keys in `classes` are 3D bitsets on: [from_state][fuzz][to_state]
    //
    // These represent "equivalency classes": words which have equivalent behavior
    // on the `expression` NFA.
    classes: IndexMap<BitSet3D, CacheClass>,

    // This is a parallel vector to `wordlist`: for each word in the wordlist, which
    // class does it belong to (by index)?
    word_classes: Vec<usize>,

    terminal_states: HashSet<usize>,
    is_dfa: bool,
}

#[derive(Debug, Clone)]
struct CacheClass {
    // TODO: In future optimizations, it may be useful to store other information here
    n_words: usize,
    first_word: Option<String>,
    set_table: Vec<usize>,
}

/// This struct is used while doing the first pass over wordlist, looking for single-word
/// matches and populating the equivalency class cache.
///
/// After scanning `wordlist` once, this struct is consumed by `finalize(...)` and turned
/// into a bare `Cache`.
struct CacheBuilder<'word> {
    cache: Cache,

    // We want an immutable reference to this, but with it allowed
    // to change out underneath us
    // This is owned by us and populated by us, others take a reference to it
    nonnull_wordlist: Vec<&'word Word>,

    // `transition_table` is a 4D bitset: [char_index][from_state][fuzz][to_state]
    //
    // After being populated by `Expression::fill_transition_table(...)`, it contains elements such
    // that starting at `from_state` and consuming characters `0..=char_index` can reach `to_state`
    // in exactly `fuzz` edit distance.
    //
    // Entries in `char_index` have undefined state when `char_index` >= `previous_chars.len()`
    transition_table: Vec<BitSet3D>,
    previous_chars: &'word [Char],

    word_index: usize,
    single_fully_drained: bool,

    // Statistics for debugging
    total_prefixed: usize,
    total_matched: usize,
    total_length: usize,
}

impl<'word> CacheBuilder<'word> {
    fn new(expression: Expression, word_len_max: usize) -> Self {
        let mut classes = IndexMap::new();
        let word_classes = vec![];

        let transition_group_new = || {
            BitSet3D::new(
                (expression.states_len(), expression.fuzz + 1),
                expression.states_len(),
            )
        };

        // The first class (0) is always the "null" class for words which do not match
        classes.insert_full(
            transition_group_new(),
            CacheClass {
                n_words: 0,
                first_word: None,
                set_table: vec![],
            },
        );

        let transition_table = vec![transition_group_new(); word_len_max];

        Self {
            cache: Cache {
                expression,
                classes,
                word_classes,
                is_dfa: false,
                terminal_states: HashSet::new(),
            },

            word_index: 0,
            nonnull_wordlist: vec![],
            transition_table,
            previous_chars: &[],

            total_prefixed: 0,
            total_matched: 0,
            total_length: 0,
            single_fully_drained: false,
        }
    }

    /// `RUNTIME: O(words * chars * fuzz * states^3)`
    fn next_single_word(&mut self, wordlist: &[&'word Word]) -> Option<&'word Word> {
        // RUNTIME: O(words * chars * fuzz * states^3)
        let fuzz_range = 0..self.cache.expression.fuzz + 1;
        while self.word_index < wordlist.len() {
            let word = &wordlist[self.word_index];
            self.word_index += 1;

            // RUNTIME: O(chars)
            let word_len = word.chars.len();
            let mut si: usize = 0;
            while si < word_len
                && si < self.previous_chars.len()
                && word.chars[si] == self.previous_chars[si]
            {
                si += 1;
            }

            // RUNTIME: O(fuzz * states^2)
            let prefixed_table = &mut self.transition_table[si..];
            if si == 0 {
                // Clear table
                prefixed_table[0].borrow_mut().clear();
                for i in 0..self.cache.expression.states_len() {
                    prefixed_table[0]
                        .slice_mut((i, 0))
                        .union_with(self.cache.expression.epsilon_states(i));
                }
            }

            // RUNTIME: O(chars * fuzz * states^3)
            let valid_len = self
                .cache
                .expression
                .fill_transition_table(&word.chars[si..], prefixed_table)
                + si;

            self.total_prefixed += si;
            self.total_matched += valid_len;
            self.total_length += word_len;

            self.previous_chars = &word.chars[0..valid_len];
            if valid_len < word_len {
                continue;
            }
            assert!(valid_len == word_len);

            // RUNTIME: O(fuzz * states^2)
            let entry = self
                .cache
                .classes
                .entry(self.transition_table[word_len].clone());
            //.and_modify(|v| v.n_words += 1);
            if entry.index() != 0 {
                self.nonnull_wordlist.push(word);
                self.cache.word_classes.push(entry.index());
            }
            entry.or_insert(CacheClass {
                n_words: 0,
                first_word: None,
                set_table: vec![],
            });

            // RUNTIME: O(fuzz)
            for f in fuzz_range.clone() {
                let start_transitions = self.transition_table[word_len].slice((0, f));
                if start_transitions.contains(self.cache.expression.states_len() - 1) {
                    return Some(word);
                }
            }
        }

        self.single_fully_drained = true;
        None
    }

    /// This must be called after consuming the entire iterator
    /// `new_wordlist` must be a subset of the original wordlist (in the exact same order)
    ///
    /// `RUNTIME: O(words)`
    fn finalize(mut self, new_wordlist: &[&'word Word]) -> Result<Cache, ()> {
        assert!(self.single_fully_drained);
        // TODO: There should be an API for stats
        //println!(
        //    "prefixed={}, matched={}, elided={}",
        //    self.total_prefixed,
        //    self.total_matched - self.total_prefixed,
        //    self.total_length - self.total_matched
        //);
        println!(
            "{} distinct classes with {} words",
            self.cache.classes.len(),
            self.nonnull_wordlist.len(),
        );

        println!("finalizing: {:?}", self.cache.expression);

        let mut new_word_classes = vec![];
        let mut i: usize = 0;

        // RUNTIME: O(words)
        for (&word, &class) in self
            .nonnull_wordlist
            .iter()
            .zip(self.cache.word_classes.iter())
        {
            if i < new_wordlist.len() && *word == *new_wordlist[i] {
                new_word_classes.push(class);
                self.cache.classes[class].n_words += 1;
                self.cache.classes[class]
                    .first_word
                    .get_or_insert_with(|| word.text.clone());
                i += 1;
            }
        }
        assert!(i == new_wordlist.len());
        self.cache.word_classes = new_word_classes;
        assert!(self.cache.word_classes.len() == new_wordlist.len());

        //for (i, (table, class)) in self.cache.classes.iter().enumerate() {
        //    println!(
        //        "class {}, n_words={}, ex={:?}: {:?}",
        //        i, class.n_words, class.first_word, table
        //    );
        //}

        let states_len = self.cache.expression.states_len();
        /*
        let start = time::Instant::now();
        let mut duplicate_states: Vec<Option<usize>> = vec![None; states_len];
        for i in 0..states_len {
            'outer: for j in i + 1..states_len {
                if duplicate_states[j].is_some() {
                    continue;
                }
                let mut inputs_same = true;
                let mut outputs_same = true;
                for f in 0..=self.cache.expression.fuzz {
                    for (transition_table, class) in self.cache.classes.iter() {
                        if class.n_words == 0 {
                            continue;
                        }
                        if transition_table.slice((i, f)) != transition_table.slice((j, f)) {
                            outputs_same = false;
                        }
                        for k in 0..states_len {
                            let slice = transition_table.slice((k, f));
                            if slice.contains(i) != slice.contains(j) {
                                inputs_same = false;
                                break;
                            }
                        }
                        if !inputs_same && !outputs_same {
                            continue 'outer;
                        }
                    }
                }
                if inputs_same {
                    duplicate_states[j] = Some(i);
                }
            }
        }
        println!(
            "checked for identical states in {:?}: {:?}",
            start.elapsed(),
            duplicate_states
        );

        let start = time::Instant::now();
        // TODO: Check that the last state (success) is still the last state
        let new_states_len = duplicate_states.iter().filter(|x| x.is_none()).count();
        if new_states_len != states_len {
            let new_state_index: Vec<_> = duplicate_states
                .iter()
                .enumerate()
                .map(|(i, x)| {
                    if x.is_some() {
                        None
                    } else {
                        Some(duplicate_states[..i].iter().filter(|y| y.is_none()).count())
                    }
                })
                .collect();

            let transition_group_new = || {
                BitSet3D::new(
                    (new_states_len, self.cache.expression.fuzz + 1),
                    new_states_len,
                )
            };
            let mut new_classes = IndexMap::new();
            new_classes.insert_full(
                transition_group_new(),
                CacheClass {
                    n_words: 0,
                    first_word: None,
                    set_table: vec![],
                },
            );

            for (index, (transition_table, class)) in self.cache.classes.iter().enumerate() {
                if class.n_words == 0 {
                    continue;
                }
                let mut new_table = transition_group_new();
                for (i, &i_new) in new_state_index.iter().enumerate() {
                    let i_new = i_new
                        .unwrap_or_else(|| new_state_index[duplicate_states[i].unwrap()].unwrap());
                    for f in 0..=self.cache.expression.fuzz {
                        let old_slice = transition_table.slice((i, f));
                        let mut new_slice = new_table.slice_mut((i_new, f));
                        for j in old_slice.ones() {
                            if let Some(new_j) = new_state_index[j] {
                                new_slice.insert(new_j);
                            }
                        }
                    }
                }

                new_classes
                    .entry(new_table)
                    .and_modify(|c| c.n_words += class.n_words)
                    .or_insert(class.clone());
            }
            for (i, (table, class)) in new_classes.iter().enumerate() {
                println!(
                    "new_class {} with {} words first {:?}: {:?}",
                    i, class.n_words, class.first_word, table
                );
            }

            let mut new_state_powerset = IndexSet::new();
            for table in new_classes.keys() {
                for i in 0..new_states_len {
                    for f in 0..=self.cache.expression.fuzz {
                        new_state_powerset.insert(table.slice((i, f)));
                    }
                }
            }
            println!("new powerset size={}", new_state_powerset.len());

            // TODO: remap word
            println!("remapped states in {:?}", start.elapsed());
        }

        let mut state_powerset = IndexSet::new();
        for table in self.cache.classes.keys() {
            for i in 0..new_states_len {
                for f in 0..=self.cache.expression.fuzz {
                    state_powerset.insert(table.slice((i, f)));
                }
            }
        }
        println!("old powerset size={}", state_powerset.len());
        */

        let copy_bitset = |bref| {
            let mut bitset = BitSet1D::new((), states_len);
            bitset.borrow_mut().copy_from(bref);
            bitset
        };

        let start = time::Instant::now();
        let mut setmap = IndexSet::new();
        // The "null" state
        setmap.insert(copy_bitset(
            self.cache.classes.get_index(0).unwrap().0.slice((0, 0)),
        ));
        // The starting state
        setmap.insert(copy_bitset(self.cache.expression.epsilon_states(0)));

        self.cache.is_dfa = self.cache.expression.fuzz == 0;
        if self.cache.is_dfa {
            for (_table, mut class) in self.cache.classes.iter_mut() {
                if class.n_words != 0 {
                    class.set_table = vec![0];
                }
            }

            let mut s = 1;
            while s < setmap.len() {
                let from_set = setmap.get_index(s).unwrap().clone();
                for (table, class) in self.cache.classes.iter_mut() {
                    if class.n_words == 0 {
                        continue;
                    }
                    for f in 0..=self.cache.expression.fuzz {
                        let mut to_set = BitSet1D::new((), states_len);
                        for i in from_set.borrow().ones() {
                            to_set.borrow_mut().union_with(table.slice((i, f)));
                        }
                        let (index, _new) = setmap.insert_full(to_set);
                        debug_assert_eq!(class.set_table.len(), s);
                        class.set_table.push(index);
                    }
                }
                if setmap.len() > 1024 {
                    self.cache.is_dfa = false;
                    break;
                }
                s += 1;
            }
        }

        if self.cache.is_dfa {
            for (index, set) in setmap.iter().enumerate() {
                if set.borrow().contains(states_len - 1) {
                    self.cache.terminal_states.insert(index);
                }
            }
            if self.cache.terminal_states.is_empty() {
                println!("-- Terminal states are unreachable; there are not matches! --");
                return Err(());
            }

            let dt = start.elapsed();
            println!(
                "Converted NFA with {} states to DFA with {} states in {:?}",
                states_len,
                setmap.len(),
                dt
            );
            //for (index, set) in setmap.iter().enumerate() {
            //    println!("{}  {:?}", index, set);
            //}
            //println!("terminal states: {:?}", self.cache.terminal_states);
            //println!("dfa time = {:?}", dt);
        }

        Ok(self.cache)
    }

    fn iter<'it>(&'it mut self, wordlist: &'it [&'word Word]) -> CacheBuilderIter<'word, 'it> {
        CacheBuilderIter {
            cache_builder: self,
            wordlist,
        }
    }
}

/// Wrapper for iterating over the single-word results from a CacheBuilder,
/// which requires providing the `wordlist` slice
struct CacheBuilderIter<'word, 'it> {
    cache_builder: &'it mut CacheBuilder<'word>,
    wordlist: &'it [&'word Word],
}

impl<'word> Iterator for CacheBuilderIter<'word, '_> {
    type Item = &'word Word;

    fn next(&mut self) -> Option<&'word Word> {
        self.cache_builder.next_single_word(self.wordlist)
    }
}
