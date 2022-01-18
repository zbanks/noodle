use crate::bitset::BitSet3D;
use crate::expression::Expression;
use crate::matcher::{PhraseDepth, PhraseMatcher, SearchPhase, WordMatcher};
use crate::parser;
use crate::words::{Tranche, Word};
use std::time::Instant;

/// Evaluate a query, consisting of multiple expressions, on a given wordset.
/// Returns words and phrases that match the given query
pub struct QueryEvaluator<'word> {
    phase: QueryPhase<'word>,

    /// The initial limit on search depth (e.g. max number of words in a phrase)
    /// This limit is used to populate the `QueryPhase::Phrase.search_queue` list,
    search_depth_limit: PhraseDepth,

    /// Maximum number of results to return.
    /// Once reached, the evaluator moves to `QueryPhase::Done`
    results_limit: Option<usize>,

    /// Number of results returned so far, used to enforce `results_limit`
    results_count: usize,
}

/// Evaluating a query goes through three separate phases:
///  - Single word matches (while simultaneously populating the PhraseMatcher datastructure)
///  - Phrase matches
///  - Done
enum QueryPhase<'word> {
    /// Phase 1, single-word matches
    Word {
        matchers: Vec<WordMatcher<'word>>,

        /// The input wordlist (unfiltered)
        wordlist: &'word [&'word Word],
    },
    /// Phase 2, multi-word phrases
    Phrase {
        matchers: Vec<PhraseMatcher>,

        /// The "alive" wordlist, which has been filtered to only contain words that
        /// *could* contribute to a phrase match across all `PhraseMatcher`s
        wordlist: Vec<&'word Word>,

        /// The search is sort of a form of IDDFS, see
        /// https://en.wikipedia.org/wiki/Iterative_deepening_depth-first_search
        /// First we try phrases length 2, then length 3, then 4, etc.
        /// This is memory efficient, and returns short phrases first.
        /// But, it requires that we do a bunch of re-computation so it can be slower
        /// than "normal" DFS.
        search_queue: Vec<SearchPhase>,
        search_layers: Vec<SearchLayer>,
        layer_index: PhraseDepth,
        phase_had_partial_match: bool,

        /// Initial (unitless) estimate for time to perform the phrase search phase,
        /// based on the size/value of the `search_queue`. (See `search_estimate`)
        initial_search_estimate: u32,
    },
    /// Phase 3, done!
    Done,
}

/// TODO
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryResponse {
    Timeout,
    Logs(Vec<String>),
    Match(Vec<Word>),
    Complete(String),
}

/// Internal state to `QueryEvaluator` during the Phrase search phase
/// Each `SearchLayer` contains the current state up to a certain depth,
/// so a search for a 10-word phrase would use a vec of 10 `SearchLayer`s
#[derive(Debug)]
struct SearchLayer {
    /// The nth word in the wordlist
    word_index: usize,

    max_tranche: Tranche,

    /// The reachable `dst_state`s for `matcher` within `fuzz` edits
    /// *before* consuming the given word
    table_matcher_fuzz_dst: BitSet3D,
}

// --

fn search_estimate(search_phases: &[SearchPhase]) -> u32 {
    // This is just an incredibly rough guess at how long each phase will take
    // In theory each phase should be `O(total_size)`, but eliminating common
    // prefixes makes it usually run much faster.
    // (Also: sometimes total_size saturates at u64::MAX)
    // Assume each phase is roughly `O((log total_size) ^ 2)`
    search_phases
        .iter()
        .map(|p| (p.total_size as f64).log2().powf(2.0) as u32)
        .sum()
}

impl<'word> QueryEvaluator<'word> {
    pub fn new(
        expressions: Vec<Expression>,
        input_wordlist: &'word [&'word Word],
        search_depth_limit: PhraseDepth,
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
        const DEFAULT_SEARCH_DEPTH_LIMIT: PhraseDepth = 10;
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

