use crate::bitset::{BitSet1D, BitSet3D, BitSetRef1D, BitSetRefMut1D};
use crate::parser;
use crate::words::{Char, CharBitset};
use std::fmt;

// This is only used while constructing the `Expression`,
// the sets are resized before they are evaluated.
const MAX_SET_SIZE: usize = 16 * 1024;

#[derive(Debug, Clone)]
struct State {
    epsilon_states: BitSet1D,

    char_bitset: CharBitset,
    next_state: usize,
}

impl State {
    fn new() -> Self {
        Self::new_transition(CharBitset::EMPTY, 0)
    }

    fn new_transition(char_bitset: CharBitset, next_state: usize) -> Self {
        Self {
            epsilon_states: BitSet1D::new((), MAX_SET_SIZE),
            char_bitset,
            next_state,
        }
    }

    fn epsilon_states_bitset(&self) -> BitSetRef1D<'_> {
        self.epsilon_states.slice(())
    }

    fn epsilon_states_bitset_mut(&mut self) -> BitSetRefMut1D<'_> {
        self.epsilon_states.slice_mut(())
    }
}

/// Representation of a low-level Noodle Expression
pub struct Expression {
    states: Vec<State>,
    text: String,

    pub ignore_word_boundaries: bool,
    pub ignore_punctuation: bool,
    pub fuzz: usize,
}

impl Expression {
    /// Compile an `Expression` from its string representation
    pub fn new(text: &str) -> parser::Result<Self> {
        let ast_root = parser::ExpressionAst::new_from_str(text)?;
        Ok(Self::from_ast(&ast_root))
    }

    pub fn from_ast(ast_root: &parser::ExpressionAst) -> Self {
        let ignore_word_boundaries = !ast_root.options.explicit_word_boundaries.unwrap_or(false);
        let ignore_punctuation = !ast_root.options.explicit_punctuation.unwrap_or(false);

        //println!("Ast: {:#?}", ast_root);

        let mut states = vec![];
        Self::build_states(&ast_root.root, &mut states);
        // Add a "success" end state (this may not be needed?) that absorbs word boundaries
        states.push(State::new_transition(Char::WORD_END.into(), states.len()));

        let mut expr = Expression {
            states,
            text: format!("{}", ast_root),

            ignore_word_boundaries,
            ignore_punctuation,
            fuzz: ast_root.options.fuzz.unwrap_or(0),
        };
        //println!("Pre-opt: {:?}", expr);
        Self::optimize_states(&mut expr.states);
        //println!("Post-opt: {:?}", expr);

        expr
    }

    pub fn states_len(&self) -> usize {
        self.states.len()
    }

    /// Extend `states` with the NFA representation of the `ast`
    /// The first new state is the "start" state, and the last added
    /// state must have a "success" transition to the next state.
    fn build_states(ast: &parser::Ast, states: &mut Vec<State>) {
        let initial_len = states.len();
        match ast {
            parser::Ast::CharClass(char_bitset) => {
                states.push(State::new_transition(*char_bitset, initial_len + 1))
            }
            parser::Ast::Alternatives(alts) => {
                states.push(State::new());
                let mut end_indexes = vec![];
                for alt in alts {
                    let next_index = states.len();
                    states[initial_len]
                        .epsilon_states_bitset_mut()
                        .insert(next_index);
                    Self::build_states(alt, states);
                    end_indexes.push(states.len() - 1);
                }
                let next_index = states.len();
                for end_index in end_indexes {
                    // Repoint the terminal states to the true success state
                    let mut epsilon_states = states[end_index].epsilon_states_bitset_mut();
                    if epsilon_states.contains(end_index + 1) {
                        epsilon_states.remove(end_index + 1);
                        epsilon_states.insert(next_index);
                    }
                    states[end_index].next_state = next_index;
                }
            }
            parser::Ast::Sequence(terms) => {
                for term in terms {
                    Self::build_states(term, states);
                }
            }
            parser::Ast::Repetition {
                term: _,
                min: 0,
                max: Some(0),
            } => {}
            parser::Ast::Repetition {
                term,
                min: 1,
                max: Some(1),
            } => {
                Self::build_states(term, states);
            }
            parser::Ast::Repetition { term, min, max } => {
                states.push(State::new());
                let repeats = std::cmp::max(1, std::cmp::max(*min, max.unwrap_or(*min)));
                let mut final_term_index = 0;
                for i in 0..repeats {
                    if i <= max.unwrap_or(*min) - *min {
                        let next_index = states.len();
                        states[initial_len]
                            .epsilon_states_bitset_mut()
                            .insert(next_index);
                    }
                    final_term_index = states.len();
                    Self::build_states(term, states);
                }
                let final_index = states.len();
                states.push(State::new());
                states[final_index]
                    .epsilon_states_bitset_mut()
                    .insert(final_index + 1);
                if *min == 0 {
                    states[initial_len]
                        .epsilon_states_bitset_mut()
                        .insert(final_index);
                }
                if *max == None {
                    states[final_index]
                        .epsilon_states_bitset_mut()
                        .insert(final_term_index);
                }
            }
            parser::Ast::Anagram { kind: _, bank: _ } => unreachable!(),
        }
    }

