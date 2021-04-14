use crate::bitset::BitSet3D;
use crate::expression::Expression;
use crate::matcher::{PhraseLength, PhraseMatcher, Tranche, WordMatcher};
use crate::parser;
use crate::words::Word;
use std::cmp::Ord;
use std::time::Instant;

/// Evaluate a query, consisting of multiple expressions, on a given wordset.
/// Returns words and phrases that match the given query
pub struct QueryEvaluator<'word> {
    phase: QueryPhase<'word>,

    search_depth_limit: PhraseLength,
    results_limit: Option<usize>,
    results_count: usize,
}

/// TODO
enum QueryPhase<'word> {
    Word {
        matchers: Vec<WordMatcher<'word>>,
        wordlist: Vec<&'word Word>,
    },
    Phrase {
        matchers: Vec<PhraseMatcher>,
        wordlist: Vec<&'word Word>,
        search_layers: Vec<SearchLayer>,
        search_depth: (PhraseLength, Tranche),
        layer_index: usize,
    },
    Done,
}

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryResponse {
    Timeout,
    Logs(Vec<String>),
    Match(Vec<Word>),
}

/// TODO
#[derive(Debug)]
struct SearchLayer {
    /// The nth word in the wordlist
    word_index: usize,

    /// The reachable `dst_state`s for `matcher` within `fuzz` edits
    /// *before* consuming the given word
    table_matcher_fuzz_dst: BitSet3D,
}

// --

