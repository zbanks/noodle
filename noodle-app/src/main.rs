use noodle::{load_wordlist, parser, Expression, Matcher, Word};
use std::cell::RefCell;
use std::rc::Rc;
use std::time;

fn main() {
    let start = time::Instant::now();
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);
    let wordlist = Rc::new(RefCell::new(wordlist));
    println!(" === Time to load wordlist: {:?} ===", start.elapsed());

    //let query_str = "ex.res*iontest !2 \n ex?z?press+[^i].*";
    let query_str = r#"
        r#"; test
    #pragma dict words
    toasting{1} !2
    #pragma words 3
    MACRO=<ttes
    MACROi><gn>
    "#;

    let start = time::Instant::now();
    let query = parser::QueryAst::new_from_str(query_str);
    let mut query = query.unwrap();
    query.expand_expressions();

    let expressions: Vec<_> = query
        .expressions
        .iter()
        .map(|expr| Expression::from_ast(expr).unwrap())
        .collect();
    for expr in expressions.iter() {
        println!("Expression: {:?}", expr);
    }
    println!(" === Time to parse query: {:?} ===", start.elapsed());

    let start = time::Instant::now();
    let matcher = Matcher::new(&expressions, wordlist.clone(), 3);
    //println!("Matcher: {:?}", matcher);

    for _w in matcher {
        println!("> {}", _w);
    }
    let duration = start.elapsed();
    println!(" === Time to evaluate matches: {:?} === ", duration);
}