    /// Perform an optimization pass on the NFA, modifying `states` so
    /// that it produces an equivalent NFA that can be evaluated more efficiently
    ///
    /// `RUNTIME: O(states^4)`
    fn optimize_states(states: &mut Vec<State>) {
        // TODO: This function could perform even more complex optimizations
        //      In general, the optimizations that *are* implemented are the low-hanging fruit
        //      for the NFAs that `build_states` spits out, rather than being "general-purpose".
        //      This keeps the `build_states` code simpler, and allows the optimizations to be
        //      maybe be applied more broadly.
        // TODO: Eliding start state
        //      If state[0].epsilon_states = {1 | state[1].epsilon_states}, and char_bitset = 0
        //      then it can be elided & state[1] can be the new start state

        // `Matcher` requires that there is a transitive closure over `epsilon_states` and that
        // each state has itself included in that set
        // RUNTIME: O(states^4)
        fn epsilon_transitive_closure(states: &mut Vec<State>) {
            let states_len = states.len();
            for i in 0..states_len {
                // Add an epsilon transition from each state to itself
                states[i].epsilon_states.borrow_mut().insert(i);

                // Calculate transitive closure over `epsilon_states`
                // RUNTIME: O(states^3)
                loop {
                    let mut ss: BitSet1D = states[i].epsilon_states.clone();
                    let mut bs = ss.borrow_mut();

                    // RUNTIME: O(states^2)
                    for (i2, state2) in states.iter().enumerate() {
                        if bs.contains(i2) {
                            bs.union_with(state2.epsilon_states_bitset());
                        }
                    }
                    if states[i].epsilon_states_bitset() == bs.reborrow() {
                        break;
                    }
                    states[i].epsilon_states = ss;
                }
            }
        }

        // Combine pairs of states, an "epsilon" state and a "char" state:
        //  - The "epsilon" state has only epsilon transitions
        //  - The "char" state has only a char_bitset transition
        //  - The epsilon state can transition to the char state AND char state's next_state
        //  - The epsilon state's epsilon transitions only contains:
        //      - Itself
        //      - The char state
        //      - The epsilon_states of the char state's next state
        // This could be more generic if states supported an arbitrary set of edges
        // TODO: this code is pretty inefficient
        // RUNTIME: O(states^3)?
        fn collapse_epsilon_char_pairs(states: &mut Vec<State>) {
            // RUNTIME: O(states)
            fn bitset_only_contains(bitset: BitSetRef1D, index: usize) -> bool {
                bitset.is_empty() || (bitset.contains(index) && bitset.ones().count() == 1)
            }

            // RUNTIME: O(states)
            #[allow(clippy::nonminimal_bool)]
            fn is_a_pair(eps_index: usize, char_index: usize, states: &[State]) -> bool {
                let eps_state = &states[eps_index];
                let char_state = &states[char_index];

                let mut allowed_epsilons = char_state.epsilon_states.clone();
                allowed_epsilons.borrow_mut().insert(eps_index);
                allowed_epsilons
                    .borrow_mut()
                    .union_with(states[char_state.next_state].epsilon_states.borrow());

                true
                    // We can't delete the first state
                    && char_index != 0
                    // eps_state must be pure-epsilon
                    && eps_state.char_bitset == CharBitset::EMPTY
                    // char_state must be pure-char (epsilon to itself is allowed)
                    && char_state.next_state != eps_index
                    && char_state.next_state != char_index
                    && bitset_only_contains(char_state.epsilon_states.borrow(), char_index)
                    // eps_state must point to char_state, ...
                    && eps_state.epsilon_states.borrow().contains(char_index)
                    // ...char_state's next_state, ...
                    && eps_state
                        .epsilon_states
                        .borrow()
                        .contains(char_state.next_state)
                    // ...but point to at most: itself, char_state, and char_state.next_state's epsilons
                    && eps_state.epsilon_states.borrow().is_subset(&allowed_epsilons.borrow())
            }

            // RUNTIME: O(states^2)
            fn collapse(eps_index: usize, char_index: usize, states: &mut Vec<State>) {
                assert!(eps_index != char_index);

                states[eps_index].char_bitset = states[char_index].char_bitset;
                states[eps_index].next_state = states[char_index].next_state;

                let remap = |i: usize| match i.cmp(&char_index) {
                    std::cmp::Ordering::Less => i,
                    std::cmp::Ordering::Equal if eps_index < char_index => eps_index,
                    std::cmp::Ordering::Equal if eps_index > char_index => eps_index - 1,
                    std::cmp::Ordering::Greater => i - 1,
                    _ => unreachable!(),
                };

                for state in states.iter_mut() {
                    state.next_state = remap(state.next_state);

                    let mut new_epsilon_states = state.epsilon_states.clone();
                    let mut new_epsilon_states_ref = new_epsilon_states.borrow_mut();
                    new_epsilon_states_ref.clear();

                    for i in state.epsilon_states.borrow().ones() {
                        new_epsilon_states_ref.insert(remap(i));
                    }

                    state.epsilon_states = new_epsilon_states;
                }

                states.remove(char_index);
            }

            // TODO: This loop indexing is sketchy, could probably be both optimized and rust-ified
            // RUNTIME: O(states^3)?
            let mut i = 0;
            'outer: while i < states.len() {
                for j in 0..states.len() {
                    if is_a_pair(i, j, states) {
                        collapse(i, j, states);
                        continue 'outer;
                    } else if is_a_pair(j, i, states) {
                        collapse(j, i, states);
                        continue 'outer;
                    }
                }
                i += 1;
            }
        }

