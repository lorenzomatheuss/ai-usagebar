//! Cross-platform semantic metrics extracted from a [`VendorSnapshot`].
//!
//! The legacy `format.rs` produced Pango-markup strings tailored to Waybar.
//! Story 1.4 replaces it with [`RawMetrics`] — a strings-free struct that
//! every UI surface can format on its own terms:
//!
//! - The SwiftUI app reads these fields directly via the UniFFI bridge and
//!   styles them with semantic color tokens defined in `Assets.xcassets`.
//! - The developer TUI (`crates/torven-tui`) projects them onto ratatui
//!   gauges with severity-driven colors (see `format_tui.rs`).
//! - Future surfaces (CLI, menu-bar text, AI Insights prompts) can consume
//!   the same struct without re-implementing snapshot semantics.
//!
//! Reference: docs/architecture/preservation-map.md#migrate, ADR-1 §4.

use crate::usage::VendorSnapshot;

/// Plain semantic snapshot of "where are we against the plan?".
///
/// All fields are optional because vendors expose genuinely different shapes
/// (e.g. OpenRouter reports balance in USD; Anthropic reports a percentage
/// of a 5h window; Z.AI may report neither when no buckets are active).
/// Consumers decide what to render based on which fields are populated and
/// the [`LabelKind`] hint.
#[derive(Debug, Clone, PartialEq)]
pub struct RawMetrics {
    /// Cost spent in the current billing window, expressed in US dollars.
    /// `None` when the vendor reports usage as percentages only.
    pub cost_usd: Option<f64>,

    /// Percentage of the relevant usage window consumed (0..=100 as a float
    /// to allow sub-percent precision on the SwiftUI side). For multi-window
    /// vendors (Anthropic, OpenAI), this is the **worst-of** the windows so
    /// the menu-bar label tracks the most urgent metric.
    pub pct_used: Option<f64>,

    /// Approximate tokens consumed. Most vendors don't expose this directly,
    /// so it's typically `None` — kept for the Z.AI / future vendors that do.
    pub tokens_used: Option<u64>,

    /// Hint about what `pct_used` / `cost_usd` semantically *mean*. The
    /// SwiftUI app uses this to pick the right unit string ("%", "$",
    /// "messages", etc.) and the AI Insights pipeline uses it as context.
    pub label_kind: LabelKind,
}

/// Semantic hint about which metric is the "headline" for this vendor.
///
/// Variants intentionally avoid presentation concerns — no colors, no
/// strings. The UI layer maps a `LabelKind` to its own theming.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelKind {
    /// Headline is a percentage of a rolling window (Anthropic 5h/weekly,
    /// OpenAI Codex 5h/7d, Z.AI buckets).
    PercentOfWindow,
    /// Headline is a remaining-messages count (OpenAI Plus when credit
    /// balance + per-message approximation is the most useful signal).
    MessagesQuota,
    /// Headline is dollars spent vs. a credit balance (OpenRouter).
    UsdSpent,
    /// Vendor requires OAuth/API-key linkage that hasn't happened yet —
    /// the UI should prompt for credentials instead of rendering a metric.
    OAuthUnlinked,
}

