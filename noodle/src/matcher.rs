use crate::bitset::{BitSet1D, BitSet3D};
use crate::expression::Expression;
use crate::parser;
use crate::words::{Char, Word};
use indexmap::IndexMap;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum MatcherResponse {
    Timeout,
    Logs(Vec<String>),
    Match(Vec<Word>),
}

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

    pub fn expressions(&self) -> impl Iterator<Item = &Expression> {
        self.cache_builders
            .iter()
            .map(|cb| &cb.cache.expression)
            .chain(self.caches.iter().map(|c| &c.expression))
    }

    pub fn progress(&self) -> String {
        if self.layers.is_empty() {
            format!("stage 1: {}", self.cache_builders[0].progress())
        } else {
            format!(
                "stage 2: = {}/{}",
                self.layers[0].word_index,
                self.wordlist.len()
            )
        }
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
    fn next_single(&mut self, deadline: Option<Instant>) -> Option<MatcherResponse> {
        assert!(!self.singles_done);

        // Check for single-word matches
        let (first_cache, remaining_caches) = self.cache_builders.split_at_mut(1);
        while let Some(word) = first_cache[0].next_single_word(&self.wordlist, deadline) {
            let mut wordlist = &first_cache[0].nonnull_wordlist;
            let mut all_match = true;
            for cache in remaining_caches.iter_mut() {
                let last_word = cache.iter(wordlist).last();
                all_match = all_match && (last_word == Some(word));
                wordlist = &cache.nonnull_wordlist;
            }
            if all_match {
                return Some(MatcherResponse::Match(vec![word.clone()]));
            }
        }
        if deadline.is_some() && Some(Instant::now()) > deadline {
            return Some(MatcherResponse::Timeout);
        }
        self.singles_done = true;
        self.wordlist = vec![];

        // Process the remaining words (even though they can't be single-word matches)
        let mut wl = &first_cache[0].nonnull_wordlist;
        for cache in remaining_caches.iter_mut() {
            let _ = cache.iter(wl).count();
            wl = &cache.nonnull_wordlist;
        }

        // RUNTIME: ???
        let mut nonnull_wordlist = self
            .cache_builders
            .iter()
            .last()
            .unwrap()
            .nonnull_wordlist
            .clone();

        // TODO: Refactor this, the code is pretty bad
        // May be able to be more clever about iterations to avoid an ~n^2 scenario?
        let start = Instant::now();
        loop {
            let mut optimized = false;

            for cache_builder in self.cache_builders.iter_mut().rev() {
                let opt = cache_builder.pre_finalize(&nonnull_wordlist);
                optimized = optimized || opt;

                if cache_builder.nonnull_wordlist.len() != nonnull_wordlist.len() {
                    println!(
                        "using {} to reduce list from {} -> {}",
                        cache_builder.cache.expression.text,
                        nonnull_wordlist.len(),
                        cache_builder.nonnull_wordlist.len()
                    );

                    assert!(cache_builder.nonnull_wordlist.len() < nonnull_wordlist.len());
                    nonnull_wordlist = cache_builder.nonnull_wordlist.clone();
                    if nonnull_wordlist.is_empty() {
                        println!("Empty wordlist!");
                        return None;
                    }
                }
            }

            if !optimized {
                break;
            }
        }
        println!("refinement took {:?}", start.elapsed());

        self.caches = self.cache_builders.drain(..).map(|c| c.cache).collect();

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
            .map(|cache| cache.states_len)
            .max()
            .unwrap_or(1);
        let mut layers: Vec<Layer<'word>> = (0..=self.max_words)
            .map(|_| Layer::new(self.caches.len(), fuzz_max + 1, states_max))
            .collect();

        for (i, cache) in self.caches.iter().enumerate() {
            layers[0].states.slice2d_mut(i).clear();

            // TODO: Expand epsilon_states(0) out to states_max
            let mut starting_states = layers[0].states.slice_mut((i, 0));
            starting_states.union_with(cache.start_states.borrow());
        }

        // TODO: This wordlist copy is also potentially more expensive than nessassary,
        // but it keeps the type of `Matcher::wordlist` simple (`Vec<_>` not `Rc<RefCell<Vec<_>>>`)
        self.wordlist = nonnull_wordlist.clone();
        self.layers = layers;
        None
    }

    /// `RUNTIME: O(words * expressions * fuzz^2 * states^2)??`
    fn next_phrase_bfs(&mut self, deadline: Option<Instant>) -> Option<MatcherResponse> {
        None
    }

    /// `RUNTIME: O(words^max_words * expressions * fuzz^2 * states^2)`
    fn next_phrase(&mut self, deadline: Option<Instant>) -> Option<MatcherResponse> {
        assert!(self.singles_done);
        if self.wordlist.is_empty() {
            return None;
        }

        // RUNTIME: O(words^max_words * expressions * fuzz^2 * states^2)
        let mut check_count = 0;
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

                    let class = cache
                        .classes
                        .get_index(cache.word_classes[word_index])
                        .unwrap()
                        .0;
                    next_layer.states.slice2d_mut(c).clear();

                    // RUNTIME: O(fuzz^2 * states^2)
                    let fuzz_limit = cache.expression.fuzz + 1;
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
                                    .union_with(class.slice((si, fd)));
                                fd += 1;
                            }
                        }
                    }

                    // RUNTIME: O(fuzz * states)
                    let mut all_empty = true;
                    let mut all_subset = true;
                    let mut any_end_match = false;
                    let success_index = cache.states_len - 1;
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
                        result = Some(MatcherResponse::Match(
                            self.layers[0..=self.layer_index]
                                .iter()
                                .map(|layer| layer.stem.unwrap().clone())
                                .collect(),
                        ));
                    }
                }
            }

            if self.advance(!no_match) || result.is_some() {
                break;
            }
            check_count += 1;
            if check_count % 256 == 0 && deadline.is_some() && Some(Instant::now()) > deadline {
                return Some(MatcherResponse::Timeout);
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

    pub fn next_with_deadline(&mut self, deadline: Option<Instant>) -> Option<MatcherResponse> {
        let r = if self
            .results_limit
            .map(|lim| lim < self.results_count)
            .unwrap_or(false)
        {
            None
        } else if !self.singles_done {
            self.next_single(deadline).or_else(|| {
                Some(MatcherResponse::Logs(vec![
                    "Done with single matches".to_string()
                ]))
            })
        } else {
            self.next_phrase(deadline)
        };
        if let Some(MatcherResponse::Match(_)) = r {
            self.results_count += 1;
        }
        r
    }
}

