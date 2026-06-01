# Decision Log — Story 1.6 (Config Migration)

Decisions taken autonomously during YOLO implementation of Story 1.6.

## D1 — `RawConfig` shape supports both schemas via `serde(untagged)` per vendor section

**Decision:** Each per-vendor `RawXxx` struct accepts EITHER the legacy fields (`api_key`, `api_key_env`, `plan_tier`) OR the new `accounts = [...]` array, with a `#[serde(default)]` so users can omit a vendor entirely. The top-level `RawConfig` then implements `TryFrom<RawConfig> for Config` and emits a `tracing::warn!` per vendor when legacy fields are detected.

**Why:** Avoids two separate parse passes (one to detect schema, then re-parse). Single deserialize, then a structural conversion. Keeps backward compat for users who left their `config.toml` from v0.x untouched.

**Trade-off:** `Raw*` structs are slightly larger because they carry both shapes. Cost is negligible — these are parse-time only.

## D2 — Legacy → migrated `Account` uses `name = "default"` and propagates `api_key_env`

**Decision:** When legacy `[zai] api_key = "..."` or `[openrouter] api_key_env = "ZAI_API_KEY"` is parsed, we produce a single `Account { name: "default", api_key, description: None, budget_usd: None, tag: None }` plus retain `api_key_env` at the vendor level (`VendorEnvConfig`). The legacy single-key behavior is preserved bit-for-bit when the migrated config is consumed.

**Why:** The story explicitly mandates "name: \"default\"" (AC-4). `api_key_env` is a vendor-level setting (which env var name to read from), not an account-level one — accounts have their own optional `api_key`. The fetcher contract `resolve_api_key(label, env_var_name, inline)` still works: we use the vendor-level `api_key_env` + the first account's `api_key`.

## D3 — Vendor-level `accounts` is `Vec<Account>` (not `HashMap<VendorId, Vec<Account>>` in TOML)

**Decision:** In TOML the schema is `[[openrouter.accounts]]` (array-of-tables under the per-vendor section, matching the existing `[openrouter]`/`[zai]` sections). In the `Config` Rust struct the type is `HashMap<VendorId, Vec<Account>>` (per AC-2).

**Why:** AC-3 specifies the TOML format `[[openrouter.accounts]]`. AC-2 specifies `HashMap<VendorId, Vec<Account>>`. We bridge the two: TOML deserializes into a `RawConfig` with per-vendor sections, then `TryFrom` produces the `HashMap`. This matches existing TOML structure (one `[vendor]` section per vendor) and the `HashMap` shape requested by AC-2.

## D4 — `api_key_env` stays as vendor-level config, not per-account

**Decision:** `api_key_env` (which env var to read) remains a vendor-level field, not part of the `Account` struct. The `Account.api_key` is for inline keys (deprecated post-Story-1.20 / Keychain) or `None`.

**Why:** The semantic is "which env var name to read the key from for THIS vendor". It doesn't make sense per-account (you'd want `OPENROUTER_API_KEY_CLIENTACME` etc., which is overkill). When Story 1.20 lands, env-var resolution becomes legacy too; for now we keep one env var per vendor.

## D5 — `validate_config()` returns `Vec<ConfigError>` (not `Result`)

**Decision:** `validate_config()` collects ALL errors instead of bailing on the first one. Useful for showing the user every problem at once in the Settings overlay (Story 1.20+).

**Why:** Failing fast on the first error means the user fixes one, re-saves, sees the next error, and so on — terrible UX. Collect-then-display is industry standard for config validation.

## D6 — `account_id(vendor, name)` is a free function in `config.rs`

**Decision:** `pub fn account_id(vendor: VendorId, name: &str) -> String` lives at the module root of `config.rs`. Format: `format!("{}-{}", vendor.slug(), name.to_lowercase())`.

**Why:** It's a deterministic ID derivation used by SQLite (Story 1.13) and AI Insights (FR-7). Keeping it next to `Account` makes the contract visible. Lowercase normalization prevents `"Personal"` and `"personal"` colliding.

## D7 — Added `toml_edit = "0.22"` to `torven-core` deps

**Decision:** Move the dependency declaration from `torven-tui` to BOTH crates. `torven-tui/settings.rs` still uses it; `torven-core/src/config.rs` now also uses it for the new `save_config()` round-trip support.