        // Identify redundant states, and prune them
        // RUNTIME: O(states^3)
        fn prune_epsilon_pairs(states: &mut Vec<State>) {
            let states_len = states.len();
            let mut state_map: Vec<_> = (0..states_len).collect();
            let mut state_map_from: Vec<Option<usize>> = (0..states_len).map(Some).collect();
            let mut deleted: Vec<usize> = vec![];
            for (i, state) in states.iter().enumerate() {
                // Shift the state index down by the number of states before it that were deleted
                state_map[i] -= deleted.iter().filter(|&d| *d < i).count();

                // Don't attempt to collapse the first state
                // Removing it would add a lot of complexity, with little benefit
                if i == 0 {
                    continue;
                }

                // Only pure-epsilon states can be removed
                if state.char_bitset != CharBitset::EMPTY {
                    continue;
                }

                // Look for any states that are equivalent to `state`,
                // (except for containing the transition from itself)
                // RUNTIME: O(states^2)
                let mut similar_states = state.epsilon_states.clone();
                similar_states.borrow_mut().remove(i);
                for (i2, state2) in states.iter().enumerate() {
                    // We're doing a "triangle search", and only comparing `states[i] ~ states[i2]` where `i > i2`
                    if i2 <= i {
                        continue;
                    }
                    if similar_states == state2.epsilon_states {
                        deleted.push(i2);
                        state_map[i2] = i;
                        state_map_from[i2] = None;
                        state_map_from.iter_mut().for_each(|s| {
                            if *s == Some(i) {
                                *s = Some(i2)
                            }
                        });
                        break;
                    }
                }
            }

            // Re-create the state table, pruning the `deleted` states
            // - `state_map_from[i] == None` iff `i` is in `deleted` (and the state is pruned)
            // - Otherwise, replace the state at `i` with the state at index `state_map_from[i]`
            // - Everywhere, change references to state `i` to `state_map[i]` (e.g. `next_state`, `epsilon_states`)
            // RUNTIME: O(states^2)
            let new_states_len = states_len - deleted.len();
            state_map.iter().for_each(|&s| assert!(s < new_states_len));
            *states = state_map_from
                .iter()
                .filter_map(|&from| {
                    from.map(|from_index| {
                        let state = &states[from_index];
                        let mut epsilon_states = BitSet1D::new((), new_states_len);
                        for s in state.epsilon_states.borrow().ones() {
                            epsilon_states.borrow_mut().insert(state_map[s])
                        }
                        State {
                            char_bitset: state.char_bitset,
                            next_state: state_map[state.next_state],
                            epsilon_states,
                        }
                    })
                })
                .collect();
        }

