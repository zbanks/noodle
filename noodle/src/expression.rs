use crate::bitset::{BitSet1D, BitSet3D, BitSetRef1D, BitSetRefMut1D};
use crate::parser;
use crate::words::{Char, CharBitset};
use std::fmt;

const MAX_SET_SIZE: usize = 2048;

pub type Result<T> = std::result::Result<T, ()>;

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

    pub ignore_whitespace: bool,
    pub ignore_punctuation: bool,
    pub fuzz: usize,
}

impl Expression {
    /// Compile an `Expression` from its string representation
    pub fn new(text: &str) -> Result<Self> {
        let ast_root = parser::ExpressionAst::new_from_str(text).unwrap();
        Self::from_ast(&ast_root)
    }

    pub fn from_ast(ast_root: &parser::ExpressionAst) -> Result<Self> {
        let ignore_whitespace = ast_root.options.whitespace.unwrap_or(true);
        let ignore_punctuation = ast_root.options.punctuation.unwrap_or(true);

        let mut states = vec![];
        Self::build_states(&ast_root.root, &mut states)?;
        // Add a "success" end state (this may not be needed?)
        states.push(State::new());

        let mut expr = Expression {
            states,
            text: format!("{}", ast_root),

            ignore_whitespace,
            ignore_punctuation,
            fuzz: ast_root.options.fuzz.unwrap_or(0),
        };
        Self::optimize_states(&mut expr.states);

        Ok(expr)
    }

    pub fn states_len(&self) -> usize {
        self.states.len()
    }

    /// Extend `states` with the NFA representation of the `ast`
    /// The first new state is the "start" state, and the last added
    /// state must have a "success" transition to the next state.
    fn build_states(ast: &parser::Ast, states: &mut Vec<State>) -> Result<()> {
        let initial_len = states.len();
        match ast {
            parser::Ast::Class(char_bitset) => {
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
                    Self::build_states(alt, states)?;
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
                    Self::build_states(term, states)?;
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
                Self::build_states(term, states)?;
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
                    Self::build_states(term, states)?;
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
            parser::Ast::Anagram(_) => unreachable!(),
        }
        Ok(())
    }

    /// Perform an optimization pass on the NFA, modifying `states` so
    /// that it produces an equivalent NFA that can be evaluated more efficiently
    ///
    /// `RUNTIME: O(states^4)`
    fn optimize_states(states: &mut Vec<State>) {
        // TODO: This function could perform even more complex optimizations

        // `Matcher` requires that there is a transitive closure over `epsilon_states` and that
        // each state has itself included in that set
        // RUNTIME: O(states^4)
        for i in 0..states.len() {
            // Add an epsilon transition from each state to itself
            states[i].epsilon_states.borrow_mut().insert(i);

            // Calculate transitive closure over `epsilon_states`
            // RUNTIME: O(states^3)
            loop {
                let mut ss: BitSet1D = states[i].epsilon_states.clone();
                let mut bs = ss.borrow_mut();

                // RUNTIME: O(states^2)
                for (i2, state2) in states.iter().enumerate() {
                    if !bs.contains(i2) {
                        continue;
                    }
                    bs.union_with(state2.epsilon_states_bitset());
                }
                if states[i].epsilon_states_bitset() == bs.reborrow() {
                    break;
                }
                states[i].epsilon_states = ss;
            }
        }

        // Shrink the `epsilon_states` set to exactly fit the total number of states, so that it
        // can be easily manipulated by `Matcher`'s `BitSet`s
        let states_len = states.len();
        for state in states.iter_mut() {
            state.epsilon_states = state.epsilon_states.resize(states_len);
        }

        // Identify redundant states, and prune them
        // RUNTIME: O(states^3)
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
        // RUNTIME: O(chars * fuzz * states^3)
        for (char_index, chr) in chars.iter().enumerate() {
            let char_bitset = CharBitset::from(*chr);
            let mut all_states_are_empty = true;

            let (lower_table, upper_table) = transition_table.split_at_mut(char_index + 1);

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