/// Project a vendor snapshot onto the cross-platform [`RawMetrics`] shape.
///
/// This is the single source of truth for "what is the headline metric for
/// vendor X?". Renderers (SwiftUI, TUI, CLI) consume the result instead of
/// each reimplementing the snapshot-to-display projection.
pub fn compute_metrics(snapshot: &VendorSnapshot) -> RawMetrics {
    match snapshot {
        VendorSnapshot::Anthropic(s) => {
            // Worst-of the three Anthropic windows for the headline percentage.
            let mut max = s.session.utilization_pct;
            if s.weekly.utilization_pct > max {
                max = s.weekly.utilization_pct;
            }
            if let Some(sonnet) = &s.sonnet {
                if sonnet.utilization_pct > max {
                    max = sonnet.utilization_pct;
                }
            }
            // Extra usage only promotes the headline when a rate-limit window
            // hits 100% — mirrors the historical claudebar behavior.
            let any_at_cap = s.session.utilization_pct >= 100
                || s.weekly.utilization_pct >= 100
                || s.sonnet.as_ref().is_some_and(|w| w.utilization_pct >= 100);
            if any_at_cap {
                if let Some(extra) = s.extra {
                    let p = extra.percent();
                    if p > max {
                        max = p;
                    }
                }
            }
            RawMetrics {
                cost_usd: s.extra.map(|e| (e.spent.0 as f64) / 100.0),
                pct_used: Some(max as f64),
                tokens_used: None,
                label_kind: LabelKind::PercentOfWindow,
            }
        }
        VendorSnapshot::Openai(s) => {
            let max = s.session.utilization_pct.max(s.weekly.utilization_pct);
            // Codex OAuth path is the canonical signal; Unavailable means the
            // user hasn't linked their Codex auth yet.
            let label_kind = match s.source {
                crate::usage::OpenAiSource::CodexOauth => LabelKind::PercentOfWindow,
                crate::usage::OpenAiSource::AdminKeyMtd => LabelKind::UsdSpent,
                crate::usage::OpenAiSource::Unavailable => LabelKind::OAuthUnlinked,
            };
            RawMetrics {
                cost_usd: None,
                pct_used: Some(max as f64),
                tokens_used: None,
                label_kind,
            }
        }
        VendorSnapshot::Zai(s) => {
            let session = s.session.as_ref().map(|w| w.utilization_pct).unwrap_or(0);
            let weekly = s.weekly.as_ref().map(|w| w.utilization_pct).unwrap_or(0);
            let mcp = s.mcp.as_ref().map(|w| w.utilization_pct).unwrap_or(0);
            let max = session.max(weekly).max(mcp);
            let pct_used = if s.session.is_some() || s.weekly.is_some() || s.mcp.is_some() {
                Some(max as f64)
            } else {
                None
            };
            RawMetrics {
                cost_usd: None,
                pct_used,
                tokens_used: None,
                label_kind: LabelKind::PercentOfWindow,
            }
        }
        VendorSnapshot::Openrouter(s) => RawMetrics {
            cost_usd: Some(s.total_usage),
            pct_used: Some(s.consumed_pct() as f64),
            tokens_used: None,
            label_kind: LabelKind::UsdSpent,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage::{
        AnthropicSnapshot, Cents, ExtraUsage, OpenAiSnapshot, OpenAiSource, OpenRouterSnapshot,
        UsageWindow, ZaiSnapshot,
    };

    fn window(pct: i32) -> UsageWindow {
        UsageWindow {
            utilization_pct: pct,
            resets_at: None,
            window_duration: chrono::Duration::hours(5),
        }
    }

    #[test]
    fn anthropic_uses_worst_of_windows() {
        let snap = AnthropicSnapshot {
            plan: "Max 5x".into(),
            session: window(40),
            weekly: window(75),
            sonnet: Some(window(20)),
            extra: None,
        };
        let m = compute_metrics(&VendorSnapshot::Anthropic(snap));
        assert_eq!(m.pct_used, Some(75.0));
        assert_eq!(m.label_kind, LabelKind::PercentOfWindow);
        assert!(m.cost_usd.is_none());
    }

    #[test]
    fn anthropic_promotes_extra_only_when_window_at_cap() {
        // Extra at 90% but no window at 100% — extra ignored.
        let snap = AnthropicSnapshot {
            plan: "Max 5x".into(),
            session: window(50),
            weekly: window(60),
            sonnet: None,
            extra: Some(ExtraUsage {
                limit: Cents(10000),
                spent: Cents(9000),
            }),
        };
        let m = compute_metrics(&VendorSnapshot::Anthropic(snap));
        assert_eq!(m.pct_used, Some(60.0));
        // Cost is still surfaced even when not the headline.
        assert_eq!(m.cost_usd, Some(90.0));
    }

    #[test]
    fn anthropic_extra_promoted_at_cap() {
        let snap = AnthropicSnapshot {
            plan: "Max 5x".into(),
            session: window(100),
            weekly: window(50),
            sonnet: None,
            extra: Some(ExtraUsage {
                limit: Cents(10000),
                spent: Cents(10000),
            }),
        };
        let m = compute_metrics(&VendorSnapshot::Anthropic(snap));
        assert_eq!(m.pct_used, Some(100.0));
    }

    #[test]
    fn openai_codex_oauth_is_percent_of_window() {
        let snap = OpenAiSnapshot {
            plan: "Plus".into(),
            session: window(20),
            weekly: window(45),
            code_review: None,
            credits: None,
            source: OpenAiSource::CodexOauth,
        };
        let m = compute_metrics(&VendorSnapshot::Openai(snap));
        assert_eq!(m.pct_used, Some(45.0));
        assert_eq!(m.label_kind, LabelKind::PercentOfWindow);
    }

    #[test]
    fn openai_unavailable_signals_oauth_unlinked() {
        let snap = OpenAiSnapshot {
            plan: "Plus".into(),
            session: window(0),
            weekly: window(0),
            code_review: None,
            credits: None,
            source: OpenAiSource::Unavailable,
        };
        let m = compute_metrics(&VendorSnapshot::Openai(snap));
        assert_eq!(m.label_kind, LabelKind::OAuthUnlinked);
    }

    #[test]
    fn zai_no_windows_yields_none_pct() {
        let snap = ZaiSnapshot {
            plan: "GLM".into(),
            session: None,
            weekly: None,
            mcp: None,
        };
        let m = compute_metrics(&VendorSnapshot::Zai(snap));
        assert!(m.pct_used.is_none());
    }

    #[test]
    fn zai_picks_worst_bucket() {
        let snap = ZaiSnapshot {
            plan: "GLM".into(),
            session: Some(window(30)),
            weekly: Some(window(85)),
            mcp: Some(window(10)),
        };
        let m = compute_metrics(&VendorSnapshot::Zai(snap));
        assert_eq!(m.pct_used, Some(85.0));
    }

    #[test]
    fn openrouter_surfaces_usd_and_pct() {
        let snap = OpenRouterSnapshot {
            label: "OR".into(),
            total_credits: 100.0,
            total_usage: 35.5,
            usage_daily: 1.0,
            usage_weekly: 5.0,
            usage_monthly: 20.0,
            is_free_tier: false,
            limit: None,
            limit_remaining: None,
        };
        let m = compute_metrics(&VendorSnapshot::Openrouter(snap));
        assert_eq!(m.cost_usd, Some(35.5));
        assert_eq!(m.pct_used, Some(36.0));
        assert_eq!(m.label_kind, LabelKind::UsdSpent);
    }
}