impl Iterator for Matcher<'_> {
    type Item = MatcherResponse;

    fn next(&mut self) -> Option<MatcherResponse> {
        self.next_with_deadline(None)
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
    states_len: usize,
    start_states: BitSet1D,

    // The keys in `classes` are 3D bitsets on: [from_state][fuzz][to_state]
    //
    // These represent "equivalency classes": words which have equivalent behavior
    // on the `expression` NFA.
    classes: IndexMap<BitSet3D, CacheClass>,

    // This is a parallel vector to `wordlist`: for each word in the wordlist, which
    // class does it belong to (by index)?
    word_classes: Vec<usize>,
}

#[derive(Debug, Clone)]
struct CacheClass {
    // TODO: In future optimizations, it may be useful to store other information here
    n_words: usize,
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

    // Statistics for debugging/progress
    total_prefixed: usize,
    total_matched: usize,
    total_length: usize,
    input_wordlist_length: Option<usize>,
}

impl<'word> CacheBuilder<'word> {
    fn new(expression: Expression, word_len_max: usize) -> Self {
        let mut classes = IndexMap::new();
        let word_classes = vec![];

        let states_len = expression.states_len();
        let transition_group_new = || BitSet3D::new((states_len, expression.fuzz + 1), states_len);

        // The first class (0) is always the "null" class for words which do not match
        classes.insert_full(transition_group_new(), CacheClass { n_words: 0 });

        let transition_table = vec![transition_group_new(); word_len_max];
        let start_states = expression.epsilon_states(0).to_bitset();

        Self {
            cache: Cache {
                expression,
                states_len,
                start_states,
                classes,
                word_classes,
            },

            word_index: 0,
            nonnull_wordlist: vec![],
            transition_table,
            previous_chars: &[],

            total_prefixed: 0,
            total_matched: 0,
            total_length: 0,
            input_wordlist_length: None,
            single_fully_drained: false,
        }
    }

