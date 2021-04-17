use std::path::PathBuf;
use structopt::StructOpt;
use noodle::{load_wordlist, parser, QueryEvaluator, QueryResponse, Word};

const DEFAULT_WORDLIST_FILE: &str = "/usr/share/dict/words";

#[derive(Debug, StructOpt)]
#[structopt(name="noodle")]
struct Opt {
    /// Input wordlist file
    #[structopt(short, long, parse(from_os_str), default_value=DEFAULT_WORDLIST_FILE)]
    input: PathBuf,

    /// Number of results to return
    #[structopt(short = "n", long)]
    count: Option<usize>,

    /// Maximum number of words to combine to make a matching phrase
    #[structopt(short = "m", long, default_value="10")]
    phrase_length: usize,

    /// Noodle query string
    #[structopt(name = "query")]
    query: String,
}

fn main() {
    let opt = Opt::from_args();

    let words = load_wordlist(opt.input).unwrap();
    let mut wordlist: Vec<&Word> = words.iter().collect();
    wordlist.sort();

    let query_ast = parser::QueryAst::new_from_str(&opt.query).unwrap();
    let mut evaluator = QueryEvaluator::from_ast(&query_ast, &wordlist);
    evaluator.set_results_limit(opt.count);
    evaluator.set_search_depth_limit(opt.phrase_length);

    for result in evaluator {
        match result {
            QueryResponse::Match(phrase) => {
                println!("{}", phrase.into_iter().map(|w| w.text).collect::<Vec<_>>().join(" "));
            },
            _ => {},
        };
    }
}
