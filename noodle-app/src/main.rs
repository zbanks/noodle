use futures::stream::{self, Stream, StreamExt};
use noodle::{load_wordlist, parser, Expression, Matcher, Word};
use std::cell::RefCell;
use std::rc::Rc;
use std::time;
use warp::Filter;

// I'm being so dumb here
enum DeferredIterator<T, I: Iterator<Item = T>, A, F: Copy + FnOnce(&A) -> I> {
    Function(A, F),
    Iter(I),
}

impl<T, I: Iterator<Item = T>, A, F: Copy + FnOnce(&A) -> I> DeferredIterator<T, I, A, F> {
    fn new(a: A, f: F) -> Self {
        Self::Function(a, f)
    }
}

impl<T, I: Iterator<Item = T>, A, F: Copy + FnOnce(&A) -> I> Iterator
    for DeferredIterator<T, I, A, F>
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if let Self::Function(a, f) = self {
            *self = Self::Iter(f(a));
        }
        if let Self::Iter(i) = self {
            i.next()
        } else {
            unreachable!();
        }
    }
}
fn run_query_sync(query_str: &str) -> http::Result<http::Response<hyper::Body>> {
    let query_str = query_str.to_string();

    // TODO: hyper's implementation of "transfer-encoding: chunked" buffers the results of the
    // iterator, and only flushes every ~24 or so items (as 24 separate chunks, at once).
    // (The flushing is not on a timeout, nor on total data size afaict)
    //
    // This makes it hard to predict when the client will actually recieve the separate chunks,
    // which makes this technique mostly insuitable for a noodle frontend
    //
    // TODO: Separately, my string building code here is pretty bad
    let result_stream = stream::iter(DeferredIterator::new(query_str, |query_str| {
        let start = time::Instant::now();
        let words = load_wordlist("/usr/share/dict/words").unwrap();
        let mut wordlist: Vec<&Word> = words.iter().collect();
        wordlist.sort_by_key(|w| &w.chars);
        println!(" === Time to load wordlist: {:?} ===", start.elapsed());

        let start = time::Instant::now();
        let mut query_ast = parser::QueryAst::new_from_str(query_str).unwrap();
        query_ast.expand_expressions();

        let expressions: Vec<_> = query_ast
            .expressions
            .iter()
            .map(|expr| Expression::from_ast(expr).unwrap())
            .collect();
        for expr in expressions.iter() {
            println!("Expression: {:?}", expr);
        }
        println!(" === Time to parse query: {:?} ===", start.elapsed());
        let matcher = Matcher::new(&expressions, &wordlist, 3);
        matcher
            .map(|m| {
                format!("{}", m)
                //http::Result::Ok(format!("{}\n", m))
            }) // TODO: how to "rinse" this iterator of the Matcher type?
            .collect::<Vec<_>>()
            .into_iter()
    }))
    .ready_chunks(16)
    .map(|ms| http::Result::Ok(format!("{}\n", ms.join("\n"))));

    let body = hyper::Body::wrap_stream(result_stream);
    http::Response::builder()
        .status(http::StatusCode::OK)
        .body(body)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    //pretty_env_logger::init();

    let get_query = warp::get()
        .and(warp::path("query"))
        .and(warp::path::param::<String>())
        .map(|query_str: String| {
            println!("query: {}", query_str);
            run_query_sync(&query_str)
        });

    let post_query = warp::post()
        .and(warp::path("query"))
        .and(warp::body::content_length_limit(64 * 1024)) // 64kB
        .and(warp::body::bytes())
        .map(|query_str: bytes::Bytes| {
            println!("query: {:?}", query_str);
            run_query_sync(std::str::from_utf8(&query_str).unwrap())
        });

    let routes = get_query.or(post_query);
    warp::serve(routes).run(([127, 0, 0, 1], 8081)).await;
}
