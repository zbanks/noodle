use crate::bitset::BitSet3D;
use crate::expression::Expression;
use crate::parser;
use crate::words::*;
use indexmap::IndexMap;
use std::fmt;

#[derive(Debug)]
struct CacheClass {
    n_words: usize,
}

struct CacheBuilder<'word> {
    cache: Cache,

    index: usize,
    // We want an immutable reference to this, but with it allowed
    // to change out underneath us
    // This is owned by us and populated by us, others take a reference to it
    nonnull_wordlist: Vec<&'word Word>,
    transition_table: Vec<BitSet3D>,
    previous_chars: &'word [Char],
    single_fully_drained: bool,

    total_prefixed: usize,
    total_matched: usize,
    total_length: usize,
}

struct CacheBuilderIter<'word, 'it> {
    cache_builder: &'it mut CacheBuilder<'word>,
    wordlist: &'it [&'word Word],
}

impl<'word> CacheBuilder<'word> {
    fn iter<'it>(&'it mut self, wordlist: &'it [&'word Word]) -> CacheBuilderIter<'word, 'it> {
        CacheBuilderIter {
            cache_builder: self,
            wordlist,
        }
    }

    fn new(expression: Expression, word_len_max: usize) -> Self {
        let mut classes = IndexMap::new();
        let word_classes = vec![];

        let transition_group_new = || {
            BitSet3D::new(
                (expression.states_len(), expression.fuzz + 1),
                expression.states_len(),
            )
        };

        // The first class (0) is always the "empty" class
        classes.insert_full(transition_group_new(), CacheClass { n_words: 0 });

        let transition_table = vec![transition_group_new(); word_len_max];

        Self {
            cache: Cache {
                expression,
                classes,
                word_classes,
            },

            index: 0,
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
        while self.index < wordlist.len() {
            let word = &wordlist[self.index];
            self.index += 1;

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
                if start_transitions.contains(self.cache.expression.states_len() - 1) {
                    return Some(word);
                }
            }
        }

        self.single_fully_drained = true;
        None
    }

    /// `RUNTIME: O(words)`
    fn finalize(mut self, new_wordlist: &[&'word Word]) -> Cache {
        assert!(self.single_fully_drained);
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
        for (&word, &class) in self
            .nonnull_wordlist
            .iter()
            .zip(self.cache.word_classes.iter())
        {
            if i < new_wordlist.len() && *word == *new_wordlist[i] {
                new_word_classes.push(class);
                i += 1;
            }
        }
        assert!(i == new_wordlist.len());
        self.cache.word_classes = new_word_classes;
        assert!(self.cache.word_classes.len() == new_wordlist.len());

        self.cache
    }
}

impl<'word> Iterator for CacheBuilderIter<'word, '_> {
    type Item = &'word Word;

    fn next(&mut self) -> Option<&'word Word> {
        self.cache_builder.next_single_word(self.wordlist)
    }
}

#[derive(Debug)]
struct Cache {
    expression: Expression,
    classes: IndexMap<BitSet3D, CacheClass>,
    word_classes: Vec<usize>,
}

#[derive(Debug)]
struct Layer<'word> {
    wi: usize,
    stem: Option<&'word Word>,
    states: BitSet3D,
}

impl<'word> Layer<'word> {
    fn new(caches_count: usize, fuzz_count: usize, states_count: usize) -> Self {
        Self {
            wi: 0,
            stem: None,
            states: BitSet3D::new((caches_count, fuzz_count), states_count),
        }
    }
}

