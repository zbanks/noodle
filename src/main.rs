use noodle::expression::*;
use noodle::matcher::*;
use noodle::words::*;
use std::time;

fn main() {
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);

    let raw_expressions = ["ex.res*iontest !2", "ex?z?press+[^i].*"];
    let expressions: Vec<_> = raw_expressions
        .iter()
        .map(|e| Expression::new(e).unwrap())
        .collect();

    let expressions = vec![Expression::example(0), Expression::example(1)];
    println!("Expression: {:?}", expressions[0]);
    println!("Expression: {:?}", expressions[1]);

    let start = time::Instant::now();
    let mut matcher = Matcher::new(&expressions, wordlist.as_slice(), 3);
    //println!("Matcher: {:?}", matcher);

    while let Some(w) = matcher.next_match() {
        //println!("> {}", w);
    }
    let duration = start.elapsed();
    println!("Time: {:?}", duration);
}
