use futures::stream::{self, StreamExt};
use noodle::{load_wordlist, parser, Matcher, Word};
use std::time;
use warp::Filter;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref WORDS: Vec<Word> = {
        let start = time::Instant::now();
        let words = load_wordlist("/usr/share/dict/words").unwrap();
        println!(" === Time to load wordlist: {:?} ===", start.elapsed());
        words
    };
    static ref WORDLIST: Vec<&'static Word> = {
        let mut wordlist: Vec<&'static Word> = WORDS.iter().collect();
        wordlist.sort_by_key(|w| &w.chars);
        wordlist
    };
}

fn run_query_sync(query_str: &str) -> http::Result<http::Response<hyper::Body>> {
    // TODO: hyper's implementation of "transfer-encoding: chunked" buffers the results of the
    // iterator, and only flushes every ~24 or so items (as 24 separate chunks, at once).
    // (The flushing is not on a timeout, nor on total data size afaict)
    //
    // This makes it hard to predict when the client will actually recieve the separate chunks,
    // which makes this technique mostly insuitable for a noodle frontend
    let start = time::Instant::now();
    let query_ast = parser::QueryAst::new_from_str(query_str);
    let body = match query_ast {
        Ok(query_ast) => {
            let matcher = Matcher::from_ast(&query_ast, &WORDLIST);
            println!(" === Time to parse query: {:?} ===", start.elapsed());

            let response = std::iter::once("#0 Running query...\n#1 ".to_string())
                .chain(matcher)
                .chain(std::iter::once("#0 Done".to_string()));

            // TODO: The string building code here is pretty bad
            let result_stream = stream::iter(response)
                .ready_chunks(1) // TODO
                .map(|ms| Result::<_, String>::Ok(format!("{}\n", ms.join("\n"))));

            hyper::Body::wrap_stream(result_stream)
        }
        Err(error) => error.to_string().into(),
    };

    http::Response::builder()
        .status(http::StatusCode::OK)
        .body(body)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    //pretty_env_logger::init();

    let index = warp::fs::file("index.html")
        .or(warp::fs::file("static/index.html"))
        .or(warp::fs::file("noodle-app/static/index.html"));

    let get_query = warp::get()
        .and(warp::path("query"))
        .and(warp::path::param::<String>())
        .map(|q: String| run_query_sync(&q));

    let post_query = warp::post()
        .and(warp::path("query"))
        .and(warp::body::content_length_limit(64 * 1024)) // 64kB
        .and(warp::body::bytes())
        .map(|query_str: bytes::Bytes| run_query_sync(std::str::from_utf8(&query_str).unwrap()));

    let routes = get_query.or(post_query).or(index);
    warp::serve(routes).run(([127, 0, 0, 1], 8081)).await;
}
