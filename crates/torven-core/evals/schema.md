# Torven AI Insights Eval Dataset — Schema

This document describes the structure of `dataset.jsonl` and the semantics of
the quality metrics (`faithfulness`, `relevance`) that the `torven-evals`
runner computes against it.

Story 1.17 establishes this dataset and runner as the first quality moat for
the AI Insights pipeline (PRD §6.1).

---

## File format

`dataset.jsonl` is one JSON object per line (JSONL). Each line is a single
labeled eval case. There are exactly **30** cases in v1 (PRD §6.1 minimum).

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Stable identifier, format `eval-NNN` (e.g. `eval-001`). Used in failure reports. |
| `usage_snapshot` | object | Privacy-redacted `InsightsContext` payload that the runner feeds to `LlmClient::request_insight_streaming`. Schema mirrors `crates/torven-core/src/insights/schema.rs::InsightsContext`. |
| `expected_category` | enum | One of `trend`, `anomaly`, `budget_risk`, `optimization_opportunity`. The category we expect at least one insight item to surface. |
| `expected_severity` | enum | One of `info`, `warning`, `critical`. The severity we expect on the primary insight item. |
| `partial_ideal_response` | string | A human-readable phrasing of the insight we expect. Used for `faithfulness` (numbers from the snapshot should appear) and `relevance` (action verb + concrete number) heuristics. |

### `usage_snapshot` shape

The `usage_snapshot` object is fed directly to the LLM client as an
`InsightsContext`. It contains:

- `usage_payload`: array of `VendorAggregate` objects (vendor_id, account_id,
  total_tokens, total_cost_usd, window_days, daily_tokens, daily_cost_usd,
  tag).
- `since_days`: integer (typically `7`).
- `prompt_version`: string (always `"v1"` in this dataset).

**Privacy invariant** (PRD §6.5): `account_id` is an 8-character hash, never a
plaintext account name. No `api_key` fields appear anywhere in the dataset.

---

## Distribution (v1 dataset)

By category:

| Category | Count |
|----------|-------|
| `trend` | 10 |
| `anomaly` | 8 |
| `budget_risk` | 7 |
| `optimization_opportunity` | 5 |
| **Total** | **30** |

By severity:

| Severity | Count |
|----------|-------|
| `info` | 15 |
| `warning` | 10 |
| `critical` | 5 |
| **Total** | **30** |

Vendor coverage: each case uses one or more of `anthropic`, `openai`,
`openrouter`, `zai`. The empty-payload edge case (`eval-030`) verifies
"no data yet" handling.

---

## Metric semantics

### Faithfulness

> **Definition**: fraction of numeric claims in the LLM output that have
> direct evidence in the input `usage_snapshot`.

**Heuristic** (`insights::eval::compute_faithfulness`):

1. Extract every number (integer or decimal) from the concatenated output
   text (`headline`, every `InsightItem.message`, every `InsightItem.evidence`,
   `recommendation`).
2. Extract every number from the input `InsightsContext` (totals, daily
   arrays, `since_days`, `window_days`).
3. Score = `(matches found in context) / (numbers detected in output)`.
   Returns `1.0` when the output mentions no numbers (vacuously faithful).

**Target**: ≥ 0.85 per PRD §6.1. Regression beyond 5% blocks merge (CI gate,
Story 1.21).

### Relevance

> **Definition**: fraction of `InsightItem`s whose `message` is *actionable*
> — contains both an action verb and a concrete number.

**Heuristic** (`insights::eval::compute_relevance`):

1. For each `InsightItem`, lower-case the `message`.
2. Check if it contains at least one action verb (a short stop-list:
   `considere`, `consider`, `reduza`, `reduce`, `mova`, `move`, `agende`,
   `schedule`, `migrate`, `migre`, `set`, `defina`, `enable`, `habilite`,
   `route`, `route`, `cap`, `pause`, `monitor`, `monitore`, `review`,
   `revise`, etc. — see source for the complete list).
3. Check if the message contains at least one decimal or integer number
   (regex `\d+(?:[.,]\d+)?`).
4. Score = `(items with both action verb AND number) / total items`.
   Returns `1.0` for empty insight lists (vacuously relevant — no items, no
   irrelevance).

**Target**: ≥ 0.80 per PRD §6.1.

### Latency

Wall-clock time from `request_insight_streaming(...)` start to
`Result::Ok(InsightsOutput)`, measured with `std::time::Instant::now()`.
Reported as `p50` and `p95` across the 30 cases.

### Cost

Estimated USD cost per case, computed by `insights::budget::estimate_cost`:

- Input tokens estimated as `chars / 4` over (rendered prompt + serialized
  context).
- Output tokens estimated from the final `InsightsOutput` JSON size /4.
- Multiplied by `claude-3-5-sonnet-20241022` pricing: $3/1M input, $15/1M
  output (verify quarterly).

Reported as `p50` and `p95`. Target: ≤ $0.05 per insight.

---

## How to add a new case

1. Append a new line to `dataset.jsonl` with a fresh `id` (next free
   `eval-NNN`).
2. Keep the `usage_snapshot` privacy-clean: only hashed `account_id`s, no
   `api_key`s, no plaintext account names. The 8-character hash format
   matches what `RealAnthropicClient::redact_payload` produces in production.
3. Choose `expected_category` + `expected_severity` honestly — these are
   labels the runner does not currently assert against (v1 metrics focus on
   faithfulness + relevance, both unsupervised). Future stories may add
   category-classification eval that uses these labels.
4. Write `partial_ideal_response` to include at least one concrete number
   from the snapshot AND at least one action verb — that is what good
   insights look like and helps anchor manual review.
5. Re-run `cargo run --bin torven-evals` and re-publish the baseline if the
   distribution shifts materially.

**Do NOT** add cases that require real-time data (tokens-per-second,
clock-time-dependent recommendations). The dataset is meant to be
deterministic and reproducible across Mock and Real LLM modes.

---

## References

- `docs/prd/torven-v1.md` §6.1 (eval pipeline targets)
- `docs/prd/torven-v1.md` §6.2 (prompt versioning + `eval_baseline_score`)
- `docs/architecture/torven-v1-adr.md` §ADR-8 (eval runner native Rust)
- `crates/torven-core/src/insights/eval.rs` (metric implementations)
- `crates/torven-core/src/bin/torven-evals.rs` (runner binary)
