use crate::bitset::{BitSet1D, BitSet2D, BitSet3D, BitSetRef2D, BitSetRefMut2D};
use crate::expression::Expression;
use crate::words::{Char, Tranche, Word};
use indexmap::IndexMap;
use std::time::Instant;

/// Search depth for phrase length, the number of words
pub type PhraseDepth = usize;

/// State for evaluating which words in a list match a given expression.
///
/// While evaluating, also populate the `PhraseMatcher` datastructure
/// which is used in later search phases to generate *phrases* which
/// will match the given expression.
///
/// The entire input wordlist needs to be evaluated before starting the
/// phrase-matching phase.
#[derive(Debug)]
pub struct WordMatcher<'word> {
    // We want an immutable reference to this, but with it allowed
    // to change out underneath us
    // This is owned by us and populated by us, others take a reference to it
    //
    /// The set of words which are considered "alive" for a given expression;
    /// these are words which *may* be possible to use to build a matching
    /// phrase in later serach phases
    ///
    /// If there are multiple `WordMatchers` in a single `QueryEvaluator`,
    /// typically each `alive_wordlist` is a (weak) subset of the previous,
    /// with the first being a (weak) subset of the input wordlist.
    pub alive_wordlist: Vec<&'word Word>,

    // As each word in the input wordlist is checked in `.next_single_word(...)`,
    // we also populate the `PhraseMatcher` datastructures to generate
    // phrase matches in a later serach phase
    // When the `WordMatcher` is done, use `.into_phrase_matcher()` to convert
    // this `WordMatcher` into a `PhraseMatcher`.
    phrase_matcher: PhraseMatcher,

    // `table_char_src_fuzz_dst` is a 4D bitset: [char_index][from_state][fuzz][to_state]
    // TODO: Should this be done with an actual 4D bitset?
    //
    // This is used as a cache when evaluating words in series:
    // if the next word in the input wordlist shares a common prefix with the
    // previous word (see `table_chars`), then only the *un*-common suffix
    // needs to be evaluated on the `Expression`.
    table_char_src_fuzz_dst: Vec<BitSet3D>,

    // A parallel array to `table_char_src_fuzz_dst`.
    // If `table_chars` has N elements, then the first N elements of
    // `table_char_src_fuzz_dst` are valid, and correspond to the
    // possible state transitions from evaluating the `Char`s in this array.
    table_chars: &'word [Char],

    // Tracks progress through evalutating the input wordlist.
    // Starts at `0`, and goes up to `input_wordlist.len()`.
    // NB: the input wordlist can grow between calls to `.next_single_word(...)`
    word_index: usize,
}

/// Wrapper around `WordMatcher` created from `WordMatcher::iter`.
/// Used to turn the results of `WordMatcher::next_single_word` into an `Iterator`.
pub struct WordMatcherIter<'word, 'it> {
    word_matcher: &'it mut WordMatcher<'word>,
    wordlist: &'it [&'word Word],
    deadline: Option<Instant>,
}

/// State for generating phrases that match the given `Expression`
///
/// This struct is not constructed directly; it is populated by making
/// a `WordMatcher`, feeding it a wordlist, then calling `WordMatcher::into_phrase_matcher()`
#[derive(Debug)]
pub struct PhraseMatcher {
    pub expression: Expression,

    pub fuzz_limit: usize,

    // `states_len` & `start_states` may not match up with the values in
    // `expression`, due to optimizations. For a given input wordlist, we
    // may be able to elide/combine certain NFA states to make the search
    // more efficient.
    pub states_len: usize,
    pub start_states: BitSet1D,

    /// The keys in `classes` are 3D bitsets on: [from_state][fuzz][to_state]
    /// These represent "equivalency classes": words which have equivalent behavior
    /// on the `expression` NFA.
    classes: IndexMap<BitSet3D, WordClass>,

    /// This is a parallel vector to `wordlist`: for each word in the wordlist,
    /// which class does it belong to (by index)
    word_classes: Vec<usize>,
}

/// Inside `PhraseMatcher`, `Word`s are sorted into these equivalency classes.
///
/// These classes group words which have the equivalent behavior on the
/// expression's NFA.
///
/// Ex: For expression `".{5}"`, all 5-letter words would be equivalent
///
/// Class 0 is reserved for the "dead" class - the words which cannot possibly
/// be part of a phrase match because they have an empty dst state set, regardless
/// of starting state or fuzz.
#[derive(Debug, Clone, Default)]
struct WordClass {
    /// Number of words in this class. (If 0, this class can be elided)
    words_count: usize,

