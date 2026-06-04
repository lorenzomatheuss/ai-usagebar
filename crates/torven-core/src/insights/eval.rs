//! Eval metrics — faithfulness + relevance heuristics (Story 1.17).
//!
//! These functions are the unsupervised quality moat for the AI Insights
//! pipeline (PRD §6.1). They run inside the `torven-evals` binary against a
//! 30-case dataset and emit baseline scores that downstream CI gates (Story
//! 1.21) use to block regressions.
//!
//! ## Faithfulness — numeric evidence check
//!
//! **Hypothesis:** a "faithful" insight only mentions numbers that the model
//! actually saw in the input usage payload. If the LLM hallucinates a "$200
//! spent" claim when the snapshot only contains $42, faithfulness drops.
//!
//! **Heuristic:**
//!
//! 1. Extract every numeric token (regex `\d+(?:[.,]\d+)?`) from the output
//!    text (`headline`, every `InsightItem.{message, evidence}`,
//!    `recommendation`).
//! 2. Extract every numeric token from the input `InsightsContext` — totals,
//!    daily arrays, `since_days`, `window_days`.
//! 3. Score = matches / numbers_in_output (or 1.0 if no numbers detected —
//!    vacuously faithful, e.g. the "not enough data yet" guardrail path).
//!
//! **Limitations** (documented but accepted for v1.0):
//!
//! - "$42" vs "42 dollars" — the second form is captured as just "42", but
//!   the context likely also has "42" somewhere in token counts, so the
//!   heuristic over-credits. This biases toward over-estimation of
//!   faithfulness, which is the safer error direction for a baseline.
//! - Compound numbers ("3x more") — the "3" is extracted but the multiplier
//!   semantics are lost. Manual review can catch egregious cases.
//! - Rounded numbers ("approximately 200000") — if the context has 198304
//!   the match fails. Acceptable; the prompt template encourages exact
//!   citation.
//!
//! Future stories may upgrade to LLM-as-judge for category/severity
//! classification eval — that is intentionally NOT in this module's scope.
//!
//! ## Relevance — actionability check
//!
//! **Hypothesis:** a "relevant" insight tells the user what to *do*, not
//! just what *happened*. "Usage grew 3x" is informational; "Consider
//! migrating to Sonnet to save $X" is actionable.
//!
//! **Heuristic:** for each `InsightItem.message`, check whether the
//! lower-cased text contains:
//!
//! - At least one action verb from the curated stop-list (see
//!   [`ACTION_VERBS`] — PT + EN coverage).
//! - At least one numeric token (regex `\d+(?:[.,]\d+)?`).
//!
//! Score = items_with_both / total_items (1.0 for empty lists — vacuously
//! relevant).

use super::schema::{InsightsContext, InsightsOutput};

/// Action verbs recognized by [`compute_relevance`]. Curated to cover both
/// Portuguese and English imperative forms that the v1 prompt template
/// encourages. The list is intentionally non-exhaustive — adding too many
/// verbs inflates the score artificially. Add only when manual review
/// surfaces a legitimately actionable phrasing we missed.
const ACTION_VERBS: &[&str] = &[
    // Portuguese imperatives
    "considere",
    "reduza",
    "mova",
    "agende",
    "migre",
    "defina",
    "habilite",
    "monitore",
    "revise",
    "limite",
    "ajuste",
    "verifique",
    "investigue",
    "configure",
    "pause",
    // English imperatives
    "consider",
    "reduce",
    "move",
    "schedule",
    "migrate",
    "set",
    "enable",
    "monitor",
    "review",
    "limit",
    "adjust",
    "check",
    "investigate",
    "configure",
    "pause",
    "cap",
    "route",
    "raise",
    "lower",
    "correlate",
    "confirm",
    "switch",
];

/// Extract every numeric token from the given text. Returns the matches as
/// owned strings so the caller does not need to keep `text` alive.
///
/// Matches: `42`, `3.14`, `1,500`, `0.50`. Misses currency prefixes (kept as
/// adjacent characters) and units (handled by the caller).
fn extract_numbers(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == ',')
            {
                i += 1;
            }
            // Strip trailing punctuation that wasn't actually part of a
            // number (e.g. "42." at end of sentence).
            let mut end = i;
            while end > start {
                let last = chars[end - 1];
                if last == '.' || last == ',' {
                    end -= 1;
                } else {
                    break;
                }
            }
            if end > start {
                out.push(chars[start..end].iter().collect());
            }
        } else {
            i += 1;
        }
    }
    out
}

