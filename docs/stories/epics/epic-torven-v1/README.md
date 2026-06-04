# Epic Torven v1.0 — Pivot Waybar Linux → macOS Native

> **Status:** Em Andamento
> **Responsável:** @pm (Morgan) + @sm (River)
> **Data criação:** 2026-05-31
> **Predecessores:** `docs/prd/torven-v1.md`, `docs/architecture/torven-v1-adr.md`, `docs/architecture/preservation-map.md`
> **Stack:** SwiftUI + Rust core via UniFFI (ADR v2 — SwiftUI path)

---

## Visão do Épico

Transformar o `ai-usagebar` (widget Waybar Linux) em **Torven**, um app nativo macOS com:

- **Menu bar + popover + janela detalhada** em SwiftUI puro
- **Rust core via UniFFI FFI** preservando ~82% do código existente (fetchers, OAuth, cache, pacing)
- **AI Insights eval-grade** com pipeline de eval público (dataset 30+ casos, métricas em CI)
- **Multi-account por vendor** (OpenRouter, Z.AI) com API keys no macOS Keychain
- **Distribuição zero-friction** via .dmg notarizado + Sparkle 2 auto-update

Referência completa: `docs/architecture/torven-v1-adr.md` (10 ADRs + migration sequence de 31 stories).

---

## As 31 Stories

### Grupo 1 — Foundation (paralelizáveis, sem dependência de UX)

Estas 11 stories podem ser iniciadas **antes** da UX spec estar pronta. São o foco do @dev agora.

| ID | Story | Status | Paralelização |
|----|-------|--------|---------------|
| 1.1 | Bootstrap Cargo workspace + Xcode project via XcodeGen | Done | INDEP |
| 1.2 | Setup UniFFI binding tool | Done | SEQ após 1.1 |
| 1.3 | Cleanup Linux-coupled code (waybar/widget/pango/tooltip/theme/active) | Done | PAR com 1.2 (após 1.1) |
| 1.4 | Migrate format.rs para RawMetrics + extrair torven-core lib | Done | SEQ após 1.3 |
| 1.5 | First FFI bridge: get_vendor_list() + consumir em Swift skeleton | Done | SEQ após 1.2 + 1.4 |
| 1.6 | Config migration para Vec<Account> shape (FR-6 multi-account schema) | Done | PAR com 1.5 (após 1.4) |
| 1.13 | History SQLite layer no torven-core + FFI helpers | In Review | SEQ após 1.4 |
| 1.15 | AI Insights core: Anthropic client + tool_use + streaming callback | Ready | SEQ após 1.13 |
| 1.17 | Eval runner bin (torven-evals) + dataset 30 casos rotulados | Ready | PAR com 1.15 (após 1.13) |
| 1.20 | Keychain migration: TOML api_keys → macOS Keychain via security-framework | In Review | SEQ após 1.6 |
| 1.26 | CI baseline: cargo test + clippy + cargo machete + xcodebuild scaffold | Done (CONCERNS) | PAR (após 1.1) |

### Grupo 2 — UI + SwiftUI (aguardam UX spec — NÃO criar ainda)

Estas 20 stories dependem das respostas do @ux-design-expert para as UX-Q1 a UX-Q6 definidas em `docs/architecture/torven-v1-adr.md#8-handoff-para-ux-design-expert`.

| # ADR | Story | Aguarda |
|--------|-------|---------|
| 3 | Bootstrap Xcode project via XcodeGen (hello world MenuBarExtra) | UX-Q1 (layout popover) |
| 8 | NSStatusBar tray + dynamic label (spike multi-monitor AR-8) | UX-Q1, UX-Q6 |
| 9 | FFI: fetch_snapshot + get_all_snapshots | — |
| 10 | SwiftUI Popover skeleton — header + 5 VendorCard placeholders | UX-Q1, UX-Q3 |
| 11 | Vendor card — first vendor (OpenRouter) | UX-Q2, UX-Q3 |
| 12 | Extend cards to all 5 vendors | UX-Q2 |
| 14 | Account picker UI (AccountPicker.swift) | UX-Q3 |
| 16 | Main window + sidebar + Swift Charts | UX-Q2, UX-Q5 |
| 18 | FFI: streaming callback interface (spike UniFFI [Async]) | UX-Q4 |
| 19 | SwiftUI Insights UI streaming consumption | UX-Q4 |
| 21 | CI eval gate (.github/workflows/eval-gate.yml) | — |
| 22 | Settings UI (SwiftUI Settings scene) | UX-Q5 |
| 23 | Menu bar label dinâmico (NSStatusItem label rotation) | UX-Q6 |
| 24 | Budget alerts (UNUserNotificationCenter) | UX-Q1 |
| 25 | Diagnostics command (Torven Doctor) | — |
| 26 (ADR) | Sparkle 2 integration | — |
| 27 | Code signing + notarization pipeline | — |
| 28 | CI release workflow rewrite | — |
| 29 | First-run onboarding wizard (FR-14) | UX-Q3 |
| 30 | README + screenshots + CHANGELOG + CLAUDE.md rewrite | — |
| 31 | v1.0 release tagging | — |

**Quando criar as stories do Grupo 2:** após @ux-design-expert entregar UX spec respondendo UX-Q1 a UX-Q6. Stories 9, 21, 25, 26, 27, 28, 30, 31 não dependem de UX diretamente mas dependem de stories anteriores de UI ficarem prontas.

---

## Diagrama de Dependências (Foundation Stories)

```
1.1 (INDEP)
 ├── 1.2 (SEQ)
 │    └── 1.5 (SEQ, junto com 1.4)
 ├── 1.3 (PAR com 1.2)
 │    └── 1.4 (SEQ)
 │         ├── 1.5 (SEQ, junto com 1.2)
 │         ├── 1.6 (PAR com 1.5)
 │         │    └── 1.20 (SEQ)
 │         └── 1.13 (SEQ)
 │              ├── 1.15 (SEQ)
 │              └── 1.17 (PAR com 1.15)
 └── 1.26 (PAR — só precisa de 1.1)
```

**Dia 1 — pode começar imediatamente:**
- Story 1.1 (Bootstrap workspace)
- Story 1.26 pode ser iniciada logo após 1.1 completar

**Dia 2-3 — em paralelo após 1.1:**
- Story 1.2 (UniFFI setup) e Story 1.3 (Cleanup) rodam em paralelo

---

## Riscos Arquiteturais Ativos (do ADR)

| Código | Risco | Severidade | Mitigação |
|--------|-------|------------|-----------|
| AR-1 | UniFFI maturity gaps em macOS | MEDIUM | Spike 0.5d em Story 1.2 |
| AR-2 | Tokio runtime em FFI context | MEDIUM-HIGH | `new_current_thread` + spike em Story 1.15 |
| AR-3 | Memory ownership ao cruzar FFI boundary | HIGH | `leaks` tool profile em Story 1.5 |
| AR-4 | Sparkle EdDSA key perdida | MEDIUM | Backup + runbook em `apple/Sparkle/README.md` |
| AR-8 | NSStatusItem positioning bugs multi-monitor | MEDIUM | Spike 0.5d em Story 8 (Grupo 2) |

---

## Referências

- `docs/prd/torven-v1.md` — FRs e NFRs
- `docs/architecture/torven-v1-adr.md` — 10 ADRs + migration sequence (§6)
- `docs/architecture/preservation-map.md` — PRESERVE/MIGRATE/DELETE/NEW map