pub struct Matcher<'word> {
    cache_builders: Vec<CacheBuilder<'word>>,
    caches: Vec<Cache>,
    layers: Vec<Layer<'word>>,
    // TODO: On `new`, this is populated with the input wordlist;
    // but later it is replaced with the last `nonnull_wordlist`
    // We don't really need the input wordlist?
    wordlist: Vec<&'word Word>,
    singles_done: bool,

    max_words: usize,
    results_limit: Option<usize>,

    index: usize,
    results_count: usize,
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
            index: 0,
            results_count: 0,
        }
    }

    /// `RUNTIME: O(expressions * (words + fuzz * states))`
    fn next_single(&mut self) -> Option<String> {
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
                return Some(word.text.clone());
            }
        }

        // Process the remaining words (even though they can't be single-word matches)
        let mut wl = &first_cache[0].nonnull_wordlist;
        for cache in remaining_caches.iter_mut() {
            let _ = cache.iter(wl).count();
            wl = &cache.nonnull_wordlist;
        }

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
            .collect();

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
            layers[0].states.slice2d_mut(i).clear();

            // TODO: Expand epsilon_states(0) out to states_max
            let mut starting_states = layers[0].states.slice_mut((i, 0));
            starting_states.union_with(cache.expression.epsilon_states(0));
        }

        // TODO: This wordlist copy is also potentially more expensive than nessassary,
        // but it keeps the type of `Matcher::wordlist` simple (`Vec<_>` not `Rc<RefCell<Vec<_>>>`)
        self.wordlist = nonnull_wordlist.clone();
        self.layers = layers;
        self.singles_done = true;
        None
    }

    fn format_output(&self) -> String {
        self.layers[0..=self.index]
            .iter()
            .map(|layer| layer.stem.unwrap().text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// `RUNTIME: O(words^max_words * expressions * fuzz^2 * states^2)`
    fn next_phrase(&mut self) -> Option<String> {
        assert!(self.singles_done);
        if self.wordlist.is_empty() {
            return None;
        }

        // RUNTIME: O(words^max_words * expressions * fuzz^2 * states^2)
        let mut result = None;
        loop {
            let mut no_match = false;
            {
                let (lower_layers, upper_layers) = self.layers.split_at_mut(self.index + 1);
                let this_layer = &mut lower_layers[self.index];
                let next_layer = &mut upper_layers[0];

                // RUNTIME: O(expressions * fuzz^2 * states^2)
                let mut all_end_match = true;
                let mut all_no_advance = true;
                let wi = this_layer.wi;
                let word = self.wordlist[wi];
                for (c, cache) in self.caches.iter().enumerate() {
                    if cache.word_classes[wi] == 0 {
                        no_match = true;
                        break;
                    }

                    let class = cache.classes.get_index(cache.word_classes[wi]).unwrap().0;
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
                    if all_end_match && self.index >= 1 {
                        result = Some(self.format_output());
                    }
                }
            }

            if self.advance(!no_match) || result.is_some() {
                break;
            }
        }

        result
    }

    /// `RUNTIME: O(max_words)`
    fn advance(&mut self, partial_match: bool) -> bool {
        let mut pm = partial_match;
        loop {
            if !pm {
                self.layers[self.index].wi += 1;
                if self.layers[self.index].wi >= self.wordlist.len() {
                    if self.index == 0 {
                        println!(
                            "Matcher done with {} result(s) (up to {} word phrases)",
                            self.results_count, self.max_words
                        );
                        return true;
                    }
                    self.index -= 1;
                    continue;
                }
                break;
            } else {
                if self.index + 1 >= self.max_words {
                    pm = false;
                    continue;
                }
                self.index += 1;
                self.layers[self.index].wi = 0;
                break;
            }
        }
        false
    }
}

impl Iterator for Matcher<'_> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let r = if self
            .results_limit
            .map(|lim| lim < self.results_count)
            .unwrap_or(false)
        {
            None
        } else if !self.singles_done {
            self.next_single().or_else(|| self.next_phrase())
        } else {
            self.next_phrase()
        };
        if r.is_some() {
            self.results_count += 1;
        }
        r
    }
}

impl<'word> fmt::Debug for Matcher<'word> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Matcher for {} expressions", self.caches.len())
    }
}
