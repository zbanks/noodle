use crate::bitset::{BitSet, FixedBitSet, HashBitSet, Set, SmallBitSet};
use crate::parser;
use crate::words::{Char, CharBitset};
use std::fmt;

const MAX_SET_SIZE: usize = 64;

//pub type StateBitSet = BitSet; // 320ms (only 64 bits)
//pub type StateBitSet = SmallBitSet; // [u64; 1]=495ms; [u32; 1]=520ms; [u64; 4]=495ms; [u64; 4]=510ms
pub type StateBitSet = FixedBitSet; // 710ms
                                    //pub type StateBitSet = HashBitSet; // 2300ms (!!!)

pub type Result<T> = std::result::Result<T, ()>;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct TransitionGroup {
    items: Vec<StateBitSet>,
    inner_size: usize,
}

impl TransitionGroup {
    pub fn new(outer_size: usize, inner_size: usize, set_size: usize) -> Self {
        let items = vec![StateBitSet::create(set_size); outer_size * inner_size];
        Self { items, inner_size }
    }

    pub fn slice(&self, index: usize) -> &[StateBitSet] {
        let i = index * self.inner_size;
        self.items.get(i..i + self.inner_size).unwrap()
    }

    pub fn slice_mut(&mut self, index: usize) -> &mut [StateBitSet] {
        let i = index * self.inner_size;
        self.items.get_mut(i..i + self.inner_size).unwrap()
    }
}

#[derive(Debug, Clone)]
struct State {
    epsilon_states: StateBitSet,

    char_bitset: CharBitset,
    next_state: usize,
}

impl State {
    fn new() -> Self {
        Self {
            epsilon_states: StateBitSet::create(MAX_SET_SIZE),
            char_bitset: CharBitset::EMPTY,
            next_state: 0,
        }
    }
}

pub struct Expression {
    states: Vec<State>,
    text: String,

    pub ignore_whitespace: bool,
    pub ignore_punctuation: bool,
    pub fuzz: usize,
}

