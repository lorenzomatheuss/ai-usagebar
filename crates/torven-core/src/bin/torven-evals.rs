//! torven-evals — offline evaluation harness for the AI Insights pipeline
//! (Story 1.17).
//!
//! Reads a JSONL dataset of labeled cases (default
//! `crates/torven-core/evals/dataset.jsonl`), runs each case through an
//! `LlmClient` implementation (Mock by default, Real Anthropic with
//! `--real-llm`), and emits a Markdown report with faithfulness, relevance,
//! latency p50/p95, and cost p50/p95 across the dataset.
//!
//! Targets (PRD §6.1):
//! - Faithfulness ≥ 0.85
//! - Relevance    ≥ 0.80
//! - Cost p95     ≤ $0.05
//!
//! ## Usage
//!
//! ```bash
//! # default: MockLlmClient::for_eval, prints report to stdout
//! cargo run -p torven-core --bin torven-evals -- \
//!   --dataset crates/torven-core/evals/dataset.jsonl
//!
//! # save to file
//! cargo run -p torven-core --bin torven-evals -- \
//!   --dataset crates/torven-core/evals/dataset.jsonl \
//!   --output crates/torven-core/evals/results/report-2026-06-04.md
//!
//! # real Anthropic API (requires ANTHROPIC_API_KEY env var)
//! cargo run -p torven-core --bin torven-evals -- --real-llm
//! ```

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use clap::Parser;
use serde::Deserialize;

use torven_core::insights::RealAnthropicClient;
use torven_core::insights::budget::estimate_cost;
use torven_core::insights::cancel::CancelHandle;
use torven_core::insights::eval::{compute_faithfulness, compute_relevance};
use torven_core::insights::llm_client::{InsightsCallback, LlmClient, MockLlmClient};
use torven_core::insights::schema::{InsightsContext, InsightsOutput};
use torven_core::runtime::get_runtime;

const FAITHFULNESS_TARGET: f64 = 0.85;
const RELEVANCE_TARGET: f64 = 0.80;
const COST_TARGET_USD: f64 = 0.05;

/// CLI arguments. See module docs for end-user usage examples.
#[derive(Parser, Debug)]
#[command(
    name = "torven-evals",
    about = "Offline eval runner for the AI Insights pipeline.",
    long_about = "Runs a labeled JSONL dataset through MockLlmClient (default) or RealAnthropicClient and emits a Markdown report with faithfulness, relevance, latency, and cost metrics. See crates/torven-core/evals/schema.md for dataset shape."
)]
struct Args {
    /// Path to the JSONL dataset.
    #[arg(long, default_value = "crates/torven-core/evals/dataset.jsonl")]
    dataset: PathBuf,

    /// Optional path to write the Markdown report to (defaults to stdout).
    #[arg(long)]
    output: Option<PathBuf>,

    /// Use `RealAnthropicClient` instead of `MockLlmClient::for_eval`.
    /// Requires `ANTHROPIC_API_KEY` env var.
    #[arg(long, default_value_t = false)]
    real_llm: bool,
}

/// One row of `dataset.jsonl`. The runner only consumes `id` and
/// `usage_snapshot` directly; the other fields are labels for documentation
/// and future supervised-eval extensions (Story 1.21+).
#[derive(Debug, Deserialize)]
struct EvalCase {
    id: String,
    usage_snapshot: InsightsContext,
    expected_category: String,
    expected_severity: String,
    #[serde(default)]
    partial_ideal_response: String,
}

/// Per-case run output, retained for the report.
struct CaseResult {
    id: String,
    expected_category: String,
    expected_severity: String,
    faithfulness: f64,
    relevance: f64,
    latency_ms: u128,
    cost_usd: f64,
    error: Option<String>,
    /// Brief failure reason (for the report's `Failures` section). `None`
    /// when the case meets BOTH the faithfulness and relevance targets.
    failure_reason: Option<String>,
    ideal: String,
}