/// Normalize a numeric token for comparison: drop trailing `.0` / `,0`
/// fractional parts that came from `f64`-style serialization in the
/// context, and collapse comma/dot grouping to a canonical decimal form.
fn normalize_number(s: &str) -> String {
    // Normalize separators: treat both `,` and `.` as decimal candidates.
    // Strip thousands grouping by checking if there are multiple separators.
    let dots = s.matches('.').count();
    let commas = s.matches(',').count();
    let mut canonical: String = if dots > 1 || commas > 1 {
        // Likely grouping like "1,000,000" — drop separators entirely.
        s.chars().filter(|c| c.is_ascii_digit()).collect()
    } else {
        // Single optional separator — keep as decimal.
        s.replace(',', ".")
    };
    // Strip trailing zeros after a decimal point, then the dot itself.
    if canonical.contains('.') {
        while canonical.ends_with('0') {
            canonical.pop();
        }
        if canonical.ends_with('.') {
            canonical.pop();
        }
    }
    canonical
}

/// Gather all numeric tokens from an [`InsightsContext`] (totals, daily
/// arrays, window sizes). Returned as normalized strings ready for
/// containment checks.
fn context_numbers(context: &InsightsContext) -> Vec<String> {
    let mut nums = Vec::new();
    nums.push(context.since_days.to_string());
    for v in &context.usage_payload {
        nums.push(v.window_days.to_string());
        nums.push(v.total_tokens.to_string());
        nums.push(format!("{}", v.total_cost_usd));
        for t in &v.daily_tokens {
            nums.push(t.to_string());
        }
        for c in &v.daily_cost_usd {
            nums.push(format!("{c}"));
        }
    }
    nums.into_iter().map(|n| normalize_number(&n)).collect()
}

/// Gather all numeric tokens from the LLM output (headline + every
/// insight's message/evidence + recommendation). Returned as normalized
/// strings.
fn output_numbers(output: &InsightsOutput) -> Vec<String> {
    let mut text = String::new();
    text.push_str(&output.headline);
    text.push(' ');
    for item in &output.insights {
        text.push_str(&item.message);
        text.push(' ');
        text.push_str(&item.evidence);
        text.push(' ');
    }
    text.push_str(&output.recommendation);
    extract_numbers(&text)
        .into_iter()
        .map(|n| normalize_number(&n))
        .collect()
}

/// Compute the faithfulness score for an [`InsightsOutput`] against its
/// originating [`InsightsContext`].
///
/// Returns a value in `[0.0, 1.0]`. `1.0` indicates every numeric claim in
/// the output has direct evidence in the context (or the output contains no
/// numbers at all — vacuously faithful, e.g. the "no data yet" guardrail).
///
/// Target baseline: ≥ 0.85 (PRD §6.1).
pub fn compute_faithfulness(output: &InsightsOutput, context: &InsightsContext) -> f64 {
    let claims = output_numbers(output);
    if claims.is_empty() {
        return 1.0;
    }
    let evidence = context_numbers(context);
    let total = claims.len();
    let matches = claims
        .iter()
        .filter(|c| evidence.iter().any(|e| e == *c))
        .count();
    matches as f64 / total as f64
}