    /// Minimum `Tranche` value over all words in this class.
    min_tranche: Tranche,
}

/// See `QueryEvaluator` for a description of the IDDFS search.
/// TODO
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SearchPhase {
    pub depth: PhraseDepth,
    pub tranche: Tranche,
    pub total_size: u64,
}

// ---

impl<'word> WordMatcher<'word> {
    pub fn new(expression: Expression, max_word_len: usize) -> Self {
        let phrase_matcher = PhraseMatcher::new(expression);

        let states_len = phrase_matcher.states_len;
        let fuzz_limit = phrase_matcher.fuzz_limit;
        let empty_table_src_fuzz_dst = BitSet3D::new((states_len, fuzz_limit), states_len);
        let table_char_src_fuzz_dst = vec![empty_table_src_fuzz_dst; max_word_len];

        WordMatcher {
            phrase_matcher,

            word_index: 0,
            alive_wordlist: vec![],
            table_char_src_fuzz_dst,
            table_chars: &[],
        }
    }

    pub fn iter<'it>(
        &'it mut self,
        wordlist: &'it [&'word Word],
        deadline: Option<Instant>,
    ) -> WordMatcherIter<'word, 'it> {
        WordMatcherIter {
            word_matcher: self,
            wordlist,
            deadline,
        }
    }

    pub fn progress(&self, wordlist: &[&'word Word]) -> String {
        format!(
            "Single-word matches: {}/{} ({}%)",
            self.word_index,
            wordlist.len(),
            100 * self.word_index / wordlist.len()
        )
    }

    pub fn expression(&self) -> &Expression {
        &self.phrase_matcher.expression
    }

    pub fn filter_search_phases(&self, search_phases: &mut Vec<SearchPhase>) {
        self.phrase_matcher.filter_search_phases(search_phases)
    }

    /// TODO
    //pub fn is_word_match(&self, word: &'word Word) -> bool {
    //    false
    //}

    /// Find the next word in `wordlist` that matches the target expression, or `None` if the
    /// `deadline` is exceeded.
    ///
    /// Between calls, `wordlist` cannot be reordered or shrink (but it can be extended)
    /// While iterating, also compute the state needed by PhraseMatcher
    pub fn next_single_word(
        &mut self,
        wordlist: &[&'word Word],
        deadline: Option<Instant>,
    ) -> Option<&'word Word> {
        let mut deadline_check_count = 0;
        let states_len = self.phrase_matcher.states_len;
        let fuzz_range = 0..self.phrase_matcher.fuzz_limit;

        // Iterate through the words we have not yet processed
        while self.word_index < wordlist.len() {
            // If we've exceeded the deadline, return `None` for now
            // (But only check the clock a small fraction of the time)
            if deadline_check_count % 256 == 0
                && deadline.is_some()
                && Some(Instant::now()) > deadline
            {
                return None;
            }
            deadline_check_count += 1;

            let word = &wordlist[self.word_index];
            self.word_index += 1;

            // Find the common prefix with the last word we processed
            let word_len = word.chars.len();
            let mut prefix_len: usize = 0;
            while prefix_len < word_len
                && prefix_len < self.table_chars.len()
                && word.chars[prefix_len] == self.table_chars[prefix_len]
            {
                prefix_len += 1;
            }

            // Populate the transition table for the new word, re-using the previous values for the
            // common prefix
            // (Here, "prefixed" refers to the *uncommon suffix*)
            let prefixed_chars = &word.chars[prefix_len..];
            let prefixed_table = &mut self.table_char_src_fuzz_dst[prefix_len..];

            // If there is no common prefix, clear the table & populate with initial state
            if prefix_len == 0 {
                prefixed_table[0].borrow_mut().clear();
                for src in 0..states_len {
                    // After consuming 0 chars (and 0 fuzz), the only states reachable from state
                    // `src` are its epsilon transitions
                    prefixed_table[0]
                        .slice_mut((src, 0))
                        .union_with(self.phrase_matcher.expression.epsilon_states(src));
                }
            }

            // Fill the table, but this can return early if the chars are not a match
            // `partial_len` refers to how many chars are at least a partial match
            let partial_len = prefix_len
                + self
                    .phrase_matcher
                    .expression
                    .fill_transition_table(prefixed_chars, prefixed_table);

            self.table_chars = &word.chars[0..partial_len];
            if partial_len < word_len {
                continue;
            }
            assert_eq!(partial_len, word_len);

            // The word is "alive", it may be useful as part of a phrase match
            self.alive_wordlist.push(word);
            let word_table_src_fuzz_dst = &self.table_char_src_fuzz_dst[word_len];
            self.phrase_matcher
                .insert_word_table(word, &word_table_src_fuzz_dst);

            // Check if the word is a match on its own (within the fuzz limit)
            let success_state = states_len - 1;
            for f in fuzz_range.clone() {
                if word_table_src_fuzz_dst
                    .slice((0, f))
                    .contains(success_state)
                {
                    return Some(word);
                }
            }
        }

        None
    }

    pub fn optimize_for_wordlist(
        &mut self,
        new_input_wordlist: &[&'word Word],
        search_queue: &Vec<SearchPhase>,
    ) -> bool {
        // Filter down `alive_wordlist` to exactly match `new_input_wordlist`.
        //
        // `new_input_wordlist` must be a (weak) subset of our `alive_wordlist`
        // If they are not already equal, filter out the missing words
        {
            assert!(self.alive_wordlist.len() >= new_input_wordlist.len());

            // Reset the classes internal metadata (e.g. # of words)
            self.phrase_matcher
                .classes
                .values_mut()
                .for_each(|wc| wc.clear());

            let mut new_word_classes = vec![];
            let mut new_alive_wordlist = vec![];
            let mut i: usize = 0;
            for (&word, &class_index) in self
                .alive_wordlist
                .iter()
                .zip(self.phrase_matcher.word_classes.iter())
            {
                if i < new_input_wordlist.len() && *word == *new_input_wordlist[i] {
                    if class_index != 0 {
                        new_word_classes.push(class_index);
                        new_alive_wordlist.push(word);
                        self.phrase_matcher
                            .classes
                            .get_index_mut(class_index)
                            .unwrap()
                            .1
                            .add_word(word);
                    }
                    i += 1;
                }
            }
            assert_eq!(new_word_classes.len(), new_alive_wordlist.len());
            assert_eq!(i, new_input_wordlist.len());
            assert!(new_alive_wordlist.len() <= new_input_wordlist.len());
            self.phrase_matcher.word_classes = new_word_classes;
            self.alive_wordlist = new_alive_wordlist;
        }

        let states_len = self.phrase_matcher.states_len;
        let fuzz_limit = self.phrase_matcher.fuzz_limit;

        // With the present search_queue, determine which depths are actually possible
        assert!(!search_queue.is_empty());
        // `valid_search_depths` acts like a bitset: `[i]` is `true` if depth `i` is possible
        let valid_search_depths: Vec<bool> = {
            let mut depths = vec![false; search_queue.iter().map(|p| p.depth).max().unwrap() + 1];
            search_queue.iter().for_each(|p| depths[p.depth] = true);
            depths
        };

        // Compute the set of states which are reachable from the starting state, only using words
        // that are in the alive wordset
        let reachable_srcs = {
            // Begin with just the starting states (at fuzz 0)
            let mut reachable_fuzz_dst = BitSet2D::new(fuzz_limit, states_len);
            reachable_fuzz_dst
                .slice_mut(0)
                .union_with(self.phrase_matcher.start_states.borrow());

            // Iterate, expanding the `reachable_fuzz_dst` set until it stabilizes, or the limit is
            // reached.
            for (_, _valid) in valid_search_depths[1..].iter().enumerate() {
                let mut next_reachable_fuzz_dst = reachable_fuzz_dst.clone();
                for (table_src_fuzz_dst, word_class) in self.phrase_matcher.classes.iter() {
                    if word_class.words_count == 0 {
                        continue;
                    }
                    self.phrase_matcher.step(
                        table_src_fuzz_dst,
                        reachable_fuzz_dst.borrow(),
                        next_reachable_fuzz_dst.borrow_mut(),
                    );
                }

                if next_reachable_fuzz_dst == reachable_fuzz_dst {
                    break;
                }
                reachable_fuzz_dst = next_reachable_fuzz_dst;
            }

            // Take the union over all values of fuzz
            let mut reachable_srcs = BitSet1D::new((), states_len);
            for f in 0..fuzz_limit {
                reachable_srcs
                    .borrow_mut()
                    .union_with(reachable_fuzz_dst.slice(f));
            }
            reachable_srcs
        };

        // Compute the set of states which _cannot_ reach the success state, only using words that
        // are in the alive wordset
        let candidate_srcs = {
            let mut candidate_srcs = BitSet1D::new((), states_len);
            for src in 0..states_len {
                // Start from state `src` with fuzz 0 (not including any epsilon transitions, though!)
                let mut table_fuzz_dst = BitSet2D::new(fuzz_limit, states_len);
                table_fuzz_dst.slice_mut(0).insert(src);

                for (_, _valid) in valid_search_depths.iter().enumerate() {
                    // Check if the success state is reachable, if so mark `src` as a candidate
                    let success_state = states_len - 1;
                    for f in 0..fuzz_limit {
                        if table_fuzz_dst.slice(f).contains(success_state) {
                            candidate_srcs.borrow_mut().insert(src);
                            break;
                        }
                    }

                    // Populate `next_table_fuzz_dst` with the results of stepping one more word
                    let mut next_table_fuzz_dst = table_fuzz_dst.clone();
                    for (table_src_fuzz_dst, word_class) in self.phrase_matcher.classes.iter() {
                        if word_class.words_count == 0 {
                            continue;
                        }
                        self.phrase_matcher.step(
                            table_src_fuzz_dst,
                            table_fuzz_dst.borrow(),
                            next_table_fuzz_dst.borrow_mut(),
                        );
                    }

                    // If we've saturated the table, it's not possible to reach the success state
                    if next_table_fuzz_dst == table_fuzz_dst {
                        break;
                    }
                    table_fuzz_dst = next_table_fuzz_dst;
                }
            }
            candidate_srcs
        };

        // Compute the intersection of the reachable states & the candidate states
        // This is the set of states which are *both* visible from the initial state,
        // *and* can be used to reach the success state, with the given wordlist
        // TODO: Compute alive_states per tranche
        let alive_states = {
            let mut states = reachable_srcs.clone();
            states.borrow_mut().intersect_with(candidate_srcs.borrow());

            states
        };
        let alive_states_ref = alive_states.borrow();

        // If the success state isn't reachable, then bail early
        if !alive_states_ref.contains(states_len - 1) {
            self.alive_wordlist = vec![];
            return true;
        }

        // Identify redundant states, which are alive states that are equivalent
        // within the given wordlist.
        //
        // The element at index `i` is `None` if the state is not redundant,
        // and `Some(j)` if the state `i` is redundant to state `j`.
        // If a state `i` is not alive, then it has value `Some(i)`.
        let redundant_states = {
            let mut redundant_states: Vec<Option<usize>> = (0..states_len)
                .map(|src| {
                    if alive_states_ref.contains(src) {
                        None
                    } else {
                        Some(src)
                    }
                })
                .collect();

            // Look for `(i, j)` pairs of states which are redundant to each other
            for i in 0..states_len {
                if redundant_states[i].is_some() {
                    continue;
                }

                #[allow(clippy::needless_range_loop)]
                'outer: for j in i + 1..states_len - 1 {
                    if redundant_states[j].is_some() {
                        continue;
                    }

                    // Check that the inputs & outputs are the same for the two states,
                    // for all fuzz values. Ignore states & word classes that have already
                    // been elminiated.
                    for f in 0..fuzz_limit {
                        for (table_src_fuzz_dst, word_class) in self.phrase_matcher.classes.iter() {
                            if word_class.words_count == 0 {
                                continue;
                            }

                            // Outputs same
                            for dst in alive_states_ref.ones() {
                                let slice_i = table_src_fuzz_dst.slice((i, f));
                                let slice_j = table_src_fuzz_dst.slice((j, f));

                                if slice_i.contains(dst) != slice_j.contains(dst) {
                                    continue 'outer;
                                }
                            }

                            // Inputs same
                            for src in alive_states_ref.ones() {
                                let slice = table_src_fuzz_dst.slice((src, f));
                                if slice.contains(i) != slice.contains(j) {
                                    continue 'outer;
                                }
                            }
                        }
                    }

                    // At this point, states `(i, j)` are redundant, and `i < j`
                    assert_eq!(redundant_states[j], None);
                    redundant_states[j] = Some(i);
                }
            }

            // We can't elide the final state
            assert_eq!(redundant_states[states_len - 1], None);

            redundant_states
        };

        // Now that we've computed the set of redundant states, remove them
        let new_states_len = redundant_states.iter().filter(|x| x.is_none()).count();

        // There were no redundant states! Already optimized, nothing to remove
        if new_states_len == states_len {
            return false;
        }

        let new_state_index: Vec<_> = redundant_states
            .iter()
            .enumerate()
            .map(|(i, x)| match x {
                Some(_) => None,
                None => Some(redundant_states[..i].iter().filter(|y| y.is_none()).count()),
            })
            .collect();

        let new_start_states = {
            let mut states = BitSet1D::new((), new_states_len);
            for i in self.phrase_matcher.start_states.borrow().ones() {
                if let Some(new_i) = new_state_index[i] {
                    states.borrow_mut().insert(new_i);
                }
            }
            states
        };

        let empty_table_src_fuzz_dst = BitSet3D::new((new_states_len, fuzz_limit), new_states_len);

        let mut class_map: Vec<usize> = vec![0; self.phrase_matcher.classes.len()];
        let mut new_classes: IndexMap<_, WordClass> = IndexMap::new();
        new_classes.insert(empty_table_src_fuzz_dst.clone(), Default::default());

        for (class_index, (table_src_fuzz_dst, word_class)) in
            self.phrase_matcher.classes.iter().enumerate()
        {
            if word_class.words_count == 0 {
                continue;
            }
            let mut new_table_src_fuzz_dst = empty_table_src_fuzz_dst.clone();
            for (src, &new_src) in new_state_index.iter().enumerate() {
                // If it's deleted, we don't need to remap anything
                if redundant_states[src] == Some(src) {
                    continue;
                }

                let new_src = new_src
                    .unwrap_or_else(|| new_state_index[redundant_states[src].unwrap()].unwrap());
                for f in 0..fuzz_limit {
                    let old_slice = table_src_fuzz_dst.slice((src, f));
                    let mut new_slice = new_table_src_fuzz_dst.slice_mut((new_src, f));

                    for dst in old_slice.ones() {
                        if let Some(new_dst) = new_state_index[dst] {
                            new_slice.insert(new_dst);
                        }
                    }
                }
            }
            let entry = new_classes.entry(new_table_src_fuzz_dst);
            class_map[class_index] = entry.index();
            entry.or_insert_with(Default::default);
        }
        assert_eq!(
            self.phrase_matcher.word_classes.len(),
            self.alive_wordlist.len()
        );

        let mut new_alive_wordlist = vec![];
        let mut new_word_classes = vec![];
        for (&word, &old_class_index) in self
            .alive_wordlist
            .iter()
            .zip(self.phrase_matcher.word_classes.iter())
        {
            let new_class_index = class_map[old_class_index];
            if new_class_index != 0 {
                new_alive_wordlist.push(word);
                new_word_classes.push(new_class_index);
                new_classes
                    .get_index_mut(new_class_index)
                    .unwrap()
                    .1
                    .add_word(word);
            }
        }
        assert_eq!(new_alive_wordlist.len(), new_word_classes.len());
        self.alive_wordlist = new_alive_wordlist;
        self.phrase_matcher.classes = new_classes;
        self.phrase_matcher.word_classes = new_word_classes;
        self.phrase_matcher.states_len = new_states_len;
        self.phrase_matcher.start_states = new_start_states;

        true
    }

    pub fn into_phrase_matcher(self) -> Option<PhraseMatcher> {
        // This returns an `Option` so that future optimizations
        // could decide to elide this `PhraseMatcher` if all possible
        // inputs would successfully match
        Some(self.phrase_matcher)
    }
}
impl<'word> Iterator for WordMatcherIter<'word, '_> {
    type Item = &'word Word;

    fn next(&mut self) -> Option<&'word Word> {
        self.word_matcher
            .next_single_word(self.wordlist, self.deadline)
    }
}

