use noodle::{load_wordlist, parser, QueryEvaluator, QueryResponse, Word};
use std::time;

fn main() {
    let start = time::Instant::now();
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort();
    println!(" === Time to load wordlist: {:?} ===", start.elapsed());
    let queries = vec![
        ("helloworld", 1..=1, 13),
        ("8; [aehl]+([lo]*w[lo]*).*", 37..=40, 230),
        (
            "h.... _ w....; <hello+>; <world+5>; [hale]+<owl>.*",
            10..=10,
            140,
        ),
        ("<smiles>", 300..=10000, 15),
        ("<smiles>; .*ss.*", 120..=140, 26),
        ("ahumongoussentencewithmultiplewords", 10..=10, 40),
        ("ahumongoussentincewithmultiplewords !' !1", 265..=275, 800),
        (
            "3 3 8 7; (LOOHNEWHOOPCRLOVAIDYTILEAUQWOSLLPEASSOEHNCS:?) !'",
            24..=24,
            690,
        ),
        //(
        //    "(.{4,8}_){4}; .{20,}; (LOOHNEWHOOPCRLOVAIDYTILEAUQWOSLLPEASSOEHNCS:?) !'",
        //    300..=10000,
        //    830,
        //),
        ("hen !1; hay !1", 2..=2, 11),
        ("breadfast !2", 300..=10000, 70),
    ];
    let mut times = vec![];
    for (query_str, expected_range, _) in queries.iter() {
        println!();
        println!();
        println!(">>> Query: {} <<<", query_str);

        let start = time::Instant::now();
        let query_ast = parser::QueryAst::new_from_str(query_str).unwrap();

        let evaluator = QueryEvaluator::from_ast(&query_ast, &wordlist);
        println!(" === Time to parse query: {:?} ===", start.elapsed());
        let mut results = evaluator.filter(|m| matches!(m, QueryResponse::Match(_)));
        //let mut results = results.map(|m| println!("{:?}", m));

        let first_match = results.next();
        let first_time = start.elapsed();
        let count = (first_match.is_some() as usize) + results.count();
        println!("# matches: {}", count);
        let duration = start.elapsed();
        println!(
            " === Time to evaluate matches: {:?} (first in {:?}) === ",
            duration, first_time
        );

        if !expected_range.contains(&count) {
            println!(
                "error: query {:?} expected {:?} matches, got {}",
                query_str, expected_range, count
            );
            break;
        }
        times.push((first_time, duration));
    }
    for ((query_str, _, expected_time_ms), (first_time, duration)) in
        queries.iter().zip(times.iter())
    {
        println!(
            "{:64} {:-4}ms -> {:?} (first in {:?})",
            query_str, expected_time_ms, duration, first_time
        );
    }
}

#[test]
fn expected_count() {
    let words = load_wordlist("/usr/share/dict/words").unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort();

    let query_str = "ex.res*iontest !2 !'; ex?z?press+[^i].* !'; #words 3";
    let mut query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
    query_ast.options.results_limit = Some(2000);
    let evaluator = QueryEvaluator::from_ast(&query_ast, &wordlist);

    let count = evaluator
        .filter(|m| matches!(m, QueryResponse::Match(_)))
        //.map(|x| { println!("match: {:?}", x); x})
        .count();
    assert_eq!(count, 1395);
}
