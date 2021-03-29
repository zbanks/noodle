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
pub struct QueryAst {
    original_text: String,
    macros: IndexMap<String, String>,
    pub expressions: Vec<ExpressionAst>,
    pub options: QueryOptions,
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
pub struct ExpressionOptions {
    pub explicit_word_boundaries: Option<bool>,
    pub explicit_punctuation: Option<bool>,
    pub fuzz: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ast {
    // Base operations, available in "simple expressions"
    CharClass(CharBitset),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnagramKind {
    Standard,
    Super,
    Sub,
    TransAdd(usize),
    TransDelete(usize),
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
                Ast::CharClass(_) => (),
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
                if let Ast::Anagram { kind, bank } = node {
                    anagrams.push((*kind, bank.clone()));
                }
            });

            let anagram_sets: Vec<(_, Vec<_>)> = anagrams
                .iter()
                .map(|(kind, bank)| {
                    let mut set: [usize; Char::_MAX] = [0; Char::_MAX];
                    bank.iter().for_each(|l| set[l.as_index()] += 1);
                    (
                        *kind,
                        set.iter()
                            .enumerate()
                            .filter_map(|(i, &c)| {
                                if c > 0 {
                                    Some((Char::from_index(i), c))
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    )
                })
                .collect();

            let max_unique_letters = anagram_sets
                .iter()
                .map(|(_k, h)| h.iter().count())
                .max()
                .unwrap_or(0);

            fn ast_for_anagram(kind: AnagramKind, histogram: &[(Char, usize)], nth: usize) -> Ast {
                let total_length = histogram.iter().map(|(_, i)| i).sum();
                let mut histogram: Vec<_> = histogram.into();
                let mut char_bitset: CharBitset = CharBitset::EMPTY;
                histogram
                    .iter()
                    .for_each(|(c, _)| char_bitset.union(c.into()));

                if nth < histogram.len() {
                    let (ch, count) = histogram.remove(nth);

                    // Convert histogram element (2, a) to something like /[^a]a[^a]a[^a]/
                    // `char_ast` is the /a/ part
                    let char_ast = Ast::CharClass(ch.into());
                    let char_ast = match kind {
                        // For standard & additive, we need (at least) `count` copies of each letter
                        AnagramKind::Standard | AnagramKind::Super | AnagramKind::TransAdd(_) => {
                            char_ast
                        }
                        // For subtractive, we can have at most `count` copies of each letter.
                        // This is like using /a?/ in the regex
                        AnagramKind::Sub | AnagramKind::TransDelete(_) => Ast::Repetition {
                            term: Box::new(char_ast),
                            min: 0,
                            max: Some(1),
                        },
                    };

                    let mut fill_bitset = char_bitset;
                    fill_bitset.difference_with(ch.into());

                    // `fill_ast` is the /[^a]/ part
                    let fill_ast = match kind {
                        // For standard & subtractive, the letters "between" must be made up of
                        // other letters from the histogram.
                        // We could use /./, but being more specific here makes the eval faster
                        AnagramKind::Standard | AnagramKind::Sub | AnagramKind::TransDelete(_) => {
                            Ast::Repetition {
                                term: Box::new(Ast::CharClass(fill_bitset)),
                                min: 0,
                                max: None,
                            }
                        }
                        // For additive, the letters between can be anything
                        AnagramKind::Super | AnagramKind::TransAdd(_) => Ast::Repetition {
                            term: Box::new(Ast::CharClass(CharBitset::LETTERS)),
                            min: 0,
                            max: None,
                        },
                    };

                    let mut seq = vec![];
                    for _ in 0..count {
                        seq.push(fill_ast.clone());
                        seq.push(char_ast.clone());
                    }
                    seq.push(fill_ast);
                    Ast::Sequence(seq)
                } else {
                    // Length constraint
                    match kind {
                        AnagramKind::Standard => Ast::Repetition {
                            term: Box::new(Ast::CharClass(char_bitset)),
                            min: total_length,
                            max: Some(total_length),
                        },
                        AnagramKind::Sub => Ast::Repetition {
                            term: Box::new(Ast::CharClass(char_bitset)),
                            min: 0,
                            max: Some(total_length),
                        },
                        AnagramKind::TransDelete(d) => Ast::Repetition {
                            term: Box::new(Ast::CharClass(char_bitset)),
                            min: total_length.saturating_sub(d),
                            max: Some(total_length.saturating_sub(d)),
                        },
                        AnagramKind::Super => Ast::Repetition {
                            term: Box::new(Ast::CharClass(CharBitset::LETTERS)),
                            min: total_length,
                            max: None,
                        },
                        AnagramKind::TransAdd(a) => Ast::Repetition {
                            term: Box::new(Ast::CharClass(CharBitset::LETTERS)),
                            min: total_length + a,
                            max: Some(total_length + a),
                        },
                    }
                }
            }

            let replacements: Vec<Vec<Ast>> = anagram_sets
                .iter()
                .map(|(kind, hist)| {
                    (0..max_unique_letters + 1)
                        .map(|i| ast_for_anagram(*kind, hist, i))
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
            Ast::CharClass(CharBitset::LETTERS) => write!(f, ".")?,
            Ast::CharClass(char_bitset) => write!(f, "{:?}", char_bitset)?,
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

fn parse_anagram(mut pairs: Pairs<'_, Rule>) -> (Vec<Char>, Option<usize>) {
    let body = pairs.next().unwrap();
    assert_eq!(body.as_rule(), Rule::anagram_body);
    let bank = body.as_str().chars().map(|c| c.into()).collect();
    let number = parse_numbers(pairs).get(0).cloned();

    (bank, number)
}

fn parse_subexpression(pair: Pair<Rule>) -> Option<Ast> {
    let rule = pair.as_rule();
    match rule {
        Rule::character => Some(Ast::CharClass(CharBitset::from(
            pair.as_str().chars().next().unwrap(),
        ))),
        Rule::dot => Some(Ast::CharClass(CharBitset::LETTERS)),
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
            Some(Ast::CharClass(bitset))
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
            let dot = Ast::CharClass(CharBitset::LETTERS);
            let n: usize = pair.as_str().parse().unwrap();
            Some(Ast::Sequence(vec![
                Ast::Repetition {
                    term: Box::new(dot),
                    min: n,
                    max: Some(n),
                },
                Ast::CharClass(Char::WORD_END.into()),
            ]))
        }
        Rule::repeat_optional
        | Rule::repeat_any
        | Rule::repeat_oneormore
        | Rule::repeat_exact
        | Rule::repeat_atmost
        | Rule::repeat_atleast
        | Rule::repeat_range => {
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
        Rule::anagram | Rule::subanagram | Rule::superanagram => {
            let (bank, number) = parse_anagram(pair.into_inner());
            assert!(number.is_none());
            Some(Ast::Anagram {
                kind: match rule {
                    Rule::anagram => AnagramKind::Standard,
                    Rule::subanagram => AnagramKind::Sub,
                    Rule::superanagram => AnagramKind::Super,
                    _ => unreachable!(),
                },
                bank,
            })
        }
        Rule::transadd | Rule::transdelete => {
            let (bank, number) = parse_anagram(pair.into_inner());
            assert!(number.is_some());
            let mut number = number.unwrap();
            if rule == Rule::transdelete && number > bank.len() {
                // TODO: is this an error? or should it just degrade to a subanagram?
                println!("Warning: transdelete longer than bank");
                number = bank.len();
            }
            Some(Ast::Anagram {
                kind: match rule {
                    Rule::transadd => AnagramKind::TransAdd(number),
                    Rule::transdelete => AnagramKind::TransDelete(number),
                    _ => unreachable!(),
                },
                bank,
            })
        }
        Rule::alternatives => Some(Ast::Alternatives(
            pair.into_inner().filter_map(parse_subexpression).collect(),
        )),

        _ => None,
    }
}

fn parse_options(pairs: Pairs<'_, Rule>) -> ExpressionOptions {
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

fn detect_options(ast: &Ast, options: &mut ExpressionOptions) {
    match ast {
        Ast::CharClass(char_bitset) => {
            if char_bitset.contains(Char::WORD_END) {
                options.explicit_word_boundaries = Some(true);
            }
            if char_bitset.contains(Char::PUNCTUATION) {
                options.explicit_punctuation = Some(true);
            }
        }
        Ast::Alternatives(nodes) => nodes.iter().for_each(|n| detect_options(n, options)),
        Ast::Sequence(nodes) => nodes.iter().for_each(|n| detect_options(n, options)),
        Ast::Repetition {
            term,
            min: _,
            max: _,
        } => detect_options(term, options),
        Ast::Anagram { kind: _, bank } => {
            if bank.contains(&Char::WORD_END) {
                options.explicit_word_boundaries = Some(true);
            }
            if bank.contains(&Char::PUNCTUATION) {
                options.explicit_punctuation = Some(true);
            }
        }
    }
}

pub fn parse_expression(mut pairs: Pairs<'_, Rule>) -> ExpressionAst {
    let subexpression = pairs.next().unwrap();
    assert!(
        subexpression.as_rule() == Rule::sequence || subexpression.as_rule() == Rule::alternatives
    );

    let expression = parse_subexpression(subexpression).unwrap();
    let mut options = parse_options(pairs);
    detect_options(&expression, &mut options);

    ExpressionAst {
        original_text: None,
        root: expression,
        options,
    }
}

#[test]
fn test_expression_ast() {
    use Ast::*;

    // Basic string
    assert_eq!(
        ExpressionAst::new_from_str("hello").unwrap().root,
        Sequence(vec![
            CharClass('h'.into()),
            CharClass('e'.into()),
            CharClass('l'.into()),
            CharClass('l'.into()),
            CharClass('o'.into()),
        ]),
    );

    // Character classes: ., [...], [^...], [a-z]
    // TODO: Test punctuation, word boundary
    let mut chars_cd: CharBitset = 'c'.into();
    chars_cd.union('d'.into());

    assert_eq!(
        ExpressionAst::new_from_str("cd[cd][^abe-z].[.abc]")
            .unwrap()
            .root,
        Sequence(vec![
            CharClass('c'.into()),
            CharClass('d'.into()),
            CharClass(chars_cd),
            CharClass(chars_cd),
            CharClass(CharBitset::LETTERS),
            CharClass(CharBitset::LETTERS),
        ])
    );

    // Grouping & repetition operators: (...), ?, +, *
    assert_eq!(
        ExpressionAst::new_from_str("a+(b[cd]?)*").unwrap().root,
        Sequence(vec![
            Repetition {
                term: Box::new(CharClass('a'.into())),
                min: 1,
                max: None,
            },
            Repetition {
                term: Box::new(Sequence(vec![
                    CharClass('b'.into()),
                    Repetition {
                        term: Box::new(CharClass(chars_cd)),
                        min: 0,
                        max: Some(1),
                    },
                ])),
                min: 0,
                max: None,
            }
        ])
    );
    assert!(ExpressionAst::new_from_str("a()b").is_err());
    assert!(ExpressionAst::new_from_str("bra(").is_err());
    assert!(ExpressionAst::new_from_str(")ket").is_err());
    assert!(ExpressionAst::new_from_str("a(b[c)d]").is_err());
    assert!(ExpressionAst::new_from_str("(a)(b)(c)((((((((d))))))))").is_ok());

    // Numeric repetition classes: {n}, {n,}, {,m}, {n,m}
    assert_eq!(
        ExpressionAst::new_from_str("a{2}b{3,}c{,4}d{5,6}")
            .unwrap()
            .root,
        Sequence(vec![
            Repetition {
                term: Box::new(CharClass('a'.into())),
                min: 2,
                max: Some(2),
            },
            Repetition {
                term: Box::new(CharClass('b'.into())),
                min: 3,
                max: None,
            },
            Repetition {
                term: Box::new(CharClass('c'.into())),
                min: 0,
                max: Some(4),
            },
            Repetition {
                term: Box::new(CharClass('d'.into())),
                min: 5,
                max: Some(6),
            },
        ])
    );
    assert!(ExpressionAst::new_from_str("{1}abc").is_err());
    assert!(ExpressionAst::new_from_str("a{}bc").is_err());
    assert!(ExpressionAst::new_from_str("a{b}c").is_err());

    // Alternatives: ...|...
    assert_eq!(
        ExpressionAst::new_from_str("a|bc|(d|ef)").unwrap().root,
        Alternatives(vec![
            Sequence(vec![CharClass('a'.into()),]),
            Sequence(vec![CharClass('b'.into()), CharClass('c'.into()),]),
            Sequence(vec![Alternatives(vec![
                Sequence(vec![CharClass('d'.into()),]),
                Sequence(vec![CharClass('e'.into()), CharClass('f'.into()),]),
            ]),]),
        ])
    );

    // Enumerations: 3 4
    assert_eq!(
        ExpressionAst::new_from_str("2 5").unwrap().root,
        Sequence(vec![
            Sequence(vec![
                Repetition {
                    term: Box::new(CharClass(CharBitset::LETTERS)),
                    min: 2,
                    max: Some(2),
                },
                CharClass('_'.into()),
            ]),
            Sequence(vec![
                Repetition {
                    term: Box::new(CharClass(CharBitset::LETTERS)),
                    min: 5,
                    max: Some(5),
                },
                CharClass('_'.into()),
            ]),
        ])
    );

    // Basic anagrams: <abcd>
    assert_eq!(
        ExpressionAst::new_from_str("a<bcb>").unwrap().root,
        Sequence(vec![
            CharClass('a'.into()),
            Anagram {
                kind: AnagramKind::Standard,
                bank: vec!['b'.into(), 'c'.into(), 'b'.into()],
            },
        ])
    );
    assert!(ExpressionAst::new_from_str("a<(bc)d>").is_err());
    assert!(ExpressionAst::new_from_str("a<[bc]d>").is_err());
    assert!(ExpressionAst::new_from_str("a<bc|d>").is_err());
    assert!(ExpressionAst::new_from_str("ab>c").is_err());
    assert!(ExpressionAst::new_from_str("ab<c").is_err());

    // Partial group: a(b(cd):?)
    assert_eq!(
        ExpressionAst::new_from_str("a(b(cd):?)").unwrap().root,
        Sequence(vec![
            CharClass('a'.into()),
            Sequence(vec![
                Repetition {
                    term: Box::new(CharClass('b'.into())),
                    min: 0,
                    max: Some(1),
                },
                Repetition {
                    term: Box::new(Sequence(
                        vec![CharClass('c'.into()), CharClass('d'.into()),]
                    )),
                    min: 0,
                    max: Some(1),
                },
            ]),
        ])
    );
}

#[test]
fn test_expression_options() {
    assert_eq!(
        ExpressionAst::new_from_str("abc").unwrap().options,
        ExpressionOptions {
            explicit_word_boundaries: None,
            explicit_punctuation: None,
            fuzz: None,
        }
    );

    assert_eq!(
        ExpressionAst::new_from_str("[._']").unwrap().options,
        ExpressionOptions {
            explicit_word_boundaries: Some(true),
            explicit_punctuation: Some(true),
            fuzz: None,
        }
    );

    assert_eq!(
        ExpressionAst::new_from_str("3 5").unwrap().options,
        ExpressionOptions {
            explicit_word_boundaries: Some(true),
            explicit_punctuation: None,
            fuzz: None,
        }
    );

    assert_eq!(
        ExpressionAst::new_from_str("can't even").unwrap().options,
        ExpressionOptions {
            explicit_word_boundaries: None,
            explicit_punctuation: Some(true),
            fuzz: None,
        }
    );

    assert_eq!(
        ExpressionAst::new_from_str("exclaim !' !_ !5    ")
            .unwrap()
            .options,
        ExpressionOptions {
            explicit_word_boundaries: Some(true),
            explicit_punctuation: Some(true),
            fuzz: Some(5),
        }
    );

    assert!(ExpressionAst::new_from_str("uh oh!").is_err());
    assert!(ExpressionAst::new_from_str("!_ too early").is_err());
}