impl PhraseMatcher {
    pub fn new(expression: Expression) -> Self {
        let states_len = expression.states_len();
        let fuzz_limit = expression.fuzz + 1;

        let empty_table_src_fuzz_dst = BitSet3D::new((states_len, fuzz_limit), states_len);
        let mut classes = IndexMap::new();
        classes.insert(empty_table_src_fuzz_dst, Default::default());

        let start_states = expression.epsilon_states(0).to_bitset();

        PhraseMatcher {
            expression,
            states_len,
            fuzz_limit,
            start_states,
            classes,
            word_classes: vec![],
        }
    }

    pub fn step_by_word_index(
        &self,
        word_index: usize,
        prev_fuzz_dst: BitSetRef2D,
        next_fuzz_dst: BitSetRefMut2D,
    ) {
        // This assert isn't stictly required; but class 0 denotes "empty" words,
        // so if there are still empty words hanging around in the alive_wordlist,
        // then there were some missed optimizations.
        assert!(self.word_classes[word_index] != 0);

        let word_table = self
            .classes
            .get_index(self.word_classes[word_index])
            .unwrap()
            .0;

        self.step(word_table, prev_fuzz_dst, next_fuzz_dst);
    }

    pub fn has_success_state(&self, table_fuzz_dst: BitSetRef2D) -> bool {
        for f in 0..self.fuzz_limit {
            if table_fuzz_dst.slice(f).contains(self.states_len - 1) {
                return true;
            }
        }
        false
    }

