use crate::words::*;
use pest::iterators::{Pair, Pairs};
use pest::Parser;

pub use pest::error::Error;
pub type Result<T> = std::result::Result<T, Error<Rule>>;

#[derive(Parser)]
#[grammar = "noodle_grammar.pest"]
struct NoodleParser;

#[derive(Debug)]
pub struct AstRoot {
    pub expression: Ast,
    pub flag_whitespace: Option<bool>,
    pub flag_punctuation: Option<bool>,
    pub flag_fuzz: Option<usize>,
}

#[derive(Debug)]
pub enum Ast {
    Class(CharBitset),
    Alternatives(Vec<Self>),
    Sequence(Vec<Self>),
    Repetition {
        term: Box<Self>,
        min: usize,
        max: Option<usize>,
    },
}

pub fn parse(input_str: &str) -> Result<AstRoot> {
    let mut pairs = NoodleParser::parse(Rule::expression, input_str)?
        .next()
        .unwrap()
        .into_inner();
    //println!("Parse: {:#?}", pairs);

    let subexpression = pairs.next().unwrap();
    assert!(
        subexpression.as_rule() == Rule::sequence || subexpression.as_rule() == Rule::alternatives
    );

    fn parse_numbers(pairs: Pairs<'_, Rule>) -> Vec<usize> {
        pairs
            .filter_map(|p| {
                if p.as_rule() == Rule::number {
                    p.as_str().parse().ok()
                } else {
                    None
                }
            })
            .collect()
    };

    fn parse_subexpression(pair: Pair<Rule>) -> Option<Ast> {
        match pair.as_rule() {
            Rule::literal => Some(Ast::Class(CharBitset::from(
                pair.as_str().chars().next().unwrap(),
            ))),
            Rule::dot => Some(Ast::Class(CharBitset::LETTERS)),
            Rule::class => {
                let mut inner = pair.into_inner();
                let mut invert = false;
                if inner.peek().map(|p| p.as_rule()) == Some(Rule::invert) {
                    inner.next().unwrap();
                    invert = true;
                }
                let mut bitset = CharBitset::EMPTY;
                inner.for_each(|p| match p.as_rule() {
                    Rule::invert => invert = true,
                    Rule::letter_range => {
                        let cs = p.as_str().chars().collect::<Vec<_>>();
                        bitset.union(CharBitset::from_range(cs[0], cs[2]));
                    }
                    Rule::literal => {
                        bitset.union(CharBitset::from(p.as_str().chars().next().unwrap()))
                    }
                    _ => unreachable!(),
                });
                if invert {
                    bitset.invert();
                }
                Some(Ast::Class(bitset))
            }
            Rule::group => Some(Ast::Sequence(
                pair.into_inner().filter_map(parse_subexpression).collect(),
            )),
            Rule::sequence => Some(Ast::Sequence(
                pair.into_inner().filter_map(parse_subexpression).collect(),
            )),
            Rule::repeat_optional
            | Rule::repeat_any
            | Rule::repeat_oneormore
            | Rule::repeat_exact
            | Rule::repeat_atmost
            | Rule::repeat_atleast
            | Rule::repeat_range => {
                let rule = pair.as_rule();
                let mut pairs = pair.into_inner();
                let term = Box::new(pairs.next().and_then(parse_subexpression).unwrap());
                let numbers = parse_numbers(pairs);
                let (min, max) = match rule {
                    Rule::repeat_optional => (0, Some(1)),
                    Rule::repeat_any => (0, None),
                    Rule::repeat_oneormore => (1, None),
                    Rule::repeat_exact => (numbers[0], Some(numbers[0])),
                    Rule::repeat_atmost => (0, Some(numbers[0])),
                    Rule::repeat_atleast => (numbers[0], None),
                    Rule::repeat_range => (numbers[0], Some(numbers[1])),
                    _ => unreachable!(),
                };
                Some(Ast::Repetition { term, min, max })
            }
            Rule::alternatives => Some(Ast::Alternatives(
                pair.into_inner().filter_map(parse_subexpression).collect(),
            )),

            _ => None,
        }
    }
    let expression = parse_subexpression(subexpression).unwrap();

    let mut flag_whitespace = None;
    let mut flag_punctuation = None;
    let mut flag_fuzz = None;
    for pair in pairs {
        match pair.as_rule() {
            Rule::flag => {
                let flag = pair.as_str().get(1..).unwrap();
                match flag {
                    "_" => flag_whitespace = Some(true),
                    "'" | "-" => flag_punctuation = Some(true),
                    _ => flag_fuzz = Some(parse_numbers(pair.into_inner())[0]),
                }
            }
            Rule::EOI => (),
            _ => unreachable!(),
        }
    }

    Ok(AstRoot {
        expression,
        flag_whitespace,
        flag_punctuation,
        flag_fuzz,
    })
}
