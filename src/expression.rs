use crate::bitset::{BitSet1D, BitSet3D, BitSetRef1D, BitSetRefMut1D};
use crate::parser;
use crate::words::{Char, CharBitset};
use std::fmt;

const MAX_SET_SIZE: usize = 64;

//pub type StateBitSet = BitSet; // 320ms (only 64 bits)
//pub type StateBitSet = FixedBitSet; // 710ms
//pub type StateBitSet = SmallBitSet; // [u64; 1]=495ms; [u32; 1]=520ms; [u64; 4]=495ms; [u64; 4]=510ms
//pub type StateBitSet = HashBitSet; // 2300ms (!!!)

pub type Result<T> = std::result::Result<T, ()>;

/*
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct TransitionGroup {
    items: Box<[StateBitSet]>,
    inner_size: usize,
}

impl TransitionGroup {
    pub fn new(outer_size: usize, inner_size: usize, set_size: usize) -> Self {
        let items = vec![StateBitSet::create(set_size); outer_size * inner_size].into_boxed_slice();
        Self { items, inner_size }
    }

    pub fn slice(&self, index: usize) -> &[StateBitSet] {
        let i = index * self.inner_size;
        // This saves about ~5-8% total time over the checked method
        unsafe { self.items.get_unchecked(i..i + self.inner_size) }
    }

    pub fn slice_mut(&mut self, index: usize) -> &mut [StateBitSet] {
        let i = index * self.inner_size;
        self.items.get_mut(i..i + self.inner_size).unwrap()
    }
}
*/

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

pub struct Expression {
    states: Vec<State>,
    pub text: String,

    pub ignore_whitespace: bool,
    pub ignore_punctuation: bool,
    pub fuzz: usize,
}

impl Expression {
    /// Compile an `Expression` from its string representation
    pub fn new(text: &str) -> Result<Self> {
        let ast_root = parser::parse(text).unwrap();
        let ignore_whitespace = ast_root.flag_whitespace.unwrap_or(true);
        let ignore_punctuation = ast_root.flag_punctuation.unwrap_or(true);

        let mut states = vec![];
        Self::build_states(&ast_root.expression, &mut states)?;
        // Add a "success" end state (this may not be needed?)
        states.push(State::new());
        Self::optimize_states(&mut states);

        Ok(Expression {
            states,
            text: text.to_owned(),

            ignore_whitespace,
            ignore_punctuation,
            fuzz: ast_root.flag_fuzz.unwrap_or(0),
        })
    }

    pub fn states_len(&self) -> usize {
        self.states.len()
    }

    /// Perform an optimization pass on the NFA, modifying `states` so
    /// that it produces an equivalent NFA that can be evaluated more efficiently
    fn optimize_states(states: &mut Vec<State>) {
        // TODO: This currently only calculates a transitive closure over
        // `epsilon_states`; future versions could prune unneeded states, etc.
        for i in 0..states.len() {
            loop {
                let mut ss: BitSet1D = states[i].epsilon_states.clone();
                let mut bs = ss.borrow_mut();

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
        }
        Ok(())
    }

    pub fn epsilon_states(&self, state_index: usize) -> BitSetRef1D<'_> {
        self.states[state_index].epsilon_states_bitset()
    }

    pub fn epsilon_states_mut(&mut self, state_index: usize) -> BitSetRefMut1D<'_> {
        self.states[state_index].epsilon_states_bitset_mut()
    }

    /*
    // This takes a from_state=0 slice, of type [fuzz][to_states]
    pub fn init_transitions_start(&self, transitions: &mut [StateBitSet]) {
        transitions.iter_mut().for_each(|s| s.clear());

        transitions[0].insert(0);
        transitions[0].union_with(self.states[0].epsilon_states);
    }

    // This takes a whole group, of type [from_state][fuzz][to_states]
    pub fn init_transition_table(&self, transition_table: &mut BitSetGroup) {
        assert!(transition_table.items.len() == self.states_len() * transition_table.inner_size);
        for (i, state) in self.states.iter().enumerate() {
            let state_slice = transition_table.slice_mut(i);
            state_slice.iter_mut().for_each(|s| s.clear());

            state_slice[0].insert(i);
            state_slice[0].union_with(state.epsilon_states);
        }
    }
    */

    // transition_table: [char][from_state][fuzz][to_state]
    pub fn fill_transition_table(
        &self,
        chars: &[Char],
        transition_table: &mut [BitSet3D],
    ) -> usize {
        for (char_index, chr) in chars.iter().enumerate() {
            let char_bitset = CharBitset::from(*chr);
            let mut all_states_are_empty = true;

            let (lower_table, upper_table) = transition_table.split_at_mut(char_index + 1);

            // Consume 1 character from the buffer and compute the set of possible resulting states
            for state_index in 0..self.states.len() {
                //let state_transitions = lower_table[char_index].slice(state_index);
                //assert!(state_transitions.len() == self.fuzz + 1);

                //let next_state_transitions = upper_table[0].slice_mut(state_index);
                //assert!(next_state_transitions.len() == self.fuzz + 1);

                let mut all_fuzz_are_empty = true;
                for fuzz_index in 0..=self.fuzz {
                    let state_transitions =
                        lower_table[char_index].slice((state_index, fuzz_index));
                    let mut next_state_transitions =
                        upper_table[0].slice_mut((state_index, fuzz_index));

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
                let mut fuzz_superset = BitSet1D::new((), self.states.len());
                for fuzz_index in 0..self.fuzz {
                    let state_transitions =
                        lower_table[char_index].slice((state_index, fuzz_index));
                    //let next_state_transitions = upper_table[0].slice_mut((state_index, fuzz_index));
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

    fn char_transitions<'a>(
        &'a self,
        char_bitset: CharBitset,
        start_states: BitSetRef1D<'a>,
    ) -> BitSet1D {
        let mut result_bitset = BitSet1D::new((), self.states.len());
        let mut end_states = result_bitset.slice_mut(());

        if !start_states.is_empty() {
            for (si, state) in self.states.iter().enumerate() {
                if !start_states.contains(si) {
                    continue;
                }

                if char_bitset.is_intersecting(state.char_bitset) {
                    end_states.insert(state.next_state);
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
            writeln!(
                f,
                "    {}: {:?} -> {}; epsilon -> {:?}",
                i, state.char_bitset, state.next_state, state.epsilon_states
            )?;
        }
        Ok(())
    }
}
