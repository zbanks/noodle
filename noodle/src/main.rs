use noodle::{load_wordlist, parser, Matcher, Word};
use std::time;

fn main() {
    let start = time::Instant::now();
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);
    println!(" === Time to load wordlist: {:?} ===", start.elapsed());

    let queries = vec![
        ("helloworld", 1..=1),
        ("8; [aehl]+([lo]*w[lo]*).*", 37..=40),
        (
            "h.... _ w....; <hello+>; <world+5>; [hale]+<owl>.*",
            10..=10,
        ),
        ("<smiles>", 300..=10000),
        ("<smiles>; .*ss.*", 120..=140),
        ("ahumongoussentencewithmultiplewords", 10..=10),
        ("ahumongoussentincewithmultiplewords !' !1", 265..=275),
        (
            "3 3 8 7; (LOOHNEWHOOPCRLOVAIDYTILEAUQWOSLLPEASSOEHNCS:?) !'",
            24..=24,
        ),
        ("hen !1; hay !1", 2..=2),
        ("breadfast !2", 300..=10000),
    ];
    for (query_str, expected_range) in queries {
        println!();
        println!();
        println!(">>> Query: {} <<<", query_str);

        let start = time::Instant::now();
        let query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
        let matcher = Matcher::from_ast(&query_ast, &wordlist);
        println!(" === Time to parse query: {:?} ===", start.elapsed());

        //let start = time::Instant::now();
        //let matcher: Vec<String> = matcher.collect();
        //for w in matcher.iter() {
        //    println!("> {}", w);
        //}
        //let count = matcher.len();

        let count = matcher.count();
        println!("# matches: {}", count);
        let duration = start.elapsed();
        println!(" === Time to evaluate matches: {:?} === ", duration);

        if !expected_range.contains(&count) {
            println!(
                "error: query {:?} expected {:?} matches, got {}",
                query_str, expected_range, count
            );
            break;
        }
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

    assert_eq!(matcher.filter(|m| m == MatcherResponse::Match(_)).count(), 1395);
}
