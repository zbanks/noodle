use anyhow::{self as ah, anyhow};
use futures::task::Poll;
use futures::{future, poll, stream, SinkExt, StreamExt};
use noodle::{load_wordlist, parser, QueryEvaluator, QueryResponse, Word};
use percent_encoding::percent_decode_str;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use warp::ws::Message;
use warp::Filter;

#[macro_use]
extern crate lazy_static;

static DEFAULT_WORDLIST: &str = "american-english";

lazy_static! {
    static ref WORDLISTS: HashMap<String, Vec<Word>> = {
        let args: Vec<_> = std::env::args().collect();
        let wordlist_dir = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| "/usr/share/dict".to_string());

        let mut map = HashMap::new();
        for path in std::fs::read_dir(wordlist_dir).unwrap() {
            let path = path.unwrap();
            let ftype = path.file_type().unwrap();
            if !ftype.is_file() || ftype.is_symlink() {
                continue;
            }
            let name = path.file_name().into_string().unwrap();
            let name = name
                .split_once('.')
                .map(|(p, _)| p)
                .unwrap_or(&name)
                .to_string();
            let filepath = path.path();

            let start = Instant::now();
            let words = load_wordlist(&filepath).unwrap();
            println!(
                "Time to load wordlist {name} from {:?}: {:?}",
                filepath,
                start.elapsed()
            );
            map.insert(name, words);
        }
        map
    };
    static ref ACTIVE_QUERIES: AtomicUsize = AtomicUsize::new(0_usize);
    static ref TOTAL_QUERIES: AtomicUsize = AtomicUsize::new(0_usize);
}
static TIMEOUT: Duration = Duration::from_secs(150);
static TIMEOUT_PLAINTEXT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Response {
    Status(String),
    Log { message: String },
    Match { phrase: Vec<Word> },
}

fn flatten_phrase(phrase: Vec<Word>) -> String {
    let response = Response::Match { phrase };
    serde_json::to_string(&response).unwrap()
}

fn get_metrics() -> http::Result<http::Response<hyper::Body>> {
    let mut output = String::new();
    writeln!(
        &mut output,
        "\
# HELP noodle_queries_active
# TYPE noodle_queries_active gauge
noodle_queries_active {}

# HELP noodle_queries_total
# TYPE noodle_queries_total counter
noodle_queries_total {}

# HELP noodle_wordlist_size
# TYPE noodle_wordlist_size gauge",
        ACTIVE_QUERIES.load(Ordering::Relaxed),
        TOTAL_QUERIES.load(Ordering::Relaxed),
    )
    .unwrap();
    for (name, words) in WORDLISTS.iter() {
        writeln!(
            &mut output,
            "noodle_wordlist_size{{wordlist=\"{}\"}} {}",
            name,
            words.len()
        )
        .unwrap();
    }
    http::Response::builder()
        .status(http::StatusCode::OK)
        .body(output.into())
}

fn get_wordlist(name: String) -> http::Result<http::Response<hyper::Body>> {
    let stream = stream::iter(
        words(&name)
            .iter()
            .map(|w| http::Result::Ok(format!("{}\n", w.text))),
    );
    let body = hyper::Body::wrap_stream(stream);
    http::Response::builder()
        .status(http::StatusCode::OK)
        .body(body)
}

fn words(wordlist_name: &str) -> &'static [Word] {
    WORDLISTS
        .get(wordlist_name)
        .unwrap_or_else(|| WORDLISTS.get(DEFAULT_WORDLIST).unwrap())
}

/// Plain HTTP interface, for use with cURL or GSheets IMPORTDATA
fn run_query_sync(query_str: &str, plaintext: bool) -> http::Result<impl warp::Reply> {
    // TODO: hyper's implementation of "transfer-encoding: chunked" buffers the results of the
    // iterator, and only flushes every ~24 or so items (as 24 separate chunks, at once).
    // (The flushing is not on a timeout, nor on total data size afaict)
    //
    // This makes it hard to predict when the client will actually recieve the separate chunks,
    // which makes this technique mostly insuitable for a noodle frontend
    //
    // TODO: I don't like that this has a lot of the same code as run_websocket,
    // but it's a pain to abstract out streams due to their long types
    TOTAL_QUERIES.fetch_add(1, Ordering::Relaxed);
    ACTIVE_QUERIES.fetch_add(1, Ordering::Relaxed);

    let timeout = if plaintext {
        TIMEOUT_PLAINTEXT
    } else {
        TIMEOUT
    };
    let query_ast = parser::QueryAst::new_from_str(query_str);

    let result = match query_ast {
        Ok(mut query_ast) => {
            let mut body = String::new();
            if plaintext && query_ast.options.results_limit.is_none() {
                query_ast.options.results_limit = Some(15);
            }
            let deadline = Instant::now() + timeout;
            let mut evaluator = QueryEvaluator::from_ast(&query_ast, words("default"));
            loop {
                match evaluator.next_within_deadline(Some(deadline)) {
                    QueryResponse::Match(p) => {
                        if plaintext {
                            for word in p.iter() {
                                body.push_str(&word.text);
                                body.push(' ');
                            }
                            body.push('\n');
                        } else {
                            body.push_str(&flatten_phrase(p));
                            body.push('\n');
                        }
                    }
                    QueryResponse::Timeout => {
                        body.push_str(&format!("# Timeout after {:?}\n", timeout));
                        break;
                    }
                    QueryResponse::Logs(_) => {}
                    QueryResponse::Complete(_) => {
                        break;
                    }
                };
            }
            Ok(body)
        }
        Err(error) => Ok(error.to_string()),
    };
    ACTIVE_QUERIES.fetch_sub(1, Ordering::Relaxed);
    result
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

        TOTAL_QUERIES.fetch_add(1, Ordering::Relaxed);
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

        let mut evaluator = QueryEvaluator::from_ast(&query_ast, words("default"));
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

    // Force load of WORDS lazy_static at startup
    //println!("wordlist size {}", WORDS.len());

    // Static files
    let index = warp::fs::file("index.html")
        .or(warp::fs::file("static/index.html"))
        .or(warp::fs::file("noodle-webapp/static/index.html"));
    let statics = warp::fs::dir("static").or(warp::fs::dir("noodle-webapp/static"));

    // Metrics
    let metrics = warp::get().and(warp::path("metrics")).map(get_metrics);

    // Wordlist
    let wordlist = warp::get()
        .and(warp::path("wordlist"))
        .and(warp::path::param())
        .map(get_wordlist);

    // Websockets interface
    let ws = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| ws.on_upgrade(run_websocket));

    // Plain HTTP interface for cURL, GSheets, etc. Available through GET params or POST body
    let get_query = warp::get()
        .and(warp::path("query"))
        .and(warp::path::param())
        .map(|q: String| run_query_sync(&percent_decode_str(&q).decode_utf8_lossy(), true));

    let post_query = warp::post()
        .and(warp::path("query"))
        .and(warp::body::content_length_limit(64 * 1024)) // 64kB
        .and(warp::body::bytes())
        .map(|query_str: bytes::Bytes| {
            run_query_sync(std::str::from_utf8(&query_str).unwrap(), false)
        });

    let routes = get_query
        .or(post_query)
        .or(ws)
        .or(wordlist)
        .or(metrics)
        .or(statics)
        .or(index);
    let addr = IpAddr::from_str("::0").unwrap();
    warp::serve(routes).run((addr, 8082)).await;
}