/// No-op callback — the eval runner does not surface streaming progress.
/// Token chunks are consumed silently; the final `InsightsOutput` is what
/// the metrics scoring uses.
struct SilentCallback;
impl InsightsCallback for SilentCallback {
    fn on_token(&self, _token: String) {}
    fn on_error(&self, _error: String) {}
}

fn main() {
    let args = Args::parse();
    let cases = match load_dataset(&args.dataset) {
        Ok(c) => c,
        Err(e) => {
            eprintln!(
                "torven-evals: failed to load dataset {:?}: {}",
                args.dataset, e
            );
            std::process::exit(2);
        }
    };

    if cases.is_empty() {
        eprintln!("torven-evals: dataset is empty");
        std::process::exit(2);
    }

    // The eval runner uses the same FFI-owned tokio runtime that the
    // production app uses (runtime::get_runtime). Story 1.15's runtime is
    // single-threaded; that's fine for the eval pipeline because we run
    // cases sequentially — concurrency would only confuse the latency
    // numbers we report.
    let rt = get_runtime();

    // Box one of two LlmClient impls. We use Arc<dyn LlmClient> because the
    // trait requires `Arc<dyn InsightsCallback>` already, and keeping the
    // client behind the same shape lets us share the per-case loop below.
    let client: Arc<dyn LlmClient> = if args.real_llm {
        let key = std::env::var("ANTHROPIC_API_KEY").unwrap_or_else(|_| {
            eprintln!("torven-evals: --real-llm requires ANTHROPIC_API_KEY env var");
            std::process::exit(2);
        });
        Arc::new(RealAnthropicClient::new(key))
    } else {
        // The Mock client is per-case (derived from the case's context), so
        // we can't construct it once here — we'd need one per loop
        // iteration. Use a sentinel that we replace in the loop.
        // We pick the "current case's context" client inside the loop
        // below; for `--real-llm` we share a single client across cases.
        Arc::new(MockLlmClient::for_eval(&cases[0].usage_snapshot))
    };

    let mut results = Vec::with_capacity(cases.len());
    for case in &cases {
        // Build the per-case mock client (the real client is shared).
        let case_client: Arc<dyn LlmClient> = if args.real_llm {
            client.clone()
        } else {
            Arc::new(MockLlmClient::for_eval(&case.usage_snapshot))
        };

        let cancel = CancelHandle::new_arc();
        let cb: Arc<dyn InsightsCallback> = Arc::new(SilentCallback);
        let ctx = case.usage_snapshot.clone();
        let start = Instant::now();
        let out_result = rt.block_on(case_client.request_insight_streaming(ctx, cb, cancel));
        let elapsed = start.elapsed();

        let (faith, rel, cost, err, output) = match out_result {
            Ok(out) => {
                let f = compute_faithfulness(&out, &case.usage_snapshot);
                let r = compute_relevance(&out);
                let c = estimate_per_case_cost(&case.usage_snapshot, &out);
                (f, r, c, None, Some(out))
            }
            Err(e) => (0.0, 0.0, 0.0, Some(e.to_string()), None),
        };

        let failure_reason = match (&err, &output) {
            (Some(e), _) => Some(format!("error: {e}")),
            (None, Some(_)) => {
                if faith < FAITHFULNESS_TARGET {
                    Some(format!(
                        "faithfulness {:.2} < {:.2}",
                        faith, FAITHFULNESS_TARGET
                    ))
                } else if rel < RELEVANCE_TARGET {
                    Some(format!("relevance {:.2} < {:.2}", rel, RELEVANCE_TARGET))
                } else {
                    None
                }
            }
            (None, None) => Some("no output, no error (impossible)".to_string()),
        };

        results.push(CaseResult {
            id: case.id.clone(),
            expected_category: case.expected_category.clone(),
            expected_severity: case.expected_severity.clone(),
            faithfulness: faith,
            relevance: rel,
            latency_ms: elapsed.as_millis(),
            cost_usd: cost,
            error: err,
            failure_reason,
            ideal: case.partial_ideal_response.clone(),
        });
    }

    let llm_label = if args.real_llm {
        "Real Anthropic claude-3-5-sonnet-20241022"
    } else {
        "Mock"
    };
    let report = build_report(&results, &args.dataset, llm_label);

    match args.output.as_ref() {
        Some(path) => {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(e) = fs::write(path, &report) {
                eprintln!("torven-evals: failed to write report to {path:?}: {e}");
                std::process::exit(2);
            }
            eprintln!("torven-evals: wrote report to {}", path.display());
        }
        None => {
            print!("{report}");
        }
    }
}