/// Compute the relevance score for an [`InsightsOutput`] — fraction of
/// insight items whose `message` is actionable.
///
/// "Actionable" = contains both an action verb (from [`ACTION_VERBS`]) and
/// at least one numeric token.
///
/// Returns a value in `[0.0, 1.0]`. `1.0` for an empty insight list
/// (vacuously relevant) or when every item is actionable.
///
/// Target baseline: ≥ 0.80 (PRD §6.1).
pub fn compute_relevance(output: &InsightsOutput) -> f64 {
    if output.insights.is_empty() {
        return 1.0;
    }
    let total = output.insights.len();
    let actionable = output
        .insights
        .iter()
        .filter(|i| {
            let lower = i.message.to_lowercase();
            let has_verb = ACTION_VERBS.iter().any(|v| lower.contains(v));
            let has_number = !extract_numbers(&lower).is_empty();
            has_verb && has_number
        })
        .count();
    actionable as f64 / total as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::insights::schema::{InsightItem, InsightItemType, InsightSeverity, VendorAggregate};

    fn ctx_with_numbers() -> InsightsContext {
        InsightsContext::new(vec![VendorAggregate {
            vendor_id: "anthropic".to_string(),
            account_id: Some("hash1234".to_string()),
            total_tokens: 1200000,
            total_cost_usd: 36.0,
            window_days: 7,
            daily_tokens: vec![160000, 170000, 180000, 175000, 170000, 175000, 170000],
            daily_cost_usd: vec![4.8, 5.1, 5.4, 5.25, 5.1, 5.25, 5.1],
            tag: None,
        }])
    }

    fn empty_output() -> InsightsOutput {
        InsightsOutput {
            headline: "Test".to_string(),
            insights: vec![],
            recommendation: "ok".to_string(),
            cost_usd: 0.0,
            latency_ms: 0,
            prompt_version: "v1".to_string(),
        }
    }

    #[test]
    fn test_faithfulness_perfect() {
        // Every number in the output appears in the context.
        let out = InsightsOutput {
            headline: "Total tokens 1200000 across 7 days".to_string(),
            insights: vec![InsightItem {
                type_: InsightItemType::Trend,
                severity: InsightSeverity::Info,
                message: "Daily peak 180000 tokens".to_string(),
                evidence: "Window 7 days at 36 dollars".to_string(),
            }],
            recommendation: "Monitor 7 day cycle".to_string(),
            cost_usd: 0.01,
            latency_ms: 100,
            prompt_version: "v1".to_string(),
        };
        let score = compute_faithfulness(&out, &ctx_with_numbers());
        assert!(score >= 0.99, "expected perfect faithfulness, got {score}",);
    }

    #[test]
    fn test_faithfulness_zero() {
        // Output cites numbers that are NOT in the context.
        let out = InsightsOutput {
            headline: "Spend hit 999 dollars over 42 weeks".to_string(),
            insights: vec![InsightItem {
                type_: InsightItemType::BudgetRisk,
                severity: InsightSeverity::Critical,
                message: "Pay 555 immediately".to_string(),
                evidence: "Based on 88 tokens".to_string(),
            }],
            recommendation: "Cap at 777".to_string(),
            cost_usd: 0.01,
            latency_ms: 100,
            prompt_version: "v1".to_string(),
        };
        let score = compute_faithfulness(&out, &ctx_with_numbers());
        // None of 999, 42, 555, 88, 777 appear in ctx_with_numbers.
        assert!(score <= 0.01, "expected zero faithfulness, got {score}");
    }

    #[test]
    fn test_faithfulness_vacuous_when_no_numbers() {
        // Output with no numbers should score 1.0 (vacuously faithful).
        let out = InsightsOutput {
            headline: "Looks fine".to_string(),
            insights: vec![],
            recommendation: "Carry on".to_string(),
            cost_usd: 0.0,
            latency_ms: 0,
            prompt_version: "v1".to_string(),
        };
        assert_eq!(compute_faithfulness(&out, &ctx_with_numbers()), 1.0);
    }

    #[test]
    fn test_relevance_actionable() {
        // Three items, all with action verb + number — score should be 1.0.
        let out = InsightsOutput {
            headline: "h".to_string(),
            insights: vec![
                InsightItem {
                    type_: InsightItemType::OptimizationOpportunity,
                    severity: InsightSeverity::Info,
                    message: "Consider migrating 60 percent of workload".to_string(),
                    evidence: "e".to_string(),
                },
                InsightItem {
                    type_: InsightItemType::BudgetRisk,
                    severity: InsightSeverity::Warning,
                    message: "Reduce spend by 20 dollars weekly".to_string(),
                    evidence: "e".to_string(),
                },
                InsightItem {
                    type_: InsightItemType::Trend,
                    severity: InsightSeverity::Info,
                    message: "Monitor 7 day cycle".to_string(),
                    evidence: "e".to_string(),
                },
            ],
            recommendation: "r".to_string(),
            cost_usd: 0.0,
            latency_ms: 0,
            prompt_version: "v1".to_string(),
        };
        let score = compute_relevance(&out);
        assert!(score >= 0.99, "expected full relevance, got {score}");
    }

    #[test]
    fn test_relevance_missing_action_verb() {
        // Message has number but no action verb -> not relevant.
        let out = InsightsOutput {
            headline: "h".to_string(),
            insights: vec![InsightItem {
                type_: InsightItemType::Trend,
                severity: InsightSeverity::Info,
                message: "Tokens hit 1500".to_string(),
                evidence: "e".to_string(),
            }],
            recommendation: "r".to_string(),
            cost_usd: 0.0,
            latency_ms: 0,
            prompt_version: "v1".to_string(),
        };
        assert_eq!(compute_relevance(&out), 0.0);
    }

    #[test]
    fn test_relevance_missing_number() {
        // Has action verb but no number -> not relevant.
        let out = InsightsOutput {
            headline: "h".to_string(),
            insights: vec![InsightItem {
                type_: InsightItemType::OptimizationOpportunity,
                severity: InsightSeverity::Info,
                message: "Consider optimizing the queries".to_string(),
                evidence: "e".to_string(),
            }],
            recommendation: "r".to_string(),
            cost_usd: 0.0,
            latency_ms: 0,
            prompt_version: "v1".to_string(),
        };
        assert_eq!(compute_relevance(&out), 0.0);
    }

    #[test]
    fn test_relevance_empty_list_is_vacuous() {
        // No insights -> 1.0 (no irrelevance to score against).
        assert_eq!(compute_relevance(&empty_output()), 1.0);
    }

    #[test]
    fn test_extract_numbers_basic() {
        let nums = extract_numbers("spent 42 dollars on 1500 tokens, total 3.14");
        assert_eq!(nums, vec!["42", "1500", "3.14"]);
    }

    #[test]
    fn test_extract_numbers_strips_trailing_punct() {
        let nums = extract_numbers("It hit 100. Then 200.");
        assert_eq!(nums, vec!["100", "200"]);
    }

    #[test]
    fn test_normalize_drops_trailing_zero() {
        assert_eq!(normalize_number("36.0"), "36");
        assert_eq!(normalize_number("5.10"), "5.1");
        assert_eq!(normalize_number("1,000,000"), "1000000");
    }
}
