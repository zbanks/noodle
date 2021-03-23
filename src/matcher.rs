use crate::bitset::Set;
use crate::expression::{Expression, TransitionGroup};
use crate::words::*;
use indexmap::IndexMap;
use std::fmt;
use std::time;

#[derive(Debug)]
struct CacheClass {
    n_words: usize,
}

#[derive(Debug)]
struct Cache<'expr, 'word> {
    expression: &'expr Expression,
    nonnull_words: Vec<&'word Word>,
    classes: IndexMap<TransitionGroup, CacheClass>,
    word_classes: Vec<usize>,
}

impl<'expr, 'word> Cache<'expr, 'word> {
    fn new(expression: &'expr Expression, wordlist: &[&'word Word]) -> Self {
        let start = time::Instant::now();
        let mut nonnull_words = vec![];
        // Preallocate a bunch of space (but not too much)
        let mut classes = IndexMap::new();
        let mut word_classes = vec![];

        let transition_group_new =
            || TransitionGroup::new(expression.states_len(), expression.fuzz + 1);

        // The first class (0) is always the "empty" class
        let rc = classes.insert_full(transition_group_new(), CacheClass { n_words: 0 });
        //assert!(rc.1);

        let word_len_max = 1 + wordlist.iter().map(|w| w.chars.len()).max().unwrap_or(0);

        let mut transition_table = vec![transition_group_new(); word_len_max];

        let mut previous_chars: &[Char] = &[];
        let mut total_prefixed: usize = 0;
        let mut total_matched: usize = 0;
        let mut total_length: usize = 0;

        for &word in wordlist {
            let word_len = word.chars.len();
            let mut si: usize = 0;
            while si < word_len && si < previous_chars.len() && word.chars[si] == previous_chars[si]
            {
                si += 1;
            }

            let prefixed_table = &mut transition_table[si..];
            if si == 0 {
                expression.init_transition_table(&mut prefixed_table[0]);
            }
            let valid_len =
                expression.fill_transition_table(&word.chars[si..], prefixed_table) + si;

            total_prefixed += si;
            total_matched += valid_len;
            total_length += word_len;

            previous_chars = &word.chars[0..valid_len];
            if valid_len < word_len {
                continue;
            }
            assert!(valid_len == word_len);

            let entry = classes
                .entry(transition_table[word_len].clone())
                .and_modify(|v| v.n_words += 1);
            if entry.index() != 0 {
                nonnull_words.push(word);
                word_classes.push(entry.index());
            }
            entry.or_insert(CacheClass { n_words: 1 });
        }

        let duration = start.elapsed();
        println!(
            "prefixed={}, matched={}, elided={}",
            total_prefixed,
            total_matched - total_prefixed,
            total_length - total_matched
        );
        println!(
            "{} distinct classes with {}/{} words in {:?}",
            classes.len(),
            nonnull_words.len(),
            wordlist.len(),
            duration
        );

        Self {
            expression,
            nonnull_words,
            classes,
            word_classes,
        }
    }

    fn reduce_wordlist(&mut self, new_wordlist: &[&'word Word]) {
        let mut new_word_classes = vec![];
        let mut i: usize = 0;
        for (&word, &class) in self.nonnull_words.iter().zip(self.word_classes.iter()) {
            if i < new_wordlist.len() && *word == *new_wordlist[i] {
                new_word_classes.push(class);
                i += 1;
            }
        }
        assert!(i == new_wordlist.len());
        self.nonnull_words.clear();
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
    wordlist: Vec<&'word Word>,
    word_order: Vec<usize>,

    //fuzz_max: usize,
    words_max: usize,

    index: usize,
    match_count: usize,
}

impl<'expr, 'word> Matcher<'expr, 'word> {
    pub fn new(
        expressions: &'expr [Expression],
        wordlist: &'word [&'word Word],
        words_max: usize,
    ) -> Self {
        assert!(!expressions.is_empty());

        let mut caches = vec![];
        for expr in expressions {
            let words = caches
                .last()
                .map(|c: &Cache| c.nonnull_words.as_slice())
                .unwrap_or(wordlist);
            let cache = Cache::new(expr, words);
            caches.push(cache);
        }
        let (fixup_caches, last_cache) = caches.split_at_mut(expressions.len() - 1);
        let nonnull_wordlist = last_cache[0].nonnull_words.clone();
        fixup_caches
            .iter_mut()
            .for_each(|c| c.reduce_wordlist(&nonnull_wordlist));

        let mut word_order: Vec<usize> = (0..nonnull_wordlist.len()).collect();
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
            //fuzz_max,
            words_max,
            index: 0,
            match_count: 0,
        }
    }

    fn format_output(&self) -> String {
        self.layers[0..=self.index]
            .iter()
            .map(|layer| layer.stem.unwrap().text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn next_match(&mut self) -> Option<String> {
        let mut result = None;
        loop {
            let (lower_layers, upper_layers) = self.layers.split_at_mut(self.index + 1);
            let this_layer = &mut lower_layers[self.index];
            let next_layer = &mut upper_layers[0];

            let mut no_match = false;
            let mut all_end_match = true;
            let mut all_no_advance = true;
            let wi = self.word_order[this_layer.wi];
            let word = self.wordlist[wi];
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
                if all_end_match {
                    result = Some(self.format_output());
                    self.match_count += 1;
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
                if self.layers[self.index].wi >= self.wordlist.len() {
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

impl<'expr, 'word> fmt::Debug for Matcher<'expr, 'word> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Matcher for {} expressions", self.caches.len())
    }
}