    fn progress(&self) -> String {
        if let Some(input_wordlist_length) = self.input_wordlist_length {
            format!("{}/{}", self.word_index, input_wordlist_length)
        } else {
            "0/?".to_string()
        }
    }

    /// `RUNTIME: O(words * chars * fuzz * states^3)`
    fn next_single_word(
        &mut self,
        wordlist: &[&'word Word],
        deadline: Option<Instant>,
    ) -> Option<&'word Word> {
        // RUNTIME: O(words * chars * fuzz * states^3)
        self.input_wordlist_length
            .get_or_insert_with(|| wordlist.len());
        let mut check_count = 0;

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
                for i in 0..self.cache.states_len {
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
                .entry(self.transition_table[word_len].clone())
                .and_modify(|v| v.n_words += 1);
            if entry.index() != 0 {
                self.nonnull_wordlist.push(word);
                self.cache.word_classes.push(entry.index());
            }
            entry.or_insert(CacheClass { n_words: 1 });

            // RUNTIME: O(fuzz)
            for f in fuzz_range.clone() {
                let start_transitions = self.transition_table[word_len].slice((0, f));
                if start_transitions.contains(self.cache.states_len - 1) {
                    return Some(word);
                }
            }

            check_count += 1;
            if check_count % 256 == 0 && deadline.is_some() && Some(Instant::now()) > deadline {
                return None;
            }
        }

        self.single_fully_drained = true;
        None
    }

