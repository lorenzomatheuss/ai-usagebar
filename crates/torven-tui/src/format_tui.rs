//! TUI-side formatting helpers — bridge between [`torven_core::RawMetrics`] /
//! [`torven_core::pacing::PaceSeverity`] and ratatui rendering primitives.
//!
//! Story 1.4 split the formatting concern out of `torven-core`:
//!
//! - `torven-core::format` returns plain semantic structs ([`RawMetrics`],
//!   [`LabelKind`]) with no presentation knowledge.
//! - This module owns the TUI-specific layer: ratatui [`Color`] choices for
//!   severity bands, value-label formatting, and any short user-facing
//!   strings the ratatui panels need.
//!
//! The SwiftUI app gets its own analogous layer that lives on the Swift side
//! (semantic colors in `Assets.xcassets`).

use ratatui::style::Color;

use torven_core::format::{LabelKind, RawMetrics};
use torven_core::pacing::PaceSeverity;

/// Calm/amber/critical palette used by the developer TUI. Mirrors the
/// SwiftUI palette in `apple/Torven/Resources/Assets.xcassets/Colors`.
pub const CALM: Color = Color::Rgb(0x67, 0xB7, 0x6C); // green-ish (< 75%)
pub const AMBER: Color = Color::Rgb(0xE5, 0xA5, 0x4F); // warm yellow (75-89%)
pub const CRITICAL: Color = Color::Rgb(0xE0, 0x5E, 0x5E); // soft red (>= 90%)

/// Map a 0..=100 percent to the calm/amber/critical band used by the TUI.
///
/// Buckets:
///   `< 75`  → calm
///   `75..90`  → amber
///   `>= 90` → critical
///
/// These thresholds are *intentionally* tighter than
/// [`torven_core::pacing::severity_for`] (which mirrors the legacy claudebar
/// 70/85/95 bands). The TUI wants to nudge the user earlier; the menu-bar
/// label uses the legacy thresholds to stay conservative.
pub fn color_for_pct(pct: f64) -> Color {
    if pct >= 90.0 {
        CRITICAL
    } else if pct >= 75.0 {
        AMBER
    } else {
        CALM
    }
}

/// Color for a [`PaceSeverity`] — drop-in replacement for the deleted
/// `pango::severity_color` helper. Used by `panels.rs` for the per-window
/// gauge color.
pub fn color_for_severity(sev: PaceSeverity) -> Color {
    match sev {
        PaceSeverity::Low => CALM,
        PaceSeverity::Mid => AMBER,
        PaceSeverity::High => Color::Rgb(0xE5, 0x88, 0x3D), // warmer amber
        PaceSeverity::Critical => CRITICAL,
    }
}

/// Format a [`RawMetrics`] as a short, ratatui-friendly headline string —
/// suitable for the TUI tab title or a status footer.
///
/// Returns `None` when the metric set carries no displayable headline (e.g.
/// `LabelKind::OAuthUnlinked` with no values). Callers should fall back to
/// a vendor-specific "link credentials" hint.
pub fn format_for_tui(metrics: &RawMetrics) -> String {
    match metrics.label_kind {
        LabelKind::PercentOfWindow => match metrics.pct_used {
            Some(p) => format!("{}%", p.round() as i64),
            None => "—".into(),
        },
        LabelKind::MessagesQuota => match metrics.pct_used {
            Some(p) => format!("{}% messages used", p.round() as i64),
            None => "messages: —".into(),
        },
        LabelKind::UsdSpent => match (metrics.cost_usd, metrics.pct_used) {
            (Some(usd), Some(p)) => format!("${:.2} ({}%)", usd, p.round() as i64),
            (Some(usd), None) => format!("${:.2}", usd),
            (None, Some(p)) => format!("{}%", p.round() as i64),
            (None, None) => "—".into(),
        },
        LabelKind::OAuthUnlinked => "link credentials".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_thresholds_calm_amber_critical() {
        assert_eq!(color_for_pct(0.0), CALM);
        assert_eq!(color_for_pct(74.9), CALM);
        assert_eq!(color_for_pct(75.0), AMBER);
        assert_eq!(color_for_pct(89.9), AMBER);
        assert_eq!(color_for_pct(90.0), CRITICAL);
        assert_eq!(color_for_pct(100.0), CRITICAL);
    }

    #[test]
    fn percent_of_window_format() {
        let m = RawMetrics {
            cost_usd: None,
            pct_used: Some(73.4),
            tokens_used: None,
            label_kind: LabelKind::PercentOfWindow,
        };
        assert_eq!(format_for_tui(&m), "73%");
    }

    #[test]
    fn usd_spent_format_with_pct() {
        let m = RawMetrics {
            cost_usd: Some(12.75),
            pct_used: Some(48.0),
            tokens_used: None,
            label_kind: LabelKind::UsdSpent,
        };
        assert_eq!(format_for_tui(&m), "$12.75 (48%)");
    }

    #[test]
    fn usd_spent_format_without_pct() {
        let m = RawMetrics {
            cost_usd: Some(5.0),
            pct_used: None,
            tokens_used: None,
            label_kind: LabelKind::UsdSpent,
        };
        assert_eq!(format_for_tui(&m), "$5.00");
    }

    #[test]
    fn oauth_unlinked_format() {
        let m = RawMetrics {
            cost_usd: None,
            pct_used: None,
            tokens_used: None,
            label_kind: LabelKind::OAuthUnlinked,
        };
        assert_eq!(format_for_tui(&m), "link credentials");
    }

    #[test]
    fn missing_pct_falls_back_to_dash() {
        let m = RawMetrics {
            cost_usd: None,
            pct_used: None,
            tokens_used: None,
            label_kind: LabelKind::PercentOfWindow,
        };
        assert_eq!(format_for_tui(&m), "—");
    }

    #[test]
    fn severity_color_palette_distinct() {
        let low = color_for_severity(PaceSeverity::Low);
        let mid = color_for_severity(PaceSeverity::Mid);
        let high = color_for_severity(PaceSeverity::High);
        let critical = color_for_severity(PaceSeverity::Critical);
        // All four bands should yield distinct colors.
        assert_ne!(low, mid);
        assert_ne!(mid, high);
        assert_ne!(high, critical);
    }
}
