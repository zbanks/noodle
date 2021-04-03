use either::Either;
use futures::{stream, SinkExt, Stream, StreamExt};
use noodle::{load_wordlist, parser, Matcher, Word};
use serde::Serialize;
use std::sync::Mutex;
use std::time::{self, Duration};
use warp::ws::Message;
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
    static ref ACTIVE_QUERIES: Mutex<usize> = Mutex::new(0_usize);
}
static TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Response<'a> {
    Status(&'a str),
    Log { message: &'a str, level: usize },
    Match { phrase: &'a [&'a Word] },
}

/// Convert a `Stream` on `Result<T, T>` items into a `Stream` on `T`.
/// Returns items from the input stream while they are `Ok`, then returns
/// the contents of the *first* `Err`, then stops
fn stream_until_error<T, S>(stream: S) -> impl Stream<Item = T>
where
    S: Stream<Item = Result<T, T>>,
{
    // TODO: This is primarily used for implementing timeouts, and forwarding that
    // error to the end user. Is there a more elegant way of doing this?
    stream
        .flat_map(|r| match r {
            Ok(v) => stream::once(async { Some(v) }).left_stream(),
            Err(e) => stream::iter(vec![Some(e), None]).right_stream(),
        })
        .take_while(|x| futures::future::ready(x.is_some()))
        .map(|x| x.unwrap())
}

fn flatten_phrase(phrase: Vec<&Word>) -> String {
    let response = Response::Match { phrase: &phrase };
    serde_json::to_string(&response).unwrap()
}

/// Plain HTTP interface, for use with cURL or GSheets IMPORTDATA
fn run_query_sync(query_str: &str) -> http::Result<http::Response<hyper::Body>> {
    // TODO: hyper's implementation of "transfer-encoding: chunked" buffers the results of the
    // iterator, and only flushes every ~24 or so items (as 24 separate chunks, at once).
    // (The flushing is not on a timeout, nor on total data size afaict)
    //
    // This makes it hard to predict when the client will actually recieve the separate chunks,
    // which makes this technique mostly insuitable for a noodle frontend
    //
    // TODO: I don't like that this has a lot of the same code as run_websocket,
    // but it's a pain to abstract out streams due to their long types
    let query_ast = parser::QueryAst::new_from_str(query_str);
    let body = match query_ast {
        Ok(query_ast) => {
            let matcher = Matcher::from_ast(&query_ast, &WORDLIST).map(flatten_phrase);
            let response_stream = stream::iter(matcher.map(Ok));

            let timeout_stream = stream::once(tokio::time::sleep(TIMEOUT))
                .map(|_| Err(format!("# Timeout after {:?}", TIMEOUT)));

            // TODO: The string building code here is pretty bad
            let result_stream = stream_until_error(stream::select(response_stream, timeout_stream))
                .map(|ms| Result::<_, String>::Ok(format!("{}\n", ms)));
            hyper::Body::wrap_stream(result_stream)
        }
        Err(error) => error.to_string().into(),
    };

    http::Response::builder()
        .status(http::StatusCode::OK)
        .body(body)
}

/// Websockets interface, for interactive use
async fn run_websocket(websocket: warp::ws::WebSocket) {
    let (mut tx, mut rx) = websocket.split();

    // TODO: Using a new websocket for every query may seem a bit excessive, but it
    // has the benefit of being incredibly simple, and avoids the race condition of
    // returning mixing in results from the previous query.
    // We're not really using websockets for any performance reason, it's mostly for
    // the robust framing capabilities (which Warp has good support for).
    if let Some(result) = rx.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                println!("websocket error: {:?}", e);
                return;
            }
        };

        let n_active = {
            let mut aq = ACTIVE_QUERIES.lock().unwrap();
            *aq += 1;
            *aq
        };
        let start = time::Instant::now();
        let r = tx
            .send(Message::text(
                serde_json::to_string(&Response::Status(&format!(
                    "Running query ({} other(s) active)...",
                    n_active - 1
                )))
                .unwrap(),
            ))
            .await;
        if let Err(e) = r {
            println!("websocket send error: {:?}", e);
            return;
        }

        let query_ast = parser::QueryAst::new_from_str(msg.to_str().unwrap());
        let body = match query_ast {
            Ok(query_ast) => {
                let header = std::iter::once_with(|| {
                    serde_json::to_string(&Response::Status(&format!(
                        "Parsed query in {:?}...",
                        start.elapsed()
                    )))
                    .unwrap()
                });
                let matcher = Matcher::from_ast(&query_ast, &WORDLIST).map(flatten_phrase);
                let footer = std::iter::once_with(|| {
                    serde_json::to_string(&Response::Status(&format!(
                        "Finished in {:?}",
                        start.elapsed()
                    )))
                    .unwrap()
                });

                Either::Left(header.chain(matcher).map(Ok).chain(footer.map(Err)))
            }
            Err(error) => {
                println!("err: {:?}", error);
                Either::Right(std::iter::once(Err(serde_json::to_string(
                    &Response::Status(&error.to_string()),
                )
                .unwrap())))
            }
        };
        let body = stream::iter(body);
        let timeout_stream = stream::once(tokio::time::sleep(TIMEOUT)).map(|_| {
            Err(serde_json::to_string(&Response::Status(&format!(
                "Timeout after {:?}",
                start.elapsed()
            )))
            .unwrap())
        });

        let _ = stream_until_error(stream::select(body, timeout_stream))
            .map(|r| Ok(Message::text(r)))
            .forward(&mut tx)
            .await;

        *ACTIVE_QUERIES.lock().unwrap() -= 1;
    }
    let _ = tx.reunite(rx).unwrap().close().await;
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    //pretty_env_logger::init();

    // Static files
    let index = warp::fs::file("index.html")
        .or(warp::fs::file("static/index.html"))
        .or(warp::fs::file("noodle-app/static/index.html"));

    // Websockets interface
    let ws = warp::path("ws")
        .and(warp::ws())
        .map(|ws: warp::ws::Ws| ws.on_upgrade(run_websocket));

    // Plain HTTP interface for cURL, GSheets, etc. Available through GET params or POST body
    let get_query = warp::get()
        .and(warp::path("query"))
        .and(warp::path::param::<String>())
        .map(|q: String| run_query_sync(&q));

    let post_query = warp::post()
        .and(warp::path("query"))
        .and(warp::body::content_length_limit(64 * 1024)) // 64kB
        .and(warp::body::bytes())
        .map(|query_str: bytes::Bytes| run_query_sync(std::str::from_utf8(&query_str).unwrap()));

    let routes = get_query.or(post_query).or(ws).or(index);
    warp::serve(routes).run(([127, 0, 0, 1], 8082)).await;
}
