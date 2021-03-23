use crate::bitset::Set;
use crate::expression::{Expression, TransitionGroup};
use crate::words::*;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::time;

#[derive(Debug)]
struct CacheClass {
    n_words: usize,
}

#[derive(Debug)]
struct Cache<'expr, 'word> {
    expression: &'expr Expression,
    nonnull_words: Rc<RefCell<Vec<&'word Word>>>,
    classes: IndexMap<TransitionGroup, CacheClass>,
    word_classes: Vec<usize>,

    word_len_max: usize,
    index: usize,
    wordlist: Rc<RefCell<Vec<&'word Word>>>,
    transition_table: Vec<TransitionGroup>,
    previous_chars: &'word [Char],

    total_prefixed: usize,
    total_matched: usize,
    total_length: usize,
}

impl<'word> Iterator for Cache<'_, 'word> {
    type Item = &'word Word;
    fn next(&mut self) -> Option<&'word Word> {
        self.next_match()
    }
}

impl<'expr, 'word> Cache<'expr, 'word> {
    fn new(
        expression: &'expr Expression,
        wordlist: Rc<RefCell<Vec<&'word Word>>>,
        word_len_max: usize,
    ) -> Self {
        let start = time::Instant::now();
        let nonnull_words = Rc::new(RefCell::new(vec![]));
        let mut classes = IndexMap::new();
        let word_classes = vec![];

        let transition_group_new =
            || TransitionGroup::new(expression.states_len(), expression.fuzz + 1);

        // The first class (0) is always the "empty" class
        classes.insert_full(transition_group_new(), CacheClass { n_words: 0 });

        let transition_table = vec![transition_group_new(); word_len_max];

        Self {
            expression,
            nonnull_words,
            classes,
            word_classes,

            word_len_max,
            index: 0,
            wordlist: wordlist,
            transition_table: transition_table,
            previous_chars: &[],

            total_prefixed: 0,
            total_matched: 0,
            total_length: 0,
        }
    }

    fn next_match(&mut self) -> Option<&'word Word> {
        let wordlist = self.wordlist.borrow();
        let mut nonnull_words = self.nonnull_words.borrow_mut();
        while self.index < wordlist.len() {
            let word = &wordlist[self.index];
            self.index += 1;

            let word_len = word.chars.len();
            let mut si: usize = 0;
            while si < word_len
                && si < self.previous_chars.len()
                && word.chars[si] == self.previous_chars[si]
            {
                si += 1;
            }

            let prefixed_table = &mut self.transition_table[si..];
            if si == 0 {
                self.expression
                    .init_transition_table(&mut prefixed_table[0]);
            }
            let valid_len = self
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

            let entry = self
                .classes
                .entry(self.transition_table[word_len].clone())
                .and_modify(|v| v.n_words += 1);
            if entry.index() != 0 {
                nonnull_words.push(word);
                self.word_classes.push(entry.index());
            }
            entry.or_insert(CacheClass { n_words: 1 });

            for fuzz_states in self.transition_table[word_len].slice(0) {
                if fuzz_states.contains(self.expression.states_len() - 1) {
                    //println!("one word match: {}", word);
                    return Some(word);
                }
            }
        }

        None
    }

    fn reduce_wordlist(&mut self, new_wordlist: &[&'word Word]) {
        let mut nonnull_words = self.nonnull_words.borrow_mut();

        println!(
            "prefixed={}, matched={}, elided={}",
            self.total_prefixed,
            self.total_matched - self.total_prefixed,
            self.total_length - self.total_matched
        );
        println!(
            "{} distinct classes with {}/{} words",
            self.classes.len(),
            nonnull_words.len(),
            self.wordlist.borrow().len(),
        );

        let mut new_word_classes = vec![];
        let mut i: usize = 0;
        for (&word, &class) in nonnull_words.iter().zip(self.word_classes.iter()) {
            if i < new_wordlist.len() && *word == *new_wordlist[i] {
                new_word_classes.push(class);
                i += 1;
            }
        }
        assert!(i == new_wordlist.len());
        nonnull_words.clear();
        self.word_classes = new_word_classes;
        assert!(self.word_classes.len() == new_wordlist.len());
    }
}

#[derive(Debug)]
struct Layer<'word> {
    wi: usize,
    stem: Option<&'word Word>,
    states: TransitionGroup,
}

impl<'word> Layer<'word> {
    fn new(caches_count: usize, fuzz_count: usize) -> Self {
        Self {
            wi: 0,
            stem: None,
            states: TransitionGroup::new(caches_count, fuzz_count),
        }
    }
}

pub struct Matcher<'expr, 'word> {
    //expressions: Vec<&'a Expression>,
    caches: Vec<Cache<'expr, 'word>>,
    layers: Vec<Layer<'word>>,
    wordlist: Rc<RefCell<Vec<&'word Word>>>,
    word_order: Vec<usize>,
    single_matches: Vec<&'word Word>,

    //fuzz_max: usize,
    words_max: usize,

    index: usize,
    match_count: usize,
}

