extern crate pest;
#[macro_use]
extern crate pest_derive;

mod bitset;
pub mod parser;

pub mod expression;
pub mod matcher;
pub mod words;

pub use expression::Expression;
pub use matcher::{Matcher, MatcherResponse};
pub use words::{load_wordlist, Word};
