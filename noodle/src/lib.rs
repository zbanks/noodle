extern crate pest;
#[macro_use]
extern crate pest_derive;

mod bitset;
mod matcher;
pub mod expression;
pub mod parser;
pub mod query;
pub mod words;

pub use expression::Expression;
pub use query::{QueryEvaluator, QueryResponse};
pub use words::{load_wordlist, Word};