impl<'word> QueryEvaluator<'word> {
    pub fn new(
        expressions: Vec<Expression>,
        input_wordlist: &[&'word Word],
        search_depth_limit: PhraseLength,
        results_limit: Option<usize>,
    ) -> Self {
        assert!(!expressions.is_empty());

        // TODO: This does a copy of the wordlist slice, which would not be great
        // if we had ~millions of words.
        let input_wordlist = input_wordlist.to_vec();

        // TODO: Remove +1
        let max_word_len = 1 + input_wordlist
            .iter()
            .map(|w| w.chars.len())
            .max()
            .unwrap_or(0);

        let word_matchers = expressions
            .into_iter()
            .map(|expr| WordMatcher::new(expr, max_word_len))
            .collect();

        QueryEvaluator {
            phase: QueryPhase::Word {
                matchers: word_matchers,
                wordlist: input_wordlist,
            },
            search_depth_limit,
            results_limit,
            results_count: 0,
        }
    }

    pub fn from_ast(query_ast: &parser::QueryAst, input_wordlist: &[&'word Word]) -> Self {
        // TODO: Use `options.dictionary`
        // TODO: Use `options.quiet`
        // TODO: Maybe `results_limit` should be handled upstream?
        const DEFAULT_SEARCH_DEPTH_LIMIT: PhraseLength = 10;
        const DEFAULT_RESULTS_LIMIT: usize = 300;

        let search_depth_limit = query_ast
            .options
            .max_words
            .unwrap_or(DEFAULT_SEARCH_DEPTH_LIMIT);
        let results_limit = query_ast
            .options
            .results_limit
            .or(Some(DEFAULT_RESULTS_LIMIT));

        let expressions: Vec<_> = query_ast
            .expressions
            .iter()
            .map(|expr| Expression::from_ast(expr))
            .collect();

        Self::new(
            expressions,
            input_wordlist,
            search_depth_limit,
            results_limit,
        )
    }

    pub fn expressions(&self) -> Vec<&Expression> {
        match &self.phase {
            QueryPhase::Word { matchers, .. } => matchers.iter().map(|m| m.expression()).collect(),
            QueryPhase::Phrase { matchers, .. } => matchers.iter().map(|m| &m.expression).collect(),
            QueryPhase::Done => vec![],
        }
    }

    pub fn progress(&self) -> String {
        format!("TODO")
    }

    pub fn next_within_deadline(&mut self, deadline: Option<Instant>) -> Option<QueryResponse> {
        match &mut self.phase {
            QueryPhase::Word { matchers, wordlist } => {
                // Check for single-word matches
                let (first_matcher, remaining_matchers) = matchers.split_at_mut(1);

                // Iterate over every word which satisfies the first matcher...
                while let Some(word) = first_matcher[0].next_single_word(wordlist, deadline) {
                    // ...then have all of the remaining matchers consume the (growing) `alive_wordlist`
                    // The `alive_wordlist` of matcher `i` is fed into matcher `i+1`
                    let mut wordlist = &first_matcher[0].alive_wordlist;
                    let mut all_match = true;
                    for matcher in remaining_matchers.iter_mut() {
                        let last_word = matcher.iter(wordlist).last();
                        all_match = all_match && (last_word == Some(word));
                        wordlist = &matcher.alive_wordlist;
                    }

                    // A single word is match if it is returned by every matcher's iterator
                    if all_match {
                        return Some(QueryResponse::Match(vec![word.clone()]));
                    }
                }
                if deadline.is_some() && Some(Instant::now()) > deadline {
                    return Some(QueryResponse::Timeout);
                }

                // Now, we're done with the single-word matches
                if self.search_depth_limit <= 1 {
                    self.phase = QueryPhase::Done;
                    return None;
                }

                // Process remaining words to populate phrase-matching data, even though they won't yield any single-word matches
                let mut alive_wordlist = {
                    let mut wordlist = &first_matcher[0].alive_wordlist;
                    for matcher in remaining_matchers.iter_mut() {
                        let _ = matcher.iter(wordlist).count();
                        wordlist = &matcher.alive_wordlist;
                    }

                    wordlist
                }
                .clone();

                // TODO: Be clever to avoid ~n^2 scenario?
                {
                    let initial_size = alive_wordlist.len();
                    let start = Instant::now();
                    loop {
                        let mut optimized = false;
                        for matcher in matchers.iter_mut().rev() {
                            let opt = matcher.optimize_for_wordlist(&alive_wordlist);
                            optimized = optimized || opt;

                            if matcher.alive_wordlist.len() != alive_wordlist.len() {
                                assert!(matcher.alive_wordlist.len() < alive_wordlist.len());
                                alive_wordlist = matcher.alive_wordlist.clone();
                                if alive_wordlist.is_empty() {
                                    break;
                                }
                            }
                        }
                        if !optimized {
                            break;
                        }
                    }

                    println!(
                        "optimizing took {:?}, wordlist shrunk {} -> {}",
                        start.elapsed(),
                        initial_size,
                        alive_wordlist.len()
                    );
                }

                // TODO:
                if alive_wordlist.is_empty() {
                    self.phase = QueryPhase::Done;
                    return None;
                }

                let phrase_matchers: Vec<_> = matchers
                    .drain(..)
                    .map(|m| m.into_phrase_matcher())
                    .collect();

                // TODO
                for phrase_matcher in phrase_matchers.iter() {
                    self.search_depth_limit = self
                        .search_depth_limit
                        .min(phrase_matcher.phrase_length_bounds(self.search_depth_limit));
                }

                let states_max = phrase_matchers
                    .iter()
                    .map(|pm| pm.states_len)
                    .max()
                    .unwrap();

                let fuzz_max = phrase_matchers
                    .iter()
                    .map(|pm| pm.fuzz_limit)
                    .max()
                    .unwrap();

                let mut search_layers: Vec<_> = (0..=self.search_depth_limit)
                    .map(|_| SearchLayer::new(phrase_matchers.len(), fuzz_max, states_max))
                    .collect();

                for (i, phrase_matcher) in phrase_matchers.iter().enumerate() {
                    search_layers[0]
                        .table_matcher_fuzz_dst
                        .slice2d_mut(i)
                        .clear();
                    search_layers[0]
                        .table_matcher_fuzz_dst
                        .slice_mut((i, 0))
                        .union_with(phrase_matcher.start_states.borrow());
                }

                self.phase = QueryPhase::Phrase {
                    matchers: phrase_matchers,
                    wordlist: alive_wordlist.to_vec(),
                    search_layers,
                    search_depth: (2, 1),
                    layer_index: 0,
                };

                None
            }
            QueryPhase::Phrase {
                matchers,
                wordlist,
                search_layers,
                search_depth: _,
                layer_index,
            } => {
                assert!(!wordlist.is_empty());

                let mut deadline_check_count = 0;
                let mut result = None;
                loop {
                    let (lower_layers, upper_layers) = search_layers.split_at_mut(*layer_index + 1);
                    let prev_layer = &mut lower_layers[*layer_index];
                    let next_layer = &mut upper_layers[0];

                    let word_index = prev_layer.word_index;

                    let mut all_end_match = true;
                    let mut all_no_advance = true;
                    let mut no_match = false;
                    for (m, matcher) in matchers.iter().enumerate() {
                        let prev_table_fuzz_dst = prev_layer.table_matcher_fuzz_dst.slice2d(m);
                        let mut next_table_fuzz_dst =
                            next_layer.table_matcher_fuzz_dst.slice2d_mut(m);
                        next_table_fuzz_dst.clear();

                        matcher.step_by_word_index(
                            word_index,
                            prev_table_fuzz_dst,
                            next_table_fuzz_dst,
                        );
                        let next_table_fuzz_dst = next_layer.table_matcher_fuzz_dst.slice2d_mut(m);

                        let mut all_empty = true;
                        let mut all_subset = true;
                        let mut any_end_match = false;
                        let success_state = matcher.states_len - 1;
                        for f in 0..matcher.fuzz_limit {
                            let dst_states = next_table_fuzz_dst.slice(f);
                            if dst_states.contains(success_state) {
                                any_end_match = true;
                                all_empty = false;
                                all_subset = false;
                            } else if !dst_states.is_empty() {
                                if !dst_states
                                    .is_subset(&prev_layer.table_matcher_fuzz_dst.slice((m, f)))
                                {
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
                    if all_no_advance {
                        no_match = true;
                    }
                    if !no_match {
                        if all_end_match && *layer_index >= 1 {
                            result = Some(QueryResponse::Match(
                                search_layers[0..=*layer_index]
                                    .iter()
                                    .map(|sl| wordlist[sl.word_index].clone())
                                    .collect(),
                            ));
                        }
                    }

                    let mut partial_match = !no_match;
                    loop {
                        if partial_match {
                            // If we've hit the `max_words` limit, too bad.
                            // Treat it like we didn't have a partial match
                            if *layer_index + 1 >= self.search_depth_limit {
                                partial_match = false;
                                continue;
                            }

                            // Descend to the next layer (resetting the layer's word_index to 0)
                            *layer_index += 1;
                            search_layers[*layer_index].word_index = 0;
                            break;
                        } else {
                            // This word didn't create a match, so try the next word
                            search_layers[*layer_index].word_index += 1;

                            // Did we exhaust the whole word list at this layer?
                            if search_layers[*layer_index].word_index >= wordlist.len() {
                                // If there isn't a previous layer, then we're done!
                                if *layer_index == 0 {
                                    println!(
                                        "Matcher done with {} result(s) (up to {} word phrases)",
                                        self.results_count, self.search_depth_limit
                                    );

                                    // Signal that the iterator is exhausted
                                    self.phase = QueryPhase::Done;
                                    return None;
                                }

                                // Ascend back to the previous layer (perhaps recursively!)
                                *layer_index -= 1;
                                continue;
                            }
                            break;
                        }
                    }

                    if result.is_some() {
                        break;
                    }

                    deadline_check_count += 1;
                    if deadline_check_count % 256 == 0
                        && deadline.is_some()
                        && Some(Instant::now()) > deadline
                    {
                        return Some(QueryResponse::Timeout);
                    }
                }
                result
            }
            QueryPhase::Done => None,
        }
    }
}

impl Iterator for QueryEvaluator<'_> {
    type Item = QueryResponse;

    fn next(&mut self) -> Option<QueryResponse> {
        while !matches!(self.phase, QueryPhase::Done) {
            let result = self.next_within_deadline(None);
            if result.is_some() {
                self.results_count += 1;
                if Some(self.results_count) >= self.results_limit {
                    self.phase = QueryPhase::Done;
                }
                return result;
            }
        }
        None
    }
}

impl SearchLayer {
    fn new(matcher_count: usize, fuzz_max: usize, states_max: usize) -> Self {
        SearchLayer {
            word_index: 0,
            table_matcher_fuzz_dst: BitSet3D::new((matcher_count, fuzz_max), states_max),
        }
    }
}