    pub fn set_search_depth_limit(&mut self, search_depth_limit: usize) {
        assert!(matches!(self.phase, QueryPhase::Word { .. }));
        self.search_depth_limit = search_depth_limit;
    }

    pub fn set_results_limit(&mut self, results_limit: Option<usize>) {
        assert!(matches!(self.phase, QueryPhase::Word { .. }));
        self.results_limit = results_limit;
    }

    pub fn expressions(&self) -> Vec<&Expression> {
        match &self.phase {
            QueryPhase::Word { matchers, .. } => matchers.iter().map(|m| m.expression()).collect(),
            QueryPhase::Phrase { matchers, .. } => matchers.iter().map(|m| &m.expression).collect(),
            QueryPhase::Done => vec![],
        }
    }

    pub fn progress(&self) -> String {
        match &self.phase {
            QueryPhase::Word { matchers, wordlist } => matchers[0].progress(wordlist),
            QueryPhase::Phrase {
                search_layers,
                search_queue,
                initial_search_estimate,
                ..
            } => {
                let phase = &search_queue[0];
                let step_index = &search_layers[0].word_index;

                let estimate = search_estimate(search_queue);
                let step_estimate =
                    ((*step_index as f64).log2() * (phase.depth as f64)).powf(2.0) as u32;
                let estimate = initial_search_estimate - estimate.saturating_sub(step_estimate);

                format!(
                    "{}-word phrase matches: {}/{} ({}%) - tranche={}",
                    phase.depth,
                    estimate,
                    initial_search_estimate,
                    100 * estimate / initial_search_estimate,
                    phase.tranche,
                )
            }
            QueryPhase::Done => "Done".to_string(),
        }
    }

    pub fn next_within_deadline(&mut self, deadline: Option<Instant>) -> QueryResponse {
        if self.results_limit.is_some()
            && self.results_count >= self.results_limit.unwrap()
            && !matches!(self.phase, QueryPhase::Done)
        {
            self.phase = QueryPhase::Done;
            return QueryResponse::Complete(format!(
                "Found {} results, stopping",
                self.results_count
            ));
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
                        let last_word = matcher.iter(wordlist, None).last();
                        all_match = all_match && (last_word == Some(word));
                        wordlist = &matcher.alive_wordlist;
                    }

                    // A single word is match if it is returned by every matcher's iterator
                    if all_match {
                        self.results_count += 1;
                        return QueryResponse::Match(vec![word.clone()]);
                    }
                }
                if deadline.is_some() && Some(Instant::now()) > deadline {
                    return QueryResponse::Timeout;
                }

                // Now, we're done with the single-word matches
                if self.search_depth_limit <= 1 {
                    self.phase = QueryPhase::Done;
                    return QueryResponse::Complete(
                        "Complete, found all single-word matches".to_string(),
                    );
                }

                let mut log_messages = vec![];

                // Process remaining words to populate phrase-matching data, even though they won't yield any single-word matches
                let mut alive_wordlist = {
                    let mut wordlist = &first_matcher[0].alive_wordlist;
                    for matcher in remaining_matchers.iter_mut() {
                        let _ = matcher.iter(wordlist, None).count();
                        wordlist = &matcher.alive_wordlist;
                    }

                    wordlist
                }
                .clone();

                let mut tranches = alive_wordlist.iter().map(|w| w.tranche).collect::<Vec<_>>();
                tranches.dedup();
                {
                    let tranches_check = tranches.clone();
                    tranches.sort();
                    tranches.dedup();
                    assert_eq!(tranches, tranches_check);
                }

                let mut search_queue: Vec<_> = (2..=self.search_depth_limit)
                    .flat_map(|d| {
                        tranches.iter().map(move |&t| SearchPhase {
                            depth: d,
                            tranche: t,
                            total_size: 0,
                        })
                    })
                    .collect();

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

