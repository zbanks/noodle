use noodle::{load_wordlist, parser, Matcher, Word};
use std::time;

fn main() {
    let start = time::Instant::now();
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);
    println!(" === Time to load wordlist: {:?} ===", start.elapsed());

    let queries = vec![
        "helloworld",
        "h.... _ w....; <hello+>; <world+5>; [hale]+<owl>.*",
        "<smiles>",
        "<smiles>; .*ss.*",
        "ahumongoussentencewithmultiplewords",
        "ahumongoussentincewithmultiplewords !' !1",
        "3 3 8 7; (LOOHNEWHOOPCRLOVAIDYTILEAUQWOSLLPEASSOEHNCS:?) !'",
        "hen !1; hay !1",
        "breadfast !2",
    ];
    for query_str in queries {
        println!();
        println!();
        println!(">>> Query: {} <<<", query_str);

        let start = time::Instant::now();
        let query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
        let matcher = Matcher::from_ast(&query_ast, &wordlist);
        println!(" === Time to parse query: {:?} ===", start.elapsed());

        //let start = time::Instant::now();
        //for _w in matcher {
        //    println!("> {}", _w);
        //}
        println!("# matches: {}", matcher.count());
        let duration = start.elapsed();
        println!(" === Time to evaluate matches: {:?} === ", duration);
    }
}

#[test]
fn expected_count() {
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);

    let query_str = "ex.res*iontest !2 !'; ex?z?press+[^i].* !'; #words 3";
    let mut query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
    query_ast.options.results_limit = Some(2000);
    let matcher = Matcher::from_ast(&query_ast, &wordlist);

    assert_eq!(matcher.count(), 1395);
}
