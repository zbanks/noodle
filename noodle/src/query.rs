use crate::bitset::BitSet3D;
use crate::expression::Expression;
use crate::matcher::{PhraseLength, PhraseMatcher, Tranche, WordMatcher};
use crate::parser;
use crate::words::Word;
use std::time::Instant;

/// Evaluate a query, consisting of multiple expressions, on a given wordset.
/// Returns words and phrases that match the given query
pub struct QueryEvaluator<'word> {
    phase: QueryPhase<'word>,

    search_depth_limit: PhraseLength,
    results_limit: Option<usize>,
    results_count: usize,
}

/// Evaluating a query goes through three separate phases:
///  - Single word matches (while simultaneously populating the PhraseMatcher datastructure)
///  - Phrase matches
///  - Done
enum QueryPhase<'word> {
    Word {
        matchers: Vec<WordMatcher<'word>>,

        /// The input wordlist (unfiltered)
        wordlist: &'word [&'word Word],
    },
    Phrase {
        matchers: Vec<PhraseMatcher>,

        /// The "alive" wordlist, which has been filtered to only contain words that
        /// *could* contribute to a phrase match across all `PhraseMatcher`s
        wordlist: Vec<&'word Word>,

        search_layers: Vec<SearchLayer>,
        search_depth: (PhraseLength, Tranche),
        layer_index: PhraseLength,
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

/// Internal state to `QueryEvaluator` during the Phrase search phase
/// Each `SearchLayer` contains the current state up to a certain depth,
/// so a search for a 10-word phrase would use a vec of 10 `SearchLayer`s
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
        input_wordlist: &'word [&'word Word],
        search_depth_limit: PhraseLength,
        results_limit: Option<usize>,
    ) -> Self {
        assert!(!expressions.is_empty());

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

    pub fn from_ast(query_ast: &parser::QueryAst, input_wordlist: &'word [&'word Word]) -> Self {
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
        if Some(self.results_count) >= self.results_limit {
            self.phase = QueryPhase::Done;
        }

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
                        self.results_count += 1;
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

                // Repeatedly call `optimize_for_wordlist` on each `PhraseMatcher`,
                // until nothing changes (e.g. it is fully optimized)
                //
                // TODO: Be clever to avoid ~n^2 scenario?
                // Right now, `optimize_for_wordlist` is sort of "self-centered" and
                // naive -- it's expected that it'll get called repeatedly until it
                // converges. It may be possible to rewrite to avoid arbitrary looping?
                {
                    // Some debugging vars for printing later
                    let start = Instant::now();
                    let initial_size = alive_wordlist.len();
                    let mut optimization_passes: usize = 0;

                    loop {
                        optimization_passes += 1;
                        let mut converged = true;

                        // Try to tighten the `search_depth_limit` with the wordset/matchers we have so far
                        self.search_depth_limit = {
                            let mut valid_search_depths: Vec<usize> =
                                (2..=self.search_depth_limit).collect();
                            loop {
                                let l = valid_search_depths.len();
                                for matcher in matchers.iter() {
                                    matcher.phrase_length_bounds(&mut valid_search_depths);
                                }
                                if valid_search_depths.is_empty() || l == valid_search_depths.len()
                                {
                                    break;
                                }
                            }

                            let new_limit = valid_search_depths.iter().max().copied().unwrap_or(1);
                            if new_limit <= 1 {
                                self.phase = QueryPhase::Done;
                                return None;
                            } else if new_limit != self.search_depth_limit {
                                converged = false;
                            }

                            new_limit
                        };

                        for matcher in matchers.iter_mut().rev() {
                            // Optimize each PhraseMatcher
                            let did_opt = matcher
                                .optimize_for_wordlist(&alive_wordlist, self.search_depth_limit);
                            converged = converged && !did_opt;

                            // If the optimization step reduced the `alive_wordlist`, then use that
                            // moving forward.
                            if matcher.alive_wordlist.len() != alive_wordlist.len() {
                                assert!(matcher.alive_wordlist.len() < alive_wordlist.len());
                                converged = false;

                                // TODO: can the big Vec clone be avoided?
                                alive_wordlist = matcher.alive_wordlist.clone();

                                if alive_wordlist.is_empty() {
                                    break;
                                }
                            }
                        }
                        if converged {
                            break;
                        }
                    }

                    println!(
                        "optimizing took {:?} in {} passes, wordlist shrunk {} -> {}",
                        start.elapsed(),
                        optimization_passes,
                        initial_size,
                        alive_wordlist.len()
                    );
                }

                // If there are no words, there are no phrases that can match; we're done
                if alive_wordlist.is_empty() {
                    self.phase = QueryPhase::Done;
                    return None;
                }

                // We're done with the WordMatchers, pull out the PhraseMatchers
                let phrase_matchers: Vec<_> = matchers
                    .drain(..)
                    .map(|m| m.into_phrase_matcher())
                    .collect();

                println!(
                    "optimized state sizes: {:?} -> {:?}",
                    phrase_matchers
                        .iter()
                        .map(|m| m.expression.states_len())
                        .collect::<Vec<_>>(),
                    phrase_matchers
                        .iter()
                        .map(|m| m.states_len)
                        .collect::<Vec<_>>()
                );

                // Construct the SearchLayers, which are used to hold state during DFS
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

                // Recurse, to call the QueryPhase::Phrase match arm
                self.next_within_deadline(deadline)
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

                    // all_exact_match: Does this phrase lead to the success state in all matchers?
                    let mut all_exact_match = true;
                    // all_partial_match: Does this phrase lead to a nonzero state in all matchers?
                    let mut all_partial_match = true;
                    for (m, matcher) in matchers.iter().enumerate() {
                        let prev_table_fuzz_dst = prev_layer.table_matcher_fuzz_dst.slice2d(m);
                        let mut next_table_fuzz_dst =
                            next_layer.table_matcher_fuzz_dst.slice2d_mut(m);

                        // Advance the table by one word
                        next_table_fuzz_dst.clear();
                        matcher.step_by_word_index(
                            word_index,
                            prev_table_fuzz_dst,
                            next_table_fuzz_dst,
                        );

                        // Check the table to see if it is empty and/or a success
                        let next_table_fuzz_dst = next_layer.table_matcher_fuzz_dst.slice2d(m);
                        if next_table_fuzz_dst.is_empty() {
                            // No match!
                            all_exact_match = false;
                            all_partial_match = false;
                            break;
                        } else if !matcher.has_success_state(next_table_fuzz_dst) {
                            all_exact_match = false;
                        }
                    }
                    if all_exact_match && *layer_index >= 1 {
                        self.results_count += 1;
                        result = Some(QueryResponse::Match(
                            search_layers[0..=*layer_index]
                                .iter()
                                .map(|sl| wordlist[sl.word_index].clone())
                                .collect(),
                        ));
                    }

                    // There was a partial (or exact match), so try to extend the phrase by one
                    // more word (as long as we haven't hit the search depth limit)
                    if (all_partial_match || all_exact_match)
                        && *layer_index + 1 < self.search_depth_limit
                    {
                        // Descend to the next layer (and reset its word_index to 0)
                        *layer_index += 1;
                        search_layers[*layer_index].word_index = 0;
                    } else {
                        // This phrase did not match, and was not a prefix to a match, so try
                        // the next "peer" phrase (or ascend)
                        loop {
                            // Try replacing the last word with the next word
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
        self.next_within_deadline(None)
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