        // Shrink the `epsilon_states` set to exactly fit the total number of states, so that it
        // can be easily manipulated by `Matcher`'s `BitSet`s
        // RUNTIME: O(states^2)
        fn shrink_bitsets(states: &mut Vec<State>) {
            let states_len = states.len();
            for state in states.iter_mut() {
                state.epsilon_states = state.epsilon_states.borrow().resize(states_len);
            }
        }

        // RUNTIME: O(states^4)? (May be able to set a tighter bound on loop_count?)
        let mut loop_count = 0;
        loop {
            // RUNTIME: O(states^3)
            let states_len = states.len();
            epsilon_transitive_closure(states);
            prune_epsilon_pairs(states);
            collapse_epsilon_char_pairs(states);
            if states.len() == states_len {
                break;
            }

            shrink_bitsets(states);
            loop_count += 1;
        }
        if loop_count > 2 {
            println!("warning: optimize_states required {} loops", loop_count);
        }
    }

    /// Return the set of states reachable via epsilon transition(s) from the given state
    pub fn epsilon_states(&self, state_index: usize) -> BitSetRef1D<'_> {
        self.states[state_index].epsilon_states_bitset()
    }

    /// Populate a state transition table for a given word
    ///
    /// The transition table has dimensions: `[char][from_state][fuzz][to_state]`,
    /// with the given bit set if the slice `word[..char]` can transition starting
    /// from `from_state` to `to_state` with at most `fuzz` fuzz
    ///
    /// If `[c][from][fuzz][to]` is set, `[c][from][fuzz+1][to]` is *always unset*,
    /// to optimize future searches.
    ///
    /// This function does not initialize the transition table. When starting a word,
    /// the transition_table[char=0] should be initialized to only contain the starting state:
    /// The only bits that should be set are `[c][0][0][e]` where `e` is an epsilon transition
    /// from the starting state. (see `Expression::epsilon_states(0)`)
    ///
    /// `RUNTIME: O(chars * fuzz * states^3)`
    pub fn fill_transition_table(
        &self,
        chars: &[Char],
        transition_table: &mut [BitSet3D],
    ) -> usize {
        debug_assert!(transition_table.len() > chars.len());

        // RUNTIME: O(chars * fuzz * states^3)
        for (char_index, &chr) in chars.iter().enumerate() {
            let char_bitset = CharBitset::from(chr);
            let mut all_states_are_empty = true;

            let (lower_table, upper_table) = transition_table.split_at_mut(char_index + 1);

            if (self.ignore_word_boundaries && chr == Char::WORD_END)
                || (self.ignore_punctuation && chr == Char::PUNCTUATION)
            {
                upper_table[0]
                    .borrow_mut()
                    .copy_from(lower_table[char_index].borrow());
                continue;
            }

            // Consume 1 character from the buffer and compute the set of possible resulting states
            // RUNTIME: O(fuzz * states^3)
            for state_index in 0..self.states.len() {
                let mut all_fuzz_are_empty = true;
                // RUNTIME: O(fuzz * states^2)
                for fuzz_index in 0..=self.fuzz {
                    let state_transitions =
                        lower_table[char_index].slice((state_index, fuzz_index));
                    let mut next_state_transitions =
                        upper_table[0].slice_mut((state_index, fuzz_index));

                    // RUNTIME: O(states^2)
                    if state_transitions.is_empty() {
                        next_state_transitions.clear();
                    } else {
                        next_state_transitions.copy_from(
                            self.char_transitions(char_bitset, state_transitions)
                                .slice(()),
                        );
                        all_fuzz_are_empty = false;
                    }
                }
                if all_fuzz_are_empty {
                    continue;
                }
                all_states_are_empty = false;

                // For a fuzzy match, expand `next_state_table[fi+1]` by adding all states
                // reachable from `state_table[f]` *but* with a 1-character change to `chars`
                // RUNTIME: O(fuzz * states^2)
                let mut fuzz_superset = BitSet1D::new((), self.states.len());
                for fuzz_index in 0..self.fuzz {
                    let state_transitions =
                        lower_table[char_index].slice((state_index, fuzz_index));
                    let mut fuzzed_next_state_transitions =
                        upper_table[0].slice_mut((state_index, fuzz_index + 1));

                    if state_transitions.is_empty() {
                        continue;
                    }

                    // Deletion
                    fuzzed_next_state_transitions.union_with(state_transitions);

                    // Change
                    let change_set_group =
                        self.char_transitions(CharBitset::LETTERS, state_transitions);
                    let change_set = change_set_group.slice(());
                    fuzzed_next_state_transitions.union_with(change_set);

                    // Insertion
                    let insertion_set_group = self.char_transitions(char_bitset, change_set);
                    let insertion_set = insertion_set_group.slice(());
                    fuzzed_next_state_transitions.union_with(insertion_set);

                    // Optimization: discard the states we can get to with less fuzz
                    fuzzed_next_state_transitions.difference_with(fuzz_superset.borrow());
                    fuzz_superset
                        .borrow_mut()
                        .union_with(fuzzed_next_state_transitions.reborrow());
                }
            }
            if all_states_are_empty {
                return char_index;
            }
        }
        chars.len()
    }

    /// Given a set of starting states `start_states`, calculate the set of states reachable by
    /// consuming exactly one character from `char_bitset` (followed by epsilon transition(s))
    ///
    /// `RUNTIME: O(states^2)`
    fn char_transitions<'a>(
        &'a self,
        char_bitset: CharBitset,
        start_states: BitSetRef1D<'a>,
    ) -> BitSet1D {
        let mut result_bitset = BitSet1D::new((), self.states.len());
        let mut end_states = result_bitset.slice_mut(());

        if !start_states.is_empty() {
            // RUNTIME: O(states^2)
            for (si, state) in self.states.iter().enumerate() {
                // RUNTIME: O(states)
                if !start_states.contains(si) {
                    continue;
                }

                if char_bitset.is_intersecting(state.char_bitset) {
                    end_states.union_with(self.epsilon_states(state.next_state));
                }
            }
        }

        result_bitset
    }
}

impl fmt::Debug for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Expression: \"{}\"", self.text)?;
        for (i, state) in self.states.iter().enumerate() {
            // Omit self-state
            let mut epsilon_states = state.epsilon_states.clone();
            epsilon_states.borrow_mut().remove(i);
            write!(f, "    {}: ", i)?;
            if state.char_bitset != CharBitset::EMPTY {
                write!(f, "{:?} -> [{}]; ", state.char_bitset, state.next_state)?;
            }
            if !epsilon_states.borrow().is_empty() {
                write!(f, "* -> {}; ", epsilon_states)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
