use anyhow::{self as ah, anyhow};
use futures::task::Poll;
use futures::{future, poll, stream, SinkExt, Stream, StreamExt};
use noodle::{load_wordlist, parser, QueryEvaluator, QueryResponse, Word};
use serde::Serialize;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use warp::ws::Message;
use warp::Filter;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    static ref WORDS: Vec<Word> = {
        let args: Vec<_> = std::env::args().collect();
        let wordlist_filename = args.get(1).cloned().unwrap_or("/usr/share/dict/words".to_string());

        let start = Instant::now();
        let words = load_wordlist(wordlist_filename).unwrap();
        println!(" === Time to load wordlist: {:?} ===", start.elapsed());
        words
    };
    static ref WORDLIST: Vec<&'static Word> = {
        let mut wordlist: Vec<&'static Word> = WORDS.iter().collect();
        //wordlist.sort_by_key(|w| &w.chars);
        wordlist.sort();
        wordlist
    };
    static ref ACTIVE_QUERIES: AtomicUsize = AtomicUsize::new(0_usize);
}
static TIMEOUT: Duration = Duration::from_secs(60);
static TIMEOUT_PLAINTEXT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Response {
    Status(String),
    Log { message: String },
    Match { phrase: Vec<Word> },
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
        .take_while(|x| future::ready(x.is_some()))
        .map(|x| x.unwrap())
}

fn flatten_phrase(phrase: Vec<Word>) -> String {
    let response = Response::Match { phrase };
    serde_json::to_string(&response).unwrap()
}

/// Plain HTTP interface, for use with cURL or GSheets IMPORTDATA
fn run_query_sync(query_str: &str, plaintext: bool) -> http::Result<http::Response<hyper::Body>> {
    // TODO: hyper's implementation of "transfer-encoding: chunked" buffers the results of the
    // iterator, and only flushes every ~24 or so items (as 24 separate chunks, at once).
    // (The flushing is not on a timeout, nor on total data size afaict)
    //
    // This makes it hard to predict when the client will actually recieve the separate chunks,
    // which makes this technique mostly insuitable for a noodle frontend
    //
    // TODO: I don't like that this has a lot of the same code as run_websocket,
    // but it's a pain to abstract out streams due to their long types
    let timeout = if plaintext {
        TIMEOUT_PLAINTEXT
    } else {
        TIMEOUT
    };
    let query_ast = parser::QueryAst::new_from_str(query_str);
    let body = match query_ast {
        Ok(mut query_ast) => {
            if plaintext && query_ast.options.results_limit.is_none() {
                query_ast.options.results_limit = Some(15);
            }
            let evaluator = QueryEvaluator::from_ast(&query_ast, &WORDLIST).filter_map(move |m| {
                if let QueryResponse::Match(p) = m {
                    if plaintext {
                        let words: Vec<_> = p.iter().map(|w| w.text.clone()).collect();
                        Some(words.join(" "))
                    } else {
                        Some(flatten_phrase(p))
                    }
                } else {
                    None
                }
            });
            let response_stream = stream::iter(evaluator.map(Ok))
                .chain(stream::once(future::ready(Err("".to_string()))));

            let timeout_stream = stream::once(tokio::time::sleep(timeout))
                .map(move |_| Err(format!("# Timeout after {:?}", timeout)));

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
    let (tx, rx) = websocket.split();
    let mut tx = tx.with(|response| {
        future::ok::<_, warp::Error>(Message::text(serde_json::to_string(&response).unwrap()))
    });
    let mut rx = rx.fuse();

    let r: ah::Result<()> = async {
        // TODO: Using a new websocket for every query may seem a bit excessive, but it
        // has the benefit of being incredibly simple, and avoids the race condition of
        // returning mixing in results from the previous query.
        // We're not really using websockets for any performance reason, it's mostly for
        // the robust framing capabilities (which Warp has good support for).
        let msg = rx
            .next()
            .await
            .map(|m| m.map_err(Into::into))
            .unwrap_or_else(|| Err(anyhow!("Websocket closed without data")))?;

        let n_active = { ACTIVE_QUERIES.fetch_add(1, Ordering::Relaxed) };
        let start = Instant::now();
        tx.send(Response::Status(format!(
            "Running query ({} other(s) active)...",
            n_active
        )))
        .await?;

        let query_ast = parser::QueryAst::new_from_str(msg.to_str().unwrap());

        if let Err(e) = &query_ast {
            tx.send(Response::Status("Query parse error".to_string()))
                .await?;
            tx.send(Response::Log {
                message: e.to_string(),
            })
            .await?;
        }
        let query_ast = query_ast?;
        tx.send(Response::Status(format!(
            "Parsed query in {:?}...",
            start.elapsed()
        )))
        .await?;

        let mut evaluator = QueryEvaluator::from_ast(&query_ast, &WORDLIST);
        for expression in evaluator.expressions() {
            tx.send(Response::Log {
                message: format!("{:?}", expression),
            })
            .await?;
        }

        let mut duration = Duration::from_millis(0);
        loop {
            if let Poll::Ready(None) = poll!(rx.next()) {
                println!(
                    "Computation terminated by client after {:?}",
                    start.elapsed()
                );
                break;
            }

            let now = Instant::now();
            while start + duration < now {
                duration += Duration::from_millis(50);
            }
            if duration > TIMEOUT {
                tx.send(Response::Status(format!(
                    "Timeout after {:?}",
                    start.elapsed()
                )))
                .await?;
                break;
            }
            let deadline = start + duration;
            match evaluator.next_within_deadline(Some(deadline)) {
                QueryResponse::Match(phrase) => tx.send(Response::Match { phrase }).await?,
                QueryResponse::Logs(logs) => {
                    for log in logs {
                        tx.send(Response::Log { message: log }).await?;
                    }
                }
                QueryResponse::Timeout => {
                    tx.send(Response::Status(format!(
                        "Processing, {:0.01}s...: {}",
                        duration.as_secs_f64(),
                        evaluator.progress(),
                    )))
                    .await?
                }
                QueryResponse::Complete(msg) => {
                    tx.send(Response::Status(format!("{} ({:?})", msg, start.elapsed())))
                        .await?;
                    break;
                }
            };
        }

        ACTIVE_QUERIES.fetch_sub(1, Ordering::Relaxed);
        Ok(())
    }
    .await;
    if let Err(e) = r {
        println!("Error: {:?}", e);
    }
    let _ = tx
        .into_inner()
        .reunite(rx.into_inner())
        .unwrap()
        .close()
        .await;
}

#[tokio::main(flavor = "multi_thread", worker_threads = 20)]
async fn main() {
    //pretty_env_logger::init();

    // Static files
    let index = warp::fs::file("index.html")
        .or(warp::fs::file("static/index.html"))
        .or(warp::fs::file("noodle-webapp/static/index.html"));

    // Websockets interface
    let ws = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| ws.on_upgrade(run_websocket));

    // Plain HTTP interface for cURL, GSheets, etc. Available through GET params or POST body
    let get_query = warp::get()
        .and(warp::path("query"))
        .and(warp::path::param())
        .map(|q: String| run_query_sync(&q, true));

    let post_query = warp::post()
        .and(warp::path("query"))
        .and(warp::body::content_length_limit(64 * 1024)) // 64kB
        .and(warp::body::bytes())
        .map(|query_str: bytes::Bytes| {
            run_query_sync(std::str::from_utf8(&query_str).unwrap(), false)
        });

    let routes = get_query.or(post_query).or(ws).or(index);
    let addr = IpAddr::from_str("::0").unwrap();
    warp::serve(routes).run((addr, 8082)).await;
}
