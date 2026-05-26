// Eval harness binary.
//
// Indexes a corpus directory through the real `indexer::run_scan` pipeline,
// then runs each query in `eval-queries.json` through `search_engine::search_files`
// and prints one JSON document to stdout describing the per-query results.
//
// The TS runner (`desktop/scripts/eval.ts`) post-processes that JSON, computes
// metrics from `src/core/metrics.ts`, and writes the timestamped result file.

use std::path::PathBuf;
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use whatthefile_lib::{config, db, indexer, search_engine};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuerySpec {
    query: String,
    #[serde(default)]
    expected_files: Vec<String>,
    #[serde(default)]
    group: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

#[derive(Debug, Deserialize)]
struct QueryFile {
    queries: Vec<QuerySpec>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunMeta {
    sha: String,
    date: String,
    corpus_path: String,
    ollama_url: String,
    queries_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerQuery {
    query: String,
    expected_files: Vec<String>,
    retrieved: Vec<String>,
    scores: Vec<f64>,
    group: Option<String>,
    category: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Output {
    run_meta: RunMeta,
    queries: Vec<PerQuery>,
}

struct Args {
    db: Option<PathBuf>,
    corpus: PathBuf,
    queries: PathBuf,
    sha: String,
    date: String,
    ollama_url: String,
    skip_index: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut db: Option<PathBuf> = None;
    let mut corpus: Option<PathBuf> = None;
    let mut queries: Option<PathBuf> = None;
    let mut sha: Option<String> = None;
    let mut date: Option<String> = None;
    let mut skip_index = false;

    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        let arg = &raw[i];
        let val = || {
            raw.get(i + 1)
                .cloned()
                .ok_or_else(|| format!("missing value for {arg}"))
        };
        match arg.as_str() {
            "--db" => { db = Some(PathBuf::from(val()?)); i += 2; }
            "--corpus" => { corpus = Some(PathBuf::from(val()?)); i += 2; }
            "--queries" => { queries = Some(PathBuf::from(val()?)); i += 2; }
            "--sha" => { sha = Some(val()?); i += 2; }
            "--date" => { date = Some(val()?); i += 2; }
            "--skip-index" => { skip_index = true; i += 1; }
            other => return Err(format!("unknown arg: {other}")),
        }
    }

    let ollama_url = std::env::var("OLLAMA_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    Ok(Args {
        db,
        corpus: corpus.unwrap_or_else(|| PathBuf::from("../test/sample-files")),
        queries: queries.unwrap_or_else(|| PathBuf::from("../test/fixtures/eval-queries.json")),
        sha: sha.unwrap_or_else(|| "unknown".to_string()),
        date: date.unwrap_or_else(|| "unknown".to_string()),
        ollama_url,
        skip_index,
    })
}

fn run() -> Result<(), String> {
    let args = parse_args()?;

    let db_path = args.db.clone().unwrap_or_else(|| {
        std::env::temp_dir().join("wtf-eval.sqlite")
    });

    let corpus_abs = std::fs::canonicalize(&args.corpus)
        .map_err(|e| format!("corpus path invalid {}: {e}", args.corpus.display()))?;

    if args.skip_index {
        if !db_path.exists() {
            return Err(format!("--skip-index: DB not found at {}", db_path.display()));
        }
        eprintln!("eval: reusing existing DB at {} (--skip-index)", db_path.display());
    } else {
        // Hermetic: remove any prior eval DB so each run starts from a clean schema.
        if db_path.exists() {
            std::fs::remove_file(&db_path)
                .map_err(|e| format!("failed to remove prior DB at {}: {e}", db_path.display()))?;
        }
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create DB parent dir: {e}"))?;
        }
    }

    let conn = db::open_and_migrate(&db_path)
        .map_err(|e| format!("open_and_migrate: {e}"))?;

    if !args.skip_index {
        let root = config::add_root(&conn, corpus_abs.to_str().unwrap())
            .map_err(|e| format!("add_root: {e}"))?;

        eprintln!("eval: indexing corpus at {} (root_id={}) ...", corpus_abs.display(), root.id);
        {
            use whatthefile_lib::indexer_progress::{ProgressReporter, ProgressSender, ProgressEvent};
            struct NoOpSender;
            impl ProgressSender for NoOpSender {
                fn send(&self, _: ProgressEvent) {}
            }
            let reporter = ProgressReporter::new(NoOpSender, 0, root.id);
            indexer::run_scan(
                &conn,
                root.id,
                &corpus_abs,
                &args.ollama_url,
                &std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
                &|_event, _payload| { /* no-op */ },
                reporter,
            ).map_err(|e| format!("indexer::run_scan: {e}"))?;
        }
        eprintln!("eval: indexing complete");
    }

    let queries_text = std::fs::read_to_string(&args.queries)
        .map_err(|e| format!("read {}: {e}", args.queries.display()))?;
    let parsed: QueryFile = serde_json::from_str(&queries_text)
        .map_err(|e| format!("parse {}: {e}", args.queries.display()))?;

    let mut out_queries: Vec<PerQuery> = Vec::with_capacity(parsed.queries.len());
    for spec in parsed.queries {
        let q = search_engine::SearchQuery {
            query_text: spec.query.clone(),
            media_types: vec![],
            root_scope: vec![],
            date_from: None,
            date_to: None,
            min_confidence: 0.0,
            limit: 20,
            offset: 0,
            mode: String::new(), // hybrid
            cursor: String::new(),
        };
        let resp = search_engine::search_files(&conn, &q, &args.ollama_url)
            .map_err(|e| format!("search_files for {:?}: {e}", spec.query))?;
        let retrieved: Vec<String> = resp.results.iter().map(|r| r.filename.clone()).collect();
        let scores: Vec<f64> = resp.results.iter().map(|r| r.score).collect();
        out_queries.push(PerQuery {
            query: spec.query,
            expected_files: spec.expected_files,
            retrieved,
            scores,
            group: spec.group,
            category: spec.category,
        });
    }

    let output = Output {
        run_meta: RunMeta {
            sha: args.sha,
            date: args.date,
            corpus_path: corpus_abs.to_string_lossy().into_owned(),
            ollama_url: args.ollama_url,
            queries_path: args.queries.to_string_lossy().into_owned(),
        },
        queries: out_queries,
    };

    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("serialize output: {e}"))?;
    println!("{json}");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("eval: error: {e}");
            ExitCode::FAILURE
        }
    }
}