impl Expression {
    /// Hard-coded NFAs for head-to-head comparison of matching engine
    /// performance against C/Zig implementations
    pub fn example(x: usize) -> Self {
        let states = match x {
            0 => {
                let mut states: Vec<_> = (0..16)
                    .map(|i| {
                        let mut s = State::new();
                        s.next_state = i + 1;
                        s
                    })
                    .collect();
                states[0].char_bitset = Char::from('e').into();
                states[1].char_bitset = Char::from('x').into();
                states[2].char_bitset = CharBitset::LETTERS;
                states[3].char_bitset = Char::from('r').into();
                states[4].char_bitset = Char::from('e').into();
                states[5].epsilon_states.insert(6);
                states[5].epsilon_states.insert(8);
                states[6].char_bitset = Char::from('s').into();
                states[7].epsilon_states.insert(6);
                states[7].epsilon_states.insert(8);
                states[8].char_bitset = Char::from('i').into();
                states[9].char_bitset = Char::from('o').into();
                states[10].char_bitset = Char::from('n').into();
                states[11].char_bitset = Char::from('t').into();
                states[12].char_bitset = Char::from('e').into();
                states[13].char_bitset = Char::from('s').into();
                states[14].char_bitset = Char::from('t').into();
                states
            }
            1 => {
                let mut states: Vec<_> = (0..16)
                    .map(|i| {
                        let mut s = State::new();
                        s.next_state = i + 1;
                        s
                    })
                    .collect();
                states[0].char_bitset = Char::from('e').into();
                states[1].epsilon_states.insert(2);
                states[1].epsilon_states.insert(3);
                states[1].epsilon_states.insert(4);
                states[1].epsilon_states.insert(5);
                states[2].char_bitset = Char::from('x').into();
                states[3].epsilon_states.insert(4);
                states[3].epsilon_states.insert(5);
                states[4].char_bitset = Char::from('z').into();
                states[5].char_bitset = Char::from('p').into();
                states[6].char_bitset = Char::from('r').into();
                states[7].char_bitset = Char::from('e').into();
                states[8].char_bitset = Char::from('s').into();
                states[9].char_bitset = Char::from('s').into();
                states[10].epsilon_states.insert(9);
                states[10].epsilon_states.insert(11);
                states[11].char_bitset = CharBitset::LETTERS_BUT_I;
                states[12].epsilon_states.insert(13);
                states[12].epsilon_states.insert(15);
                states[13].char_bitset = CharBitset::LETTERS;
                states[14].epsilon_states.insert(13);
                states[14].epsilon_states.insert(15);
                states
            }
            _ => unreachable!(),
        };
        Self {
            states,
            text: "example".to_string(),
            ignore_whitespace: true,
            ignore_punctuation: true,
            fuzz: match x {
                0 => 2,
                1 => 0,
                _ => 0,
            },
        }
    }

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
                let mut ss: StateBitSet = states[i].epsilon_states.clone();
                for (i2, state2) in states.iter().enumerate() {
                    if !ss.contains(i2) {
                        continue;
                    }
                    ss.union_with(&state2.epsilon_states);
                }
                if ss == states[i].epsilon_states {
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
            parser::Ast::Class(char_bitset) => states.push(State {
                epsilon_states: StateBitSet::create(MAX_SET_SIZE),
                char_bitset: *char_bitset,
                next_state: initial_len + 1,
            }),
            parser::Ast::Alternatives(alts) => {
                states.push(State::new());
                let mut end_indexes = vec![];
                for alt in alts {
                    let next_index = states.len();
                    states[initial_len].epsilon_states.insert(next_index);
                    Self::build_states(alt, states)?;
                    end_indexes.push(states.len() - 1);
                }
                let next_index = states.len();
                for end_index in end_indexes {
                    // Repoint the terminal states to the true success state
                    if states[end_index].epsilon_states.contains(end_index + 1) {
                        states[end_index].epsilon_states.remove(end_index + 1);
                        states[end_index].epsilon_states.insert(next_index);
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
                        states[initial_len].epsilon_states.insert(next_index);
                    }
                    final_term_index = states.len();
                    Self::build_states(term, states)?;
                }
                let final_index = states.len();
                states.push(State::new());
                states[final_index].epsilon_states.insert(final_index + 1);
                if *min == 0 {
                    states[initial_len].epsilon_states.insert(final_index);
                }
                if *max == None {
                    states[final_index].epsilon_states.insert(final_term_index);
                }
            }
        }
        Ok(())
    }

    pub fn init_transitions_start(&self, transitions: &mut [StateBitSet]) {
        transitions.iter_mut().for_each(|s| s.clear());

        transitions[0].insert(0);
        transitions[0].union_with(&self.states[0].epsilon_states);
    }

    pub fn init_transition_table(&self, transition_table: &mut TransitionGroup) {
        assert!(transition_table.items.len() == self.states_len() * transition_table.inner_size);
        for (i, state) in self.states.iter().enumerate() {
            let state_slice = transition_table.slice_mut(i);
            state_slice.iter_mut().for_each(|s| s.clear());

            state_slice[0].insert(i);
            state_slice[0].union_with(&state.epsilon_states);
        }
    }

    pub fn fill_transition_table(
        &self,
        chars: &[Char],
        transition_table: &mut [TransitionGroup],
    ) -> usize {
        for (char_index, chr) in chars.iter().enumerate() {
            let char_bitset = CharBitset::from(*chr);
            let mut all_states_are_empty = true;

            let (lower_table, upper_table) = transition_table.split_at_mut(char_index + 1);

            // Consume 1 character from the buffer and compute the set of possible resulting states
            for state_index in 0..self.states.len() {
                let state_transitions = lower_table[char_index].slice(state_index);
                assert!(state_transitions.len() == self.fuzz + 1);

                let next_state_transitions = upper_table[0].slice_mut(state_index);
                assert!(next_state_transitions.len() == self.fuzz + 1);

                let mut all_fuzz_are_empty = true;
                for fuzz_index in 0..=self.fuzz {
                    if state_transitions[fuzz_index].is_empty() {
                        next_state_transitions[fuzz_index].clear();
                    } else {
                        next_state_transitions[fuzz_index] =
                            self.char_transitions(char_bitset, &state_transitions[fuzz_index]);
                        all_fuzz_are_empty = false;
                    }
                }
                if all_fuzz_are_empty {
                    continue;
                }
                all_states_are_empty = false;

                // For a fuzzy match, expand `next_state_table[fi+1]` by adding all states
                // reachable from `state_table[f]` *but* with a 1-character change to `chars`
                let mut fuzz_superset = StateBitSet::create(self.states.len());
                for fuzz_index in 0..self.fuzz {
                    if state_transitions[fuzz_index].is_empty() {
                        continue;
                    }

                    // Deletion
                    next_state_transitions[fuzz_index + 1]
                        .union_with(&state_transitions[fuzz_index]);

                    // Change
                    let change_set =
                        self.char_transitions(CharBitset::LETTERS, &state_transitions[fuzz_index]);
                    next_state_transitions[fuzz_index + 1].union_with(&change_set);

                    // Insertion
                    let insertion_set = self.char_transitions(char_bitset, &change_set);
                    next_state_transitions[fuzz_index + 1].union_with(&insertion_set);

                    // Optimization: discard the states we can get to with less fuzz
                    next_state_transitions[fuzz_index + 1].difference_with(&fuzz_superset);
                    fuzz_superset.union_with(&next_state_transitions[fuzz_index + 1]);
                }
            }
            if all_states_are_empty {
                return char_index;
            }
        }
        chars.len()
    }

    fn char_transitions(&self, char_bitset: CharBitset, start_states: &StateBitSet) -> StateBitSet {
        let mut end_states = StateBitSet::create(self.states.len());

        if start_states.is_empty() {
            return end_states;
        }

        for (si, state) in self.states.iter().enumerate() {
            if !start_states.contains(si) {
                continue;
            }

            if char_bitset.is_intersecting(state.char_bitset) {
                end_states.insert(state.next_state);
                end_states.union_with(&self.states[state.next_state].epsilon_states);
            }
        }

        end_states
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
