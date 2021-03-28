use crate::words::*;
use indexmap::IndexMap;
use pest::error::{Error as PestError, LineColLocation};
use pest::iterators::{Pair, Pairs};
use pest::Parser;
use std::fmt;

pub type Result<T> = std::result::Result<T, PestError<Rule>>;

#[derive(Parser)]
#[grammar = "noodle_grammar.pest"]
struct NoodleParser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionOptions {
    pub explicit_word_boundaries: Option<bool>,
    pub explicit_punctuation: Option<bool>,
    pub fuzz: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryOptions {
    pub max_words: Option<usize>,
    pub dictionary: Option<String>,
    pub results_limit: Option<usize>,
    pub quiet: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionAst {
    pub original_text: Option<String>,
    pub root: Ast,
    pub options: ExpressionOptions,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryAst {
    original_text: String,
    macros: IndexMap<String, String>,
    pub expressions: Vec<ExpressionAst>,
    pub options: QueryOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnagramKind {
    Standard,
    Super,
    Sub,
    TransAdd(usize),
    TransDelete(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ast {
    // Base operations, available in "simple expressions"
    Class(CharBitset),
    Alternatives(Vec<Self>),
    Sequence(Vec<Self>),
    Repetition {
        term: Box<Self>,
        min: usize,
        max: Option<usize>,
    },

    // Advance query operations, not available in raw expressions
    Anagram {
        kind: AnagramKind,
        bank: Vec<Char>,
    },
}

impl ExpressionAst {
    pub fn new_from_str(input_str: &str) -> Result<Self> {
        let pairs = NoodleParser::parse(Rule::expression, input_str)?
            .next()
            .unwrap()
            .into_inner();

        let mut expr = parse_expression(pairs);
        expr.original_text = Some(input_str.to_owned());

        Ok(expr)
    }
}

impl fmt::Display for ExpressionAst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        if self.options.explicit_word_boundaries == Some(true) {
            write!(f, " !_")?;
        }
        if self.options.explicit_punctuation == Some(true) {
            write!(f, " !'")?;
        }
        if let Some(fuzz) = self.options.fuzz {
            write!(f, " !{}", fuzz)?;
        }
        Ok(())
    }
}

fn error_set_line(mut err: PestError<Rule>, line: usize) -> PestError<Rule> {
    match &mut err.line_col {
        LineColLocation::Pos((l, _)) => *l = line,
        LineColLocation::Span((l1, _), (l2, _)) => {
            *l2 = line + *l2 - *l1;
            *l1 = line;
        }
    };
    err
}

impl QueryAst {
    pub fn new_from_str(input_str: &str) -> Result<Self> {
        let mut expressions = vec![];
        let mut macros: IndexMap<String, String> = IndexMap::new();
        let mut options = QueryOptions {
            max_words: None,
            dictionary: None,
            results_limit: None,
            quiet: None,
        };

        for (i, line) in input_str.split(&['\n', ';'][..]).enumerate() {
            let mut line = line.to_owned();

            for (macro_name, macro_value) in macros.iter() {
                line = line.replace(macro_name, macro_value);
            }

            let mut pair = NoodleParser::parse(Rule::query, &line)
                .map_err(|e| error_set_line(e, i + 1))?
                .next()
                .unwrap()
                .into_inner();

            if let Some(pair) = pair.next() {
                match pair.as_rule() {
                    Rule::expression => {
                        let mut expr = parse_expression(pair.into_inner());
                        expr.original_text = Some(line);

                        expressions.push(expr);
                    }
                    Rule::pragma_words => {
                        let inner = pair.into_inner();
                        let numbers = parse_numbers(inner);
                        options.max_words = numbers.get(0).cloned();
                    }
                    Rule::pragma_dict => {
                        let inner = pair.into_inner();
                        options.dictionary = Some(inner.map(|p| p.as_str()).collect());
                    }
                    Rule::pragma_limit => {
                        let inner = pair.into_inner();
                        let numbers = parse_numbers(inner);
                        options.results_limit = numbers.get(0).cloned();
                    }
                    Rule::pragma_quiet => {
                        options.quiet = Some(true);
                    }
                    Rule::macro_define => {
                        let inner = pair.into_inner();
                        let terms: Vec<_> = inner
                            .filter_map(|p| match p.as_rule() {
                                Rule::macro_name => Some(p.as_str()),
                                Rule::macro_value => Some(p.as_str()),
                                _ => None,
                            })
                            .collect();
                        // TODO: Maybe check if it's unique? Or not use a hashmap at all?
                        macros.insert(terms[0].to_owned(), terms[1].to_owned());
                    }
                    _ => println!("Unexpected: {:?}", pair),
                }
            }
        }

        let mut ast = QueryAst {
            original_text: input_str.to_owned(),
            macros,

            expressions,
            options,
        };
        ast.expand_expressions();

        Ok(ast)
    }

    fn expand_expressions(&mut self) {
        fn visit<F>(node: &mut Ast, action: &mut F)
        where
            F: FnMut(&mut Ast),
        {
            match node {
                Ast::Class(_) => (),
                Ast::Alternatives(nodes) | Ast::Sequence(nodes) => {
                    nodes.iter_mut().for_each(|n| visit(n, action))
                }
                Ast::Repetition {
                    term: _,
                    min: _,
                    max: _,
                } => (),
                Ast::Anagram { kind: _, bank: _ } => action(node),
            }
        }

        let mut new_expressions = vec![];
        for expression in self.expressions.iter() {
            let mut expression: ExpressionAst = expression.clone();

            let mut anagrams = vec![];
            visit(&mut expression.root, &mut |node: &mut Ast| {
                if let Ast::Anagram { kind: _, bank } = node {
                    anagrams.push(bank.clone());
                }
            });

            let anagram_sets: Vec<Vec<_>> = anagrams
                .iter()
                .map(|letters| {
                    let mut set: [usize; Char::_MAX] = [0; Char::_MAX];
                    letters.iter().for_each(|l| set[l.as_index()] += 1);
                    set.iter()
                        .enumerate()
                        .filter_map(|(i, &c)| {
                            if c > 0 {
                                Some((Char::from_index(i), c))
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .collect();

            let max_unique_letters = anagram_sets
                .iter()
                .map(|h| h.iter().count())
                .max()
                .unwrap_or(0);

            fn ast_for_anagram(histogram: &[(Char, usize)], nth: usize) -> Ast {
                let total_length = histogram.iter().map(|(_, i)| i).sum();
                let mut histogram: Vec<_> = histogram.into();
                let mut char_bitset: CharBitset = CharBitset::EMPTY;
                histogram
                    .iter()
                    .for_each(|(c, _)| char_bitset.union(c.into()));

                if nth < histogram.len() {
                    let (ch, count) = histogram.remove(nth);
                    let ch_ast = Ast::Class(ch.into());
                    let mut fill_bitset = char_bitset;
                    fill_bitset.difference_with(ch.into());

                    let fill_ast = Ast::Repetition {
                        term: Box::new(Ast::Class(fill_bitset)),
                        min: 0,
                        max: None,
                    };

                    let mut seq = vec![];
                    for _ in 0..count {
                        seq.push(fill_ast.clone());
                        seq.push(ch_ast.clone());
                    }
                    seq.push(fill_ast);
                    Ast::Sequence(seq)
                } else {
                    Ast::Repetition {
                        term: Box::new(Ast::Class(char_bitset)),
                        min: total_length,
                        max: Some(total_length),
                    }
                }
            }

            let replacements: Vec<Vec<Ast>> = anagram_sets
                .iter()
                .map(|hist| {
                    (0..max_unique_letters + 1)
                        .map(|i| ast_for_anagram(hist, i))
                        .collect::<Vec<_>>()
                })
                .collect();

            for i in 0..max_unique_letters + 1 {
                let mut j = 0;
                let mut expression = expression.clone();
                visit(&mut expression.root, &mut |node: &mut Ast| {
                    *node = replacements[j][i].clone();
                    j += 1;
                });
                new_expressions.push(expression);
            }
        }

        self.expressions = new_expressions;
    }
}

impl fmt::Display for Ast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ast::Class(CharBitset::LETTERS) => write!(f, ".")?,
            Ast::Class(char_bitset) => write!(f, "{:?}", char_bitset)?,
            Ast::Alternatives(nodes) => {
                if let Some(first) = nodes.get(0) {
                    write!(f, "({}", first)?;
                    for node in nodes.get(1..).unwrap() {
                        write!(f, "|{}", node)?;
                    }
                    write!(f, ")")?;
                }
            }
            Ast::Sequence(nodes) => {
                if nodes.len() > 1 {
                    write!(f, "(")?;
                }
                nodes.iter().try_for_each(|n| write!(f, "{}", n))?;
                if nodes.len() > 1 {
                    write!(f, ")")?;
                }
            }
            Ast::Repetition {
                term,
                min: 0,
                max: None,
            } => write!(f, "{}*", term)?,
            Ast::Repetition {
                term,
                min: 0,
                max: Some(1),
            } => write!(f, "{}?", term)?,
            Ast::Repetition {
                term,
                min: 1,
                max: None,
            } => write!(f, "{}+", term)?,
            Ast::Repetition {
                term,
                min: 0,
                max: Some(max),
            } => write!(f, "{}{{,{}}}", term, max)?,
            Ast::Repetition {
                term,
                min,
                max: None,
            } => write!(f, "{}{{{},}}", term, min)?,
            Ast::Repetition {
                term,
                min,
                max: Some(max),
            } if min == max => write!(f, "{}{{{}}}", term, min)?,
            Ast::Repetition {
                term,
                min,
                max: Some(max),
            } => write!(f, "{}{{{}, {}}}", term, min, max)?,
            Ast::Anagram { kind: _, bank } => {
                write!(f, "<")?;
                bank.iter().try_for_each(|c| write!(f, "{:?}", c))?;
                write!(f, ">")?;
            }
        }
        Ok(())
    }
}

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
}

fn parse_subexpression(pair: Pair<Rule>) -> Option<Ast> {
    match pair.as_rule() {
        Rule::character => Some(Ast::Class(CharBitset::from(
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
                Rule::character => {
                    bitset.union(CharBitset::from(p.as_str().chars().next().unwrap()))
                }
                _ => unreachable!(),
            });
            if invert {
                bitset.invert();
            }
            Some(Ast::Class(bitset))
        }
        Rule::partial_group => Some(Ast::Sequence(
            pair.into_inner()
                .filter_map(parse_subexpression)
                .map(|c| Ast::Repetition {
                    term: Box::new(c),
                    min: 0,
                    max: Some(1),
                })
                .collect(),
        )),
        Rule::group => Some(Ast::Sequence(
            pair.into_inner().filter_map(parse_subexpression).collect(),
        )),
        Rule::sequence => Some(Ast::Sequence(
            pair.into_inner().filter_map(parse_subexpression).collect(),
        )),
        Rule::number => {
            let dot = Ast::Class(CharBitset::LETTERS);
            let n: usize = pair.as_str().parse().unwrap();
            // TODO: Add explicit_word_boundaries; set explicit_word_boundaries flag
            Some(Ast::Repetition {
                term: Box::new(dot),
                min: n,
                max: Some(n),
            })
        }
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
        Rule::anagram_body => Some(Ast::Anagram {
            kind: AnagramKind::Standard,
            bank: pair.as_str().chars().map(|c| c.into()).collect(),
        }),
        Rule::alternatives => Some(Ast::Alternatives(
            pair.into_inner().filter_map(parse_subexpression).collect(),
        )),

        _ => None,
    }
}

fn parse_flags(pairs: Pairs<'_, Rule>) -> ExpressionOptions {
    let mut explicit_word_boundaries = None;
    let mut explicit_punctuation = None;
    let mut fuzz = None;

    for pair in pairs {
        match pair.as_rule() {
            Rule::flag => {
                let flag = pair.as_str().get(1..).unwrap();
                match flag {
                    "_" => explicit_word_boundaries = Some(true),
                    "'" | "-" => explicit_punctuation = Some(true),
                    _ => fuzz = Some(parse_numbers(pair.into_inner())[0]),
                }
            }
            Rule::EOI => (),
            _ => unreachable!(),
        }
    }

    ExpressionOptions {
        explicit_word_boundaries,
        explicit_punctuation,
        fuzz,
    }
}

pub fn parse_expression(mut pairs: Pairs<'_, Rule>) -> ExpressionAst {
    let subexpression = pairs.next().unwrap();
    assert!(
        subexpression.as_rule() == Rule::sequence || subexpression.as_rule() == Rule::alternatives
    );

    let expression = parse_subexpression(subexpression).unwrap();
    let options = parse_flags(pairs);

    ExpressionAst {
        original_text: None,
        root: expression,
        options,
    }
}
