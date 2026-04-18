//! Golden-set retrieval evaluation harness.
//!
//! Given a JSON dataset of (question, context, expected_message_ids) tuples,
//! runs the production FTS retrieval path against a real tunaFlow DB and
//! reports **recall@K** and **precision@K** per entry plus aggregates.
//!
//! Usage:
//! ```bash
//! cargo run --release --bin eval_retrieval -- \
//!     --db ~/.tunaflow/db/tunaflow.db \
//!     --set docs/eval/golden-set.json \
//!     --k 5
//! ```
//!
//! Exit code: 0 on success, non-zero when dataset is malformed. Recall scores
//! are printed to stdout in machine-parseable `metric=value` format so CI can
//! compare before/after without parsing a table.
//!
//! Golden entry schema (see `docs/eval/golden-set.json` for examples):
//! ```json
//! {
//!   "id": "q1",
//!   "question": "이전에 refactoring 얘기할 때 어느 파일 얘기했지?",
//!   "context": { "conversation_id": "conv-abc123" },
//!   "expected_message_ids": ["msg-def456", "msg-ghi789"],
//!   "notes": "optional human description"
//! }
//! ```

use rusqlite::Connection;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;

use tuna_flow_lib::commands::context_queries::retrieve_relevant_chunks_with_overlap;

#[derive(Debug, Deserialize)]
struct GoldenContext {
    conversation_id: String,
}

#[derive(Debug, Deserialize)]
struct GoldenEntry {
    id: String,
    question: String,
    context: GoldenContext,
    expected_message_ids: Vec<String>,
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoldenSet {
    version: u32,
    entries: Vec<GoldenEntry>,
}

#[derive(Debug, Default)]
struct Metrics {
    recall: f64,
    precision: f64,
    retrieved_count: usize,
    expected_count: usize,
    hits: usize,
}

fn evaluate_entry(conn: &Connection, project_key: &str, entry: &GoldenEntry, k: i64) -> Metrics {
    let chunks = retrieve_relevant_chunks_with_overlap(
        conn,
        project_key,
        &entry.context.conversation_id,
        &entry.question,
        &[],
        k,
        None,
    );

    // Collect message_ids from retrieved chunks. `RetrievedChunk.messages` is
    // `Vec<(role, content, ...)>` with no id field, so we proxy via conversation_id
    // + timestamp — expected_message_ids refer to *root* message ids though, so
    // exact match is best-effort here until chunks carry a stable id.
    // Workaround: compare on the original message rowids via a fresh query.
    let retrieved_ids: HashSet<String> = chunks
        .iter()
        .flat_map(|c| {
            let ts = c.timestamp;
            let conv = c.conversation_id.clone();
            conn.prepare(
                "SELECT id FROM messages WHERE conversation_id = ?1 AND timestamp = ?2",
            )
            .ok()
            .map(|mut stmt| {
                stmt.query_map(rusqlite::params![conv, ts], |row| row.get::<_, String>(0))
                    .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<String>>())
                    .unwrap_or_default()
            })
            .unwrap_or_default()
        })
        .collect();

    let expected: HashSet<String> = entry.expected_message_ids.iter().cloned().collect();
    let hits = retrieved_ids.intersection(&expected).count();

    Metrics {
        recall: if expected.is_empty() { 0.0 } else { hits as f64 / expected.len() as f64 },
        precision: if retrieved_ids.is_empty() { 0.0 } else { hits as f64 / retrieved_ids.len() as f64 },
        retrieved_count: retrieved_ids.len(),
        expected_count: expected.len(),
        hits,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut db_path: Option<PathBuf> = None;
    let mut set_path: Option<PathBuf> = None;
    let mut k: i64 = 5;
    let mut project_key: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--db" => { db_path = args.get(i + 1).map(PathBuf::from); i += 2; }
            "--set" => { set_path = args.get(i + 1).map(PathBuf::from); i += 2; }
            "--k" => { k = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(5); i += 2; }
            "--project-key" => { project_key = args.get(i + 1).cloned(); i += 2; }
            "-h" | "--help" => {
                eprintln!("usage: eval_retrieval --db <path> --set <path> [--k N] [--project-key KEY]");
                return Ok(());
            }
            _ => {
                eprintln!("unknown arg: {}", args[i]);
                i += 1;
            }
        }
    }

    let db_path = db_path.ok_or("--db is required")?;
    let set_path = set_path.ok_or("--set is required")?;

    let conn = Connection::open_with_flags(
        &db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.execute_batch("PRAGMA busy_timeout = 5000;")?;

    let text = std::fs::read_to_string(&set_path)?;
    let set: GoldenSet = serde_json::from_str(&text)?;

    // Resolve project_key: explicit arg > first entry's conversation lookup.
    let pk = match project_key {
        Some(pk) => pk,
        None => {
            let first = set.entries.first().ok_or("dataset is empty")?;
            conn.query_row(
                "SELECT project_key FROM conversations WHERE id = ?1",
                [&first.context.conversation_id],
                |row| row.get::<_, String>(0),
            )?
        }
    };

    println!("# eval_retrieval v{} — dataset: {} entries, k={}", set.version, set.entries.len(), k);
    println!("# project_key={}", pk);
    println!("id\tretrieved\texpected\thits\trecall\tprecision\tnotes");

    let mut agg = Metrics::default();
    let mut n = 0;
    for entry in &set.entries {
        let m = evaluate_entry(&conn, &pk, entry, k);
        println!(
            "{}\t{}\t{}\t{}\t{:.3}\t{:.3}\t{}",
            entry.id,
            m.retrieved_count,
            m.expected_count,
            m.hits,
            m.recall,
            m.precision,
            entry.notes.as_deref().unwrap_or(""),
        );
        agg.recall += m.recall;
        agg.precision += m.precision;
        agg.hits += m.hits;
        agg.retrieved_count += m.retrieved_count;
        agg.expected_count += m.expected_count;
        n += 1;
    }

    if n > 0 {
        let avg_recall = agg.recall / n as f64;
        let avg_precision = agg.precision / n as f64;
        println!();
        println!("recall_at_k={:.3}", avg_recall);
        println!("precision_at_k={:.3}", avg_precision);
        println!("total_hits={}", agg.hits);
        println!("total_expected={}", agg.expected_count);
        println!("total_retrieved={}", agg.retrieved_count);
    }
    Ok(())
}