/// Load and parse the JSONL dataset. Each non-blank line is parsed as one
/// [`EvalCase`].
fn load_dataset(path: &Path) -> Result<Vec<EvalCase>, String> {
    let body = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let case: EvalCase =
            serde_json::from_str(line).map_err(|e| format!("line {}: {}", i + 1, e))?;
        out.push(case);
    }
    Ok(out)
}

/// Estimate cost for a single case. Uses `chars/4` token estimation across
/// the rendered context + the produced output, then applies Sonnet pricing
/// via [`estimate_cost`]. Matches the heuristic used by `budget::estimate_input_tokens`.
fn estimate_per_case_cost(ctx: &InsightsContext, out: &InsightsOutput) -> f64 {
    let ctx_chars = serde_json::to_string(ctx).map(|s| s.len()).unwrap_or(0);
    let out_chars = serde_json::to_string(out).map(|s| s.len()).unwrap_or(0);
    let input_tokens = ctx_chars.div_ceil(4) as u32;
    let output_tokens = out_chars.div_ceil(4) as u32;
    estimate_cost(input_tokens, output_tokens, "claude-3-5-sonnet-20241022")
}

/// Compute the p-th percentile of a slice of f64s. Sorts a clone (the input
/// is a `&[f64]` — read-only). `p` is in `[0.0, 1.0]`.
fn percentile_f64(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Same as [`percentile_f64`] but for `u128` (latency in ms).
fn percentile_u128(values: &[u128], p: f64) -> u128 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted: Vec<u128> = values.to_vec();
    sorted.sort();
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Render the Markdown report. Format matches the AC-5 example exactly so
/// CI gates (Story 1.21) can grep for the table.
fn build_report(results: &[CaseResult], dataset_path: &Path, llm_label: &str) -> String {
    let n = results.len();
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let faiths: Vec<f64> = results.iter().map(|r| r.faithfulness).collect();
    let rels: Vec<f64> = results.iter().map(|r| r.relevance).collect();
    let lats: Vec<u128> = results.iter().map(|r| r.latency_ms).collect();
    let costs: Vec<f64> = results.iter().map(|r| r.cost_usd).collect();

    let faith_mean = faiths.iter().sum::<f64>() / n as f64;
    let rel_mean = rels.iter().sum::<f64>() / n as f64;
    let lat_p50 = percentile_u128(&lats, 0.50);
    let lat_p95 = percentile_u128(&lats, 0.95);
    let cost_p50 = percentile_f64(&costs, 0.50);
    let cost_p95 = percentile_f64(&costs, 0.95);

    let faith_status = if faith_mean >= FAITHFULNESS_TARGET {
        "PASS"
    } else {
        "FAIL"
    };
    let rel_status = if rel_mean >= RELEVANCE_TARGET {
        "PASS"
    } else {
        "FAIL"
    };
    let cost_p50_status = if cost_p50 <= COST_TARGET_USD {
        "PASS"
    } else {
        "FAIL"
    };
    let cost_p95_status = if cost_p95 <= COST_TARGET_USD {
        "PASS"
    } else {
        "FAIL"
    };

    let passed = results
        .iter()
        .filter(|r| r.failure_reason.is_none())
        .count();
    let failed = n - passed;

    let mut s = String::new();
    s.push_str("# Torven AI Insights Eval Report\n");
    s.push_str(&format!("**Date:** {date}\n"));
    s.push_str(&format!(
        "**Dataset:** {} ({n} cases)\n",
        dataset_path.display()
    ));
    s.push_str(&format!("**LLM:** {llm_label}\n\n"));
    s.push_str(&format!(
        "**Cases:** {n} total, {passed} passed, {failed} failed\n\n"
    ));

    s.push_str("## Results\n\n");
    s.push_str("| Metric | Score | Target | Status |\n");
    s.push_str("|--------|-------|--------|--------|\n");
    s.push_str(&format!(
        "| Faithfulness | {:.2} | >= {:.2} | {} |\n",
        faith_mean, FAITHFULNESS_TARGET, faith_status
    ));
    s.push_str(&format!(
        "| Relevance | {:.2} | >= {:.2} | {} |\n",
        rel_mean, RELEVANCE_TARGET, rel_status
    ));
    s.push_str(&format!("| Latency p50 | {lat_p50}ms | --- | INFO |\n"));
    s.push_str(&format!("| Latency p95 | {lat_p95}ms | --- | INFO |\n"));
    s.push_str(&format!(
        "| Cost p50 | ${:.4} | <= ${:.2} | {} |\n",
        cost_p50, COST_TARGET_USD, cost_p50_status
    ));
    s.push_str(&format!(
        "| Cost p95 | ${:.4} | <= ${:.2} | {} |\n",
        cost_p95, COST_TARGET_USD, cost_p95_status
    ));
    s.push('\n');

    s.push_str("## Failures\n\n");
    let failures: Vec<&CaseResult> = results
        .iter()
        .filter(|r| r.failure_reason.is_some())
        .collect();
    if failures.is_empty() {
        s.push_str("(none)\n\n");
    } else {
        for f in &failures {
            s.push_str(&format!(
                "- **{id}** ({cat}/{sev}): {reason}\n",
                id = f.id,
                cat = f.expected_category,
                sev = f.expected_severity,
                reason = f.failure_reason.as_deref().unwrap_or("(no reason)")
            ));
        }
        s.push('\n');
    }

    s.push_str("## Per-Case Detail\n\n");
    s.push_str("| ID | Category | Severity | Faith | Rel | Latency | Cost | Status |\n");
    s.push_str("|----|----------|----------|-------|-----|---------|------|--------|\n");
    for r in results {
        let status = if r.failure_reason.is_none() {
            "PASS"
        } else {
            "FAIL"
        };
        s.push_str(&format!(
            "| {id} | {cat} | {sev} | {faith:.2} | {rel:.2} | {lat}ms | ${cost:.4} | {status} |\n",
            id = r.id,
            cat = r.expected_category,
            sev = r.expected_severity,
            faith = r.faithfulness,
            rel = r.relevance,
            lat = r.latency_ms,
            cost = r.cost_usd,
        ));
    }

    s.push_str("\n## Notes\n\n");
    s.push_str("- `MockLlmClient::for_eval` synthesizes outputs deterministically from each case's context. Faithfulness and relevance metrics are reproducible across runs.\n");
    s.push_str("- Latency on the mock path reflects synchronous synthesis (~milliseconds). The `--real-llm` path measures actual Anthropic API round-trips.\n");
    s.push_str("- Cost is estimated via `chars/4` token counting and Sonnet pricing ($3/$15 per 1M input/output tokens).\n");

    // Surface any explicit errors at the bottom for ops triage.
    let errors: Vec<&CaseResult> = results.iter().filter(|r| r.error.is_some()).collect();
    if !errors.is_empty() {
        s.push_str("\n## Errors\n\n");
        for e in &errors {
            s.push_str(&format!(
                "- {id}: {err}\n",
                id = e.id,
                err = e.error.as_deref().unwrap_or("(unknown)")
            ));
        }
    }

    // Ideal response references for manual review.
    s.push_str("\n## Ideal references (for manual review)\n\n");
    for r in results.iter().take(3) {
        s.push_str(&format!("- {}: {}\n", r.id, r.ideal));
    }
    s.push_str("- (truncated — see dataset.jsonl for full list)\n");
    s
}