    fn step(
        &self,
        table_src_fuzz_dst: &BitSet3D,
        prev_fuzz_dst: BitSetRef2D,
        mut next_fuzz_dst: BitSetRefMut2D,
    ) {
        for f in 0..self.fuzz_limit {
            for dst in prev_fuzz_dst.slice(f).ones() {
                let mut df = 0;
                while f + df < self.fuzz_limit {
                    next_fuzz_dst
                        .slice_mut(f + df)
                        .union_with(table_src_fuzz_dst.slice((dst, df)));
                    df += 1;
                }
            }
        }
    }

    pub fn filter_search_phases(&self, search_phases: &mut Vec<SearchPhase>) {
        if search_phases.is_empty() {
            return;
        }
        let max_depth = search_phases.iter().map(|p| p.depth).max().unwrap();

        let tranches = {
            let mut ts: Vec<_> = search_phases.iter().map(|p| p.tranche).collect();
            ts.sort();
            ts.dedup();
            ts
        };
        let tranche_map: Vec<_> = {
            let mut map = vec![None; *tranches.iter().max().unwrap() as usize + 1];
            tranches.iter().enumerate().for_each(|(i, &t)| {
                let mut t = t as usize;
                while map[t].is_none() {
                    map[t] = Some(i);
                    if t == 0 {
                        break;
                    }
                    t -= 1;
                }
            });
            map
        };
        let tranches_len = tranches.len();

        let mut states_fuzz_dst = BitSet3D::new((tranches.len(), self.fuzz_limit), self.states_len);
        for t in 0..tranches_len {
            states_fuzz_dst
                .slice_mut((t, 0))
                .union_with(self.start_states.borrow());
        }

        for w in 1..=max_depth {
            let mut next_states_fuzz_dst =
                BitSet3D::new((tranches_len, self.fuzz_limit), self.states_len);
            for (table_src_fuzz_dst, word_class) in self.classes.iter() {
                for t in tranche_map[word_class.min_tranche as usize].unwrap()..tranches_len {
                    self.step(
                        table_src_fuzz_dst,
                        states_fuzz_dst.slice2d(t),
                        next_states_fuzz_dst.slice2d_mut(t),
                    );
                }
            }

            for (t, &tranche) in tranches.iter().enumerate() {
                //if next_states_fuzz_dst.slice2d(t).is_empty() {
                //    search_phases.retain(|p| p.depth < w || p.tranche != tranche);
                //    break;
                //}
                if !self.has_success_state(next_states_fuzz_dst.slice2d(t)) {
                    search_phases.retain(|p| p.depth != w || p.tranche != tranche);
                }
            }
            states_fuzz_dst = next_states_fuzz_dst;
        }
    }

    fn insert_word_table(&mut self, word: &Word, table_src_fuzz_dst: &BitSet3D) {
        use indexmap::map::Entry;
        let entry = self.classes.entry(table_src_fuzz_dst.clone());
        self.word_classes.push(entry.index());
        match entry {
            Entry::Occupied(mut entry) => {
                entry.get_mut().add_word(word);
            }
            Entry::Vacant(entry) => {
                let mut class = WordClass::default();
                class.add_word(word);

                entry.insert(class);
            }
        };
    }
}

impl WordClass {
    fn add_word(&mut self, word: &Word) {
        self.words_count += 1;
        self.min_tranche = self.min_tranche.min(word.tranche);
    }

    fn clear(&mut self) {
        *self = Default::default();
    }
}