    // TODO: This needs to be refactored, redocumented, etc.
    fn pre_finalize(&mut self, new_wordlist: &[&'word Word]) -> bool {
        println!("finalizing: {}", self.cache.expression.text);
        assert!(self.single_fully_drained);
        // TODO: There should be an API for stats
        //println!(
        //    "prefixed={}, matched={}, elided={}",
        //    self.total_prefixed,
        //    self.total_matched - self.total_prefixed,
        //    self.total_length - self.total_matched
        //);
        //println!(
        //    "{} distinct classes with {} words",
        //    self.cache.classes.len(),
        //    self.nonnull_wordlist.len(),
        //);

        let mut new_word_classes = vec![];
        let mut i: usize = 0;

        // RUNTIME: O(words)
        assert!(self.nonnull_wordlist.len() >= new_wordlist.len());
        if self.nonnull_wordlist.len() != new_wordlist.len() {
            let mut new_nonnull_wordlist = vec![];
            for (&word, &class) in self
                .nonnull_wordlist
                .iter()
                .zip(self.cache.word_classes.iter())
            {
                if i < new_wordlist.len() && *word == *new_wordlist[i] {
                    new_word_classes.push(class);
                    new_nonnull_wordlist.push(word);
                    i += 1;
                }
            }
            assert_eq!(new_word_classes.len(), new_wordlist.len());
            assert_eq!(i, new_wordlist.len());
            self.cache.word_classes = new_word_classes;
            self.nonnull_wordlist = new_nonnull_wordlist;
        } else {
            assert_eq!(&self.nonnull_wordlist, new_wordlist);
        }
        let states_len = self.cache.states_len;

        // RUNTIME: ?
        let start = Instant::now();
        let mut reachable_states = BitSet1D::new((), states_len);
        reachable_states
            .borrow_mut()
            .union_with(self.cache.start_states.borrow());

        loop {
            let mut next_reachable_states = reachable_states.clone();

            for (transition_table, class) in self.cache.classes.iter() {
                if class.n_words == 0 {
                    continue;
                }
                for s in reachable_states.borrow().ones() {
                    for f in 0..=self.cache.expression.fuzz {
                        next_reachable_states
                            .borrow_mut()
                            .union_with(transition_table.slice((s, f)));
                    }
                }
            }

            if next_reachable_states == reachable_states {
                break;
            } else {
                reachable_states = next_reachable_states;
            }
        }
        println!(
            "Computed reachable states in {:?}, {} unreachable ({:?})",
            start.elapsed(),
            states_len - reachable_states.borrow().ones().count(),
            reachable_states
        );
        let reachable_states_ref = reachable_states.borrow();
        if !reachable_states_ref.contains(states_len - 1) {
            println!("End state is not reachable");
            self.nonnull_wordlist = vec![];
            return true;
        }

        // RUNTIME: ?
        let start = Instant::now();
        let duplicate_states: Vec<Option<usize>> = (0..states_len)
            .map(|i| {
                if reachable_states_ref.contains(i) {
                    None
                } else {
                    Some(i)
                }
            })
            .collect();

        for i in reachable_states_ref.ones() {
            #[allow(clippy::needless_range_loop)]
            'outer: for j in i + 1..states_len {
                if duplicate_states[j].is_some() {
                    continue;
                }
                for f in 0..=self.cache.expression.fuzz {
                    for (transition_table, class) in self.cache.classes.iter() {
                        if class.n_words == 0 {
                            continue;
                        }
                        for k in reachable_states_ref.ones() {
                            let slice_i = transition_table.slice((i, f));
                            let slice_j = transition_table.slice((j, f));
                            // Outputs same
                            if slice_i.contains(k) != slice_j.contains(k) {
                                continue 'outer;
                            }

                            let slice = transition_table.slice((k, f));
                            // Inputs same
                            if slice.contains(i) != slice.contains(j) {
                                continue 'outer;
                            }
                        }
                    }
                }
            }
        }
        println!(
            "checked for identical states in {:?}: {:?}",
            start.elapsed(),
            duplicate_states
        );
        // We can't elide the final state
        assert_eq!(duplicate_states[states_len - 1], None);

        // RUNTIME: ?
        let start = Instant::now();
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
            let mut new_start_states = BitSet1D::new((), new_states_len);
            for i in self.cache.start_states.borrow().ones() {
                if let Some(new_i) = new_state_index[i] {
                    new_start_states.borrow_mut().insert(new_i);
                }
            }
            println!(
                "start states: {:?} -> {:?}",
                self.cache.start_states, new_start_states
            );

            let mut class_map: Vec<usize> = vec![0; self.cache.classes.len()];
            let mut new_classes = IndexMap::new();
            new_classes.insert_full(transition_group_new(), CacheClass { n_words: 0 });
            for (index, (transition_table, class)) in self.cache.classes.iter().enumerate() {
                if class.n_words == 0 {
                    continue;
                }
                let mut new_table = transition_group_new();
                for (i, &i_new) in new_state_index.iter().enumerate() {
                    if duplicate_states[i] == Some(i) {
                        continue;
                    }
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
                let entry = new_classes.entry(new_table);
                class_map[index] = entry.index();
                entry
                    .and_modify(|c| c.n_words += class.n_words)
                    .or_insert(class.clone());
            }
            //for (i, (table, class)) in self.cache.classes.iter().enumerate() {
            //    println!(
            //        "old_class {} with {} words: {:?}",
            //        i, class.n_words, table
            //    );
            //}
            //for (i, (table, class)) in new_classes.iter().enumerate() {
            //    println!(
            //        "new_class {} with {} words: {:?}",
            //        i, class.n_words, table
            //    );
            //}
            assert_eq!(self.cache.word_classes.len(), new_wordlist.len());
            let mut new_nonnull_wordlist = vec![];
            let mut new_word_classes = vec![];
            for (&word, &wclass) in self
                .nonnull_wordlist
                .iter()
                .zip(self.cache.word_classes.iter())
            {
                if class_map[wclass] != 0 {
                    new_nonnull_wordlist.push(word);
                    new_word_classes.push(class_map[wclass]);
                }
            }
            assert!(new_nonnull_wordlist.len() <= new_wordlist.len());
            assert_eq!(new_nonnull_wordlist.len(), new_word_classes.len());
            self.nonnull_wordlist = new_nonnull_wordlist;
            self.cache.word_classes = new_word_classes;
            self.cache.classes = new_classes;
            self.cache.states_len = new_states_len;
            self.cache.start_states = new_start_states;

            //println!("word classes: {:?}", self.cache.word_classes);
            //println!("wordlist: {:#?}", new_wordlist);
            println!("class map: {:?}; states_len={}", class_map, new_states_len);
            println!(
                "remapped states in {:?} from {} -> {}",
                start.elapsed(),
                states_len,
                new_states_len
            );

            true
        } else {
            false
        }
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
        self.cache_builder.next_single_word(self.wordlist, None)
    }
}
