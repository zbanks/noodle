use noodle::{load_wordlist, parser, Expression, Matcher, Word};
use std::time;

fn main() {
    let start = time::Instant::now();
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);
    println!(" === Time to load wordlist: {:?} ===", start.elapsed());

    let query_str = r#"
    #dict words
    toasting{1} !2
    #words 3
    MACRO=<ttes
    MACROi><gn>
    "#;
    //let query_str = "ex.res*iontest !2; ex?z?press+[^i].*";

    let start = time::Instant::now();
    let query = parser::QueryAst::new_from_str(query_str);
    let mut query = query.unwrap();
    query.expand_expressions();

    let expressions: Vec<_> = query
        .expressions
        .iter()
        .map(|expr| Expression::from_ast(expr).unwrap())
        .collect();

    println!(" === Time to parse query: {:?} ===", start.elapsed());
    for expr in expressions.iter() {
        println!("{:?}", expr);
    }

    let start = time::Instant::now();
    let matcher = Matcher::new(&expressions, &wordlist, 3);

    for _w in matcher {
        println!("> {}", _w);
    }
    let duration = start.elapsed();
    println!(" === Time to evaluate matches: {:?} === ", duration);
}

#[test]
fn expected_count() {
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);

    let raw_expressions = ["ex.res*iontest !2", "ex?z?press+[^i].*"];
    let expressions: Vec<_> = raw_expressions
        .iter()
        .map(|e| Expression::new(e).unwrap())
        .collect();

    let matcher = Matcher::new(&expressions, wordlist.clone(), 3);
    assert_eq!(matcher.count(), 1395);
}
