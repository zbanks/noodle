use noodle::{load_wordlist, parser, Matcher, Word};
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
    4 3
    "#;

    let query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
    let matcher = Matcher::from_ast(&query_ast, &wordlist);
    println!(" === Time to parse query: {:?} ===", start.elapsed());

    let start = time::Instant::now();
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

    let raw_query = "ex.res*iontest !2; ex?z?press+[^i].*; #words 3";
    let query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
    query_ast.expand_expressions();
    let matcher = Matcher::from_ast(&query_ast, &wordlist);

    assert_eq!(matcher.count(), 1395);
}
