# Repository Guidelines

## Project Structure & Module Organization

This is a Rust 2024 crate for a Waybar widget and terminal TUI. Source lives in `src/`, with library entry points in `src/lib.rs` and binaries in `src/bin/torven.rs` and `src/bin/torven-tui.rs`. Provider code is grouped under `src/anthropic/`, `src/openai/`, `src/openrouter/`, and `src/zai/`. Widget code is in `src/widget/`, TUI code in `src/tui/`, and shared formatting, tooltip, and cache behavior sits in top-level modules. Tests are in `tests/`, fixtures in `tests/fixtures/`, snapshots in `tests/snapshots/`, and AUR packaging in `packaging/aur/`.

## Build, Test, and Development Commands

- `cargo build` builds the debug binaries locally.
- `make build` or `cargo build --release` builds optimized release binaries.
- `cargo test` or `make test` runs the normal test suite.
- `make smoke` runs ignored live API tests and requires real credentials in the environment or local vendor auth files.
- `make clippy` runs `cargo clippy --all-targets -- -D warnings`.
- `make fmt` runs `cargo fmt`.
- `cargo run --bin torven -- --json` tests widget JSON output.
- `cargo run --bin torven-tui` launches the TUI.

## Coding Style & Naming Conventions

Use standard `rustfmt` formatting with four-space indentation. Keep modules aligned with existing boundaries: provider parsing belongs in that provider directory, while shared behavior belongs in top-level modules. Prefer snake_case for functions, modules, and variables; use PascalCase for structs/enums. Preserve the widget invariant that user-facing command failures still produce fallback Waybar JSON and exit successfully.

## Testing Guidelines

Use `cargo test` for routine validation. Add integration tests under `tests/` when behavior crosses module boundaries. Snapshot assertions use `insta`; update snapshots intentionally and review diffs before committing. Live API coverage belongs in `tests/live.rs` with `#[ignore]`, and should only be run through `make smoke` when credentials are available.

## Commit & Pull Request Guidelines

Git history uses short, imperative or release-oriented subjects such as `docs: add CHANGELOG.md` and `aur: pin v0.4.0 sha256s`; release commits may use `vX.Y.Z - summary`. Keep commits focused. PRs should describe the behavior change, list validation commands run, link related issues when applicable, and include screenshots for TUI or Waybar visual changes.

## Security & Configuration Tips

Never commit secrets. Do not print full config or credential files in logs; inspect structure only. API-key config examples belong in `config.example.toml`, with real values kept in environment variables or a chmod-protected local config. OAuth files such as `~/.claude/.credentials.json` and `~/.codex/auth.json` are user-local and must not be copied into the repo.