**Why:** Story 1.4 removed `toml_edit` from `torven-core` saying "only had a TUI consumer in `settings.rs`" — but Story 1.6 explicitly needs round-trip with comment preservation in `torven-core` (AC-5, T5). Re-introduced as a genuine dep with comment in `Cargo.toml`.

## D8 — `tracing` added as new dep on `torven-core`

**Decision:** Add `tracing = "0.1"` (just the macros — `warn!`, etc.) to `torven-core`. No subscriber configured here; consumers (`torven-tui` main, the macOS app via UniFFI) wire up subscribers themselves.

**Why:** Story explicitly mandates `tracing::warn!` for migration warnings (T4). Tiny dep (~5KB, zero runtime cost without subscriber). Alternative was `eprintln!` but that's noisy and uncapturable.

## D9 — `AccountTag` serializes lowercase via `#[serde(rename_all = "lowercase")]`

**Decision:** `enum AccountTag { Client, Personal, Team }` with `#[serde(rename_all = "lowercase")]` so TOML reads `tag = "client"`.

**Why:** Matches AC-3 exact TOML format. Consistent with `VendorId`'s existing `rename_all = "lowercase"`.

## D10 — Refactor `torven-tui/src/app.rs` and `settings.rs` call sites

**Decision:** Update `app.rs` `build_outcome()` for `Openrouter` / `Zai` to read the first account's `api_key` instead of the deprecated `config.openrouter.api_key` field. `settings.rs` `from_config()` and `save_to_path()` adapted to seed/write into the first account.

**Why:** AC-7 + DoD require `cargo test --workspace` green. Breaking call sites is unavoidable since `OpenRouterConfig`/`ZaiConfig` are gone. Mitigation: add `Config::primary_account(vendor)` helper to encapsulate "find first account" so future call sites don't poke into the HashMap directly.

## D11 — `AnthropicConfig` / `OpenAiConfig` kept as standalone (not migrated to accounts)

**Decision:** Anthropic and OpenAI use OAuth, not API keys (they have `credentials_path` / `codex_auth_path`). They DO NOT participate in the `accounts: HashMap<VendorId, Vec<Account>>` system. Only OpenRouter and Z.AI use accounts.

**Why:** Per FR-6 wording: multi-account is for OpenRouter and Z.AI. Anthropic/OpenAI auth lives in vendor-specific files (`~/.claude/.credentials.json`, `~/.codex/auth.json`). Forcing them into the `Account` shape would be wrong — they have a different credential model. Keep them as `AnthropicConfig`/`OpenAiConfig` standalone fields in the new `Config`.

## D12 — Validation regex: `^[A-Za-z0-9_.\-]+$` (allow dots too)

**Decision:** API key validation regex is `^[A-Za-z0-9_.\-]+$` — alphanumeric + `_` + `-` + `.`. OpenAI keys contain dots (e.g. `sk-proj-xxx.xxx`), so the strict story-spec regex `^[A-Za-z0-9_\-]+$` would reject valid keys.

**Why:** The story regex `^[A-Za-z0-9_\-]+$` is from the issue spec but real API keys (OpenAI session keys, JWTs) include `.`. Spaces, `!`, `@`, `#` etc. are still rejected (per story edge cases). This is a pragmatic deviation documented here.

## D13 — `[display]` `refresh_interval_secs` lower bound = 10

**Decision:** `validate_config()` rejects `refresh_interval_secs < 10`. Matches story edge case "refresh_interval_secs < 10 → ConfigError".

## D14 — `[ai_insights]` `max_cost_usd` default = 0.05, `rate_limit_per_minute` default = 20

**Decision:** Sensible defaults — matches the `config.example.toml` story spec (T8) for `max_cost_usd = 0.05`. `rate_limit_per_minute = 20` is conservative (one insight request per 3s on average).

## D15 — `Config::load()` keeps its return type but `load_from()` returns warnings via second channel

**Decision:** `Config::load()` and `Config::load_from(path)` return `Result<Config>` as today. Migration warnings are emitted as side effects via `tracing::warn!` — not returned to the caller — because almost no caller currently cares.

For test surface, add `Config::load_from_str_with_warnings(s) -> Result<(Config, Vec<String>)>` so unit tests can assert exact warning strings.

**Why:** Minimizes the API blast radius. Existing callers (`Config::load()`, `Config::load_from()`) keep their signatures; only the new test helper exposes warnings explicitly.
