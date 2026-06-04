# Prompts Changelog

All notable changes to versioned prompts in this directory.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/).
Each prompt has its own version label encoded in its filename (e.g.
`insights.v1.md`); cross-version migrations are noted here.

## [v1] — 2026-05-31 (Story 1.15)

### Added
- `insights.v1.md` — initial AI Insights prompt; tool_use mode against
  `submit_insight` schema.
- `insights.v1.schema.json` — JSON Schema for the `submit_insight` tool
  input, mirroring `InsightsOutput` in `crates/torven-core/src/insights/schema.rs`.
- `judge.v1.md` — stub for the Story 1.17 eval pipeline (not consumed in
  v1.0).
