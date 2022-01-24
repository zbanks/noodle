use anyhow::Result;
use noodle::{load_wordlist_fst, parser, QueryEvaluator, QueryResponse};
use std::path::PathBuf;
use structopt::StructOpt;

//const DEFAULT_WORDLIST_FILE: &str = "/usr/share/dict/words";
const DEFAULT_WORDLIST_FILE: &str = "basic_wordlist.fst";

#[derive(Debug, StructOpt)]
#[structopt(name = "noodle")]
struct Opt {
    /// Input wordlist file
    #[structopt(short, long, parse(from_os_str), default_value=DEFAULT_WORDLIST_FILE)]
    input: PathBuf,

    /// Number of results to return
    #[structopt(short = "n", long)]
    count: Option<usize>,

    /// Maximum number of words to combine to make a matching phrase
    #[structopt(short = "m", long, default_value = "10")]
    phrase_length: usize,

    /// Noodle query string
    #[structopt(name = "query")]
    query: String,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();
    let words = load_wordlist_fst(opt.input)?;
    //noodle::wordlist_to_fst(&words, "wordlist.fst")?;
    let query_ast = parser::QueryAst::new_from_str(&opt.query).unwrap();
    let mut evaluator = QueryEvaluator::from_ast(&query_ast, &words);
    evaluator.set_results_limit(opt.count);
    evaluator.set_search_depth_limit(opt.phrase_length);

    for result in evaluator {
        if let QueryResponse::Match(phrase) = result {
            println!(
                "{}",
                phrase
                    .into_iter()
                    .map(|w| w.text)
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }
    Ok(())
}
