use noodle::{load_wordlist, parser, Matcher, MatcherResponse, Word};
use std::time;

fn main() {
    let start = time::Instant::now();
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort_by_key(|w| &w.chars);
    println!(" === Time to load wordlist: {:?} ===", start.elapsed());
    let queries = vec![
        ("helloworld", 1..=1, 13),
        ("8; [aehl]+([lo]*w[lo]*).*", 37..=40, 204),
        (
            "h.... _ w....; <hello+>; <world+5>; [hale]+<owl>.*",
            10..=10,
            132,
        ),
        ("<smiles>", 300..=10000, 14),
        ("<smiles>; .*ss.*", 120..=140, 16),
        ("ahumongoussentencewithmultiplewords", 10..=10, 40),
        ("ahumongoussentincewithmultiplewords !' !1", 265..=275, 382),
        (
            "3 3 8 7; (LOOHNEWHOOPCRLOVAIDYTILEAUQWOSLLPEASSOEHNCS:?) !'",
            24..=24,
            508,
        ),
        ("hen !1; hay !1", 2..=2, 10),
        ("breadfast !2", 300..=10000, 128),
    ];
    let mut times = vec![];
    for (query_str, expected_range, expected_time_ms) in queries.iter() {
        println!();
        println!();
        println!(">>> Query: {} <<<", query_str);

        let start = time::Instant::now();
        let query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
        let matcher = Matcher::from_ast(&query_ast, &wordlist);
        println!(" === Time to parse query: {:?} ===", start.elapsed());

        let count = matcher
            .filter(|m| matches!(m, MatcherResponse::Match(_)))
            //.map(|x| { println!("match: {:?}", x); x})
            .count();
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
        times.push(duration) //, time::Duration::from_millis(expected_time_ms)));
    }
    for ((query_str, _, expected_time_ms), duration) in queries.iter().zip(times.iter()) {
        println!(
            "{:64} {:-4}ms -> {:?}",
            query_str, expected_time_ms, duration
        );
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

    let count = matcher
        .filter(|m| matches!(m, MatcherResponse::Match(_)))
        //.map(|x| { println!("match: {:?}", x); x})
        .count();
    assert_eq!(count, 1395);
}