                        // Try to tighten the `search_queue` with the wordset/matchers we have so far
                        loop {
                            let l = search_queue.len();
                            for matcher in matchers.iter() {
                                matcher.filter_search_phases(&mut search_queue);
                            }
                            if search_queue.is_empty() {
                                self.phase = QueryPhase::Done;
                                return QueryResponse::Complete(
                                    "Complete, found all single-word matches (no phrases possible)"
                                        .to_string(),
                                );
                            } else if l != search_queue.len() {
                                converged = false;
                            } else {
                                break;
                            }
                        }

                        for matcher in matchers.iter_mut().rev() {
                            // Optimize each PhraseMatcher
                            let did_opt =
                                matcher.optimize_for_wordlist(&alive_wordlist, &search_queue);
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
                        if optimization_passes > 100 {
                            log_messages.push(format!(
                                "gave up after performing {} optimization passes",
                                optimization_passes
                            ));
                            break;
                        }
                    }

                    log_messages.push(format!(
                        "optimizing took {:?} in {} passes, wordlist shrunk {} -> {}",
                        start.elapsed(),
                        optimization_passes,
                        initial_size,
                        alive_wordlist.len()
                    ));
                }

                // If there are no words, there are no phrases that can match; we're done
                if alive_wordlist.is_empty() || search_queue.is_empty() {
                    self.phase = QueryPhase::Done;
                    return QueryResponse::Complete(
                        "Complete, found all single-word matches (no phrases possible)".to_string(),
                    );
                }

                let tranche_max: Tranche = search_queue.iter().map(|p| p.tranche).max().unwrap();
                let mut tranche_count: Vec<_> = vec![0; tranche_max as usize + 1];
                for word in alive_wordlist.iter() {
                    tranche_count[word.tranche as usize] += 1;
                }
                let tranche_cumulative_count: Vec<u64> = tranche_count
                    .iter()
                    .scan(0, |sum, &c| {
                        *sum += c;
                        Some(*sum)
                    })
                    .collect();

                for search_phase in search_queue.iter_mut() {
                    if tranche_count[search_phase.tranche as usize] == 0 {
                        // Skip the tranche because it has no unique words
                        search_phase.total_size = 0;
                    } else {
                        let tranche_size: u64 =
                            tranche_cumulative_count[search_phase.tranche as usize];
                        search_phase.total_size =
                            tranche_size.saturating_pow(search_phase.depth as u32);
                        assert!(search_phase.total_size > 0);
                    }
                }
                search_queue.retain(|p| p.total_size > 0);
                search_queue.sort_by_key(|p| p.total_size);

                // If there are valid items left in the search_queue, we're done
                if search_queue.is_empty() {
                    self.phase = QueryPhase::Done;
                    return QueryResponse::Complete(
                        "Complete, found all single-word matches (no phrases possible)".to_string(),
                    );
                }

                // We're done with the WordMatchers, pull out the PhraseMatchers
                let phrase_matchers: Vec<_> = matchers
                    .drain(..)
                    .filter_map(|m| m.into_phrase_matcher())
                    .collect();

                log_messages.push(format!(
                    "optimized state sizes: {:?} -> {:?}",
                    phrase_matchers
                        .iter()
                        .map(|m| m.expression.states_len())
                        .collect::<Vec<_>>(),
                    phrase_matchers
                        .iter()
                        .map(|m| m.states_len)
                        .collect::<Vec<_>>()
                ));

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

                let mut search_layers: Vec<_> =
                    (0..=search_queue.iter().map(|p| p.depth).max().unwrap())
                        .map(|_| {
                            SearchLayer::new(
                                phrase_matchers.len(),
                                fuzz_max,
                                states_max,
                                alive_wordlist[0].tranche,
                            )
                        })
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

                let initial_search_estimate = search_estimate(&search_queue);

                self.phase = QueryPhase::Phrase {
                    matchers: phrase_matchers,
                    wordlist: alive_wordlist.to_vec(),
                    search_layers,
                    search_queue,
                    layer_index: 0,
                    phase_had_partial_match: false,
                    initial_search_estimate,
                };

                QueryResponse::Logs(log_messages)
            }
            QueryPhase::Phrase {
                matchers,
                wordlist,
                search_layers,
                search_queue,
                layer_index,
                phase_had_partial_match,
                initial_search_estimate: _,
            } => {
                assert!(!wordlist.is_empty());

                let mut deadline_check_count = 0;
                let mut result = None;
                loop {
                    let search_phase = &search_queue[0];
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

                    // Keep track if there was a partial match for this search phase
                    if (all_partial_match || all_exact_match)
                        && *layer_index + 1 == search_phase.depth
                    {
                        *phase_had_partial_match = true;
                    }

                    // Exact match for the appropriate search phase
                    if all_exact_match
                        && *layer_index + 1 == search_phase.depth
                        && search_layers[*layer_index].max_tranche == search_phase.tranche
                    {
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
                        && *layer_index + 1 < search_phase.depth
                    {
                        // Descend to the next layer (and reset its word_index to 0)
                        *layer_index += 1;
                        search_layers[*layer_index].word_index = 0;
                        search_layers[*layer_index].max_tranche =
                            search_layers[*layer_index - 1].max_tranche;
                    } else {
                        // This phrase did not match, and was not a prefix to a match, so try
                        // the next "peer" phrase (or ascend)
                        loop {
                            // Try replacing the last word with the next word
                            search_layers[*layer_index].word_index += 1;

                            let word_index = search_layers[*layer_index].word_index;

                            // Did we exhaust the whole word list at this layer?
                            if word_index >= wordlist.len()
                                || wordlist[word_index].tranche > search_phase.tranche
                            {
                                // If there isn't a previous layer, then we're done with this depth
                                if *layer_index == 0 {
                                    let search_phase = search_queue.remove(0);
                                    if !*phase_had_partial_match {
                                        search_queue.retain(|p| {
                                            p.tranche > search_phase.tranche
                                                || p.depth < search_phase.depth
                                        });
                                    }

                                    if search_queue.is_empty() {
                                        // If the depth queue is empty, we're done for good!
                                        // Signal that the iterator is exhausted
                                        self.phase = QueryPhase::Done;
                                        return QueryResponse::Complete(format!(
                                            "Complete, found all {} phrases up to {} words",
                                            self.results_count, self.search_depth_limit
                                        ));
                                    } else {
                                        // Otherwise, restart at the first layer with the new depth
                                        *phase_had_partial_match = false;
                                        search_layers[*layer_index].word_index = 0;
                                        search_layers[*layer_index].max_tranche =
                                            wordlist[0].tranche;
                                        break;
                                    }
                                }

                                // Ascend back to the previous layer (perhaps recursively!)
                                *layer_index -= 1;
                                continue;
                            }

                            // Update tranche
                            let max_tranche = &mut search_layers[*layer_index].max_tranche;
                            *max_tranche = (*max_tranche).max(wordlist[word_index].tranche);

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
                        return QueryResponse::Timeout;
                    }
                }
                result.unwrap()
            }
            QueryPhase::Done => QueryResponse::Complete(
                "QueryEvaluator.next_within_deadline called repeatedly".to_string(),
            ),
        }
    }
}

impl Iterator for QueryEvaluator<'_> {
    type Item = QueryResponse;

    fn next(&mut self) -> Option<QueryResponse> {
        if matches!(self.phase, QueryPhase::Done) {
            None
        } else {
            Some(self.next_within_deadline(None))
        }
    }
}

impl SearchLayer {
    fn new(
        matcher_count: usize,
        fuzz_max: usize,
        states_max: usize,
        initial_tranche: Tranche,
    ) -> Self {
        SearchLayer {
            word_index: 0,
            max_tranche: initial_tranche,
            table_matcher_fuzz_dst: BitSet3D::new((matcher_count, fuzz_max), states_max),
        }
    }
}
