use noodle::expression::*;
use noodle::matcher::*;
use noodle::words::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time;

fn main() {
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);
    let wordlist = Rc::new(RefCell::new(wordlist));

    let raw_expressions = ["ex.res*iontest !2", "ex?z?press+[^i].*"];
    //let raw_expressions = ["ex.res*iont(est)? !2", "ex?z?press+([^i].*)"];
    //let raw_expressions = ["h.*", ".e.*", "..l+.", ".*o"];
    //let raw_expressions = ["ex.*res*iontest !2", "ex?z?press+[^i].{,5}"];
    //let raw_expressions = ["expression.*expression"];
    let expressions: Vec<_> = raw_expressions
        .iter()
        .map(|e| Expression::new(e).unwrap())
        .collect();

    //let expressions = vec![Expression::example(0), Expression::example(1)];
    for expr in &expressions {
        println!("Expression: {:?}", expr);
    }

    let start = time::Instant::now();
    let matcher = Matcher::new(&expressions, wordlist.clone(), 3);
    //println!("Matcher: {:?}", matcher);

    for _w in matcher {
        //println!("> {}", _w);
    }
    let duration = start.elapsed();
    println!("Time: {:?}", duration);
}

#[test]
fn expected_count() {
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);
    let wordlist = Rc::new(RefCell::new(wordlist));

    let raw_expressions = ["ex.res*iontest !2", "ex?z?press+[^i].*"];
    let expressions: Vec<_> = raw_expressions
        .iter()
        .map(|e| Expression::new(e).unwrap())
        .collect();

    let matcher = Matcher::new(&expressions, wordlist.clone(), 3);
    assert_eq!(matcher.count(), 1395);
}