impl<'expr, 'word> Matcher<'expr, 'word> {
    pub fn new(
        expressions: &'expr [Expression],
        wordlist: Rc<RefCell<Vec<&'word Word>>>,
        words_max: usize,
    ) -> Self {
        assert!(!expressions.is_empty());

        let word_len_max = 1 + wordlist
            .borrow()
            .iter()
            .map(|w| w.chars.len())
            .max()
            .unwrap_or(0);

        let mut caches = vec![];
        for expr in expressions {
            let words = caches
                .last()
                .map(|c: &Cache| c.nonnull_words.clone())
                .unwrap_or(wordlist.clone());
            let cache = Cache::new(expr, words, word_len_max);
            caches.push(cache);
        }

        // Check for single-word matches
        // TODO: Emit these as matches as they're calculated
        let mut single_matches = vec![];
        let (first_cache, remaining_caches) = caches.split_at_mut(1);
        for word in &mut first_cache[0] {
            let mut all_match = true;
            for cache in remaining_caches.iter_mut() {
                let last_word = cache.last();
                all_match = all_match && (last_word == Some(word));
            }
            if all_match {
                single_matches.push(word);
            }
        }

        // Process the remaining words (even though they can't be single-word matches)
        remaining_caches.iter_mut().for_each(|c| {
            c.count();
        });

        let (fixup_caches, last_cache) = caches.split_at_mut(expressions.len() - 1);
        let nonnull_wordlist = last_cache[0].nonnull_words.clone();
        fixup_caches
            .iter_mut()
            .for_each(|c| c.reduce_wordlist(&nonnull_wordlist.borrow()));

        let mut word_order: Vec<usize> = (0..nonnull_wordlist.borrow().len()).collect();
        word_order.sort_by_key(|&wi| {
            caches
                .iter()
                .map(|expr| {
                    expr.classes
                        .get_index(expr.word_classes[wi])
                        .unwrap()
                        .1
                        .n_words
                })
                .min()
        });

        let fuzz_max = expressions.iter().map(|expr| expr.fuzz).max().unwrap_or(0);
        let mut layers: Vec<Layer<'word>> = (0..=words_max)
            .map(|_| Layer::new(expressions.len(), fuzz_max + 1))
            .collect();

        for (i, expr) in expressions.iter().enumerate() {
            expr.init_transitions_start(layers[0].states.slice_mut(i));
        }

        Matcher {
            caches,
            layers,
            wordlist: nonnull_wordlist,
            word_order,
            single_matches,
            //fuzz_max,
            words_max,
            index: 0,
            match_count: 0,
        }
    }

    pub fn reset(&mut self) {
        self.index = 0;
        self.match_count = 0;
    }

    fn format_output(&self) -> String {
        self.layers[0..=self.index]
            .iter()
            .map(|layer| layer.stem.unwrap().text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn next_match(&mut self) -> Option<String> {
        if let Some(word) = self.single_matches.pop() {
            return Some(word.text.clone());
        }

        let mut result = None;
        loop {
            let mut no_match = false;
            {
                let wordlist = self.wordlist.borrow();
                let (lower_layers, upper_layers) = self.layers.split_at_mut(self.index + 1);
                let this_layer = &mut lower_layers[self.index];
                let next_layer = &mut upper_layers[0];

                let mut all_end_match = true;
                let mut all_no_advance = true;
                //let wi = self.word_order[this_layer.wi];
                let wi = this_layer.wi;
                let word = wordlist[wi];
                for (c, cache) in self.caches.iter().enumerate() {
                    if cache.word_classes[wi] == 0 {
                        no_match = true;
                        break;
                    }

                    let fuzz_limit = cache.expression.fuzz + 1;
                    let end_ss = &mut next_layer.states.slice_mut(c)[0..fuzz_limit];
                    let states = &this_layer.states.slice(c)[0..fuzz_limit];

                    end_ss.iter_mut().for_each(|e| e.clear());

                    let class = cache.classes.get_index(cache.word_classes[wi]).unwrap().0;

                    for (f, fuzz_states) in states.iter().enumerate() {
                        for si in fuzz_states.ones() {
                            let mut fd = 0;
                            while f + fd < fuzz_limit {
                                end_ss[f + fd].union_with(&class.slice(si)[fd]);
                                fd += 1;
                            }
                        }
                    }

                    let mut all_empty = true;
                    let mut all_subset = true;
                    let mut any_end_match = false;
                    let success_index = cache.expression.states_len() - 1;
                    for (i, es) in end_ss.iter().enumerate() {
                        if es.contains(success_index) {
                            any_end_match = true;
                            all_empty = false;
                            all_subset = false;
                        } else if !es.is_empty() {
                            if !es.is_subset(&states[i]) {
                                all_subset = false;
                            }
                            all_empty = false;
                        }
                    }

                    if all_empty {
                        assert!(!any_end_match);
                        no_match = true;
                        break;
                    }
                    if !all_subset {
                        all_no_advance = false;
                    }
                    if !any_end_match {
                        all_end_match = false;
                    }
                }

                // Unclear if this optimization is worth it (even though it does help prevent .* blowouts)
                if all_no_advance {
                    no_match = true;
                }

                if !no_match {
                    this_layer.stem = Some(word);
                    if all_end_match && self.index >= 1 {
                        result = Some(self.format_output());
                        self.match_count += 1;
                    }
                }
            }

            if self.advance(!no_match) || result.is_some() {
                break;
            }
        }

        result
    }

    fn advance(&mut self, partial_match: bool) -> bool {
        let mut pm = partial_match;
        loop {
            if !pm {
                self.layers[self.index].wi += 1;
                if self.layers[self.index].wi >= self.wordlist.borrow().len() {
                    if self.index == 0 {
                        println!(
                            "Matcher done with {} results (up to {} words)",
                            self.match_count, self.words_max
                        );
                        return true;
                    }
                    self.index -= 1;
                    continue;
                }
                break;
            } else {
                if self.index + 1 >= self.words_max {
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

impl Iterator for Matcher<'_, '_> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        self.next_match()
    }
}

impl<'expr, 'word> fmt::Debug for Matcher<'expr, 'word> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Matcher for {} expressions", self.caches.len())
    }
}
