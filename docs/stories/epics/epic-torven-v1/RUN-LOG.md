# Torven v1.0 — Epic Run Log

Histórico narrativo das waves do epic `epic-torven-v1` (pivot macOS native). Consolida handoffs YAML antigos arquivados em `.aiox/handoffs/_archive/` conforme rule `handoff-consolidation.md`. Wave ativa mantém handoff individual em `.aiox/handoffs/`.

**Epic:** Torven v1.0 — Pivot macOS native (rebrandado de `ai-usagebar`)
**Start:** 2026-05-31 (Tauri+React descartado mesmo dia, replanned como SwiftUI+Rust)
**Target:** v1.0 desktop `.app` notarizado em GitHub Releases

---

## Wave 0 — Planning ✅ DONE (2026-05-31)

**Goal:** ADR + PRD + estrutura de 11 stories backend + estimativas.

**Status:** done, sem código, só docs (todos em `docs/architecture/`, `docs/prd/`, `docs/research/`).

**Decisões-chave:**
- Pivot Tauri→SwiftUI formalizado (ADR v2 `docs/architecture/torven-v1-adr.md`)
- Stack: SwiftUI + Rust core via UniFFI (10 ADR entries — ADR-1..ADR-10)
- 8 riscos arquiteturais identificados (AR-1..AR-8); 3 marcados como spike obrigatório (AR-1 build perf, AR-2 [Async] cancel, AR-3 FFI memory)
- Competitive analysis: 4 competidores diretos (ClaudeBar, Tokens 4 Breakfast, Claude Usage Tracker, CodexBar) — todos single-vendor, SwiftUI nativos. Torven vence em multi-vendor + AI Insights layer

---

## Wave 1 — Foundation backend ✅ DONE (2026-06-04, merge `3c1b8d8`)

**Goal:** Rust core operacional + UniFFI bridge + AI Insights baseline + eval gate scaffolding.

**Execução:** 5 sessões, 11 stories serializadas com rajadas paralelas internas (1.5/1.6 paralelo).

**Stories entregues:**

| Story | Title | PR |
|---|---|---|
| 1.1 | Bootstrap workspace + Xcode skeleton | (consolidado) |
| 1.2 | Setup UniFFI scaffolding | (consolidado) |
| 1.3 | Cleanup Linux/Waybar code | (consolidado) |
| 1.4 | Extract torven-core crate | (consolidado) |
| 1.5 | First FFI bridge — get_vendor_list() | #6 |
| 1.6 | Config migration | (consolidado) |
| 1.13 | History SQLite layer | #8 |
| 1.15 | AI Insights core ([Async] FFI + AR-2 res) | #10 |
| 1.17 | Eval runner + 30-case dataset + baseline | #11 |
| 1.20 | Keychain migration | #9 |
| 1.26 | CI baseline | (consolidado) |

**Riscos resolvidos:**
- AR-1 (build perf) — RESOLVED Story 1.4
- AR-2 ([Async] cancel) — RESOLVED Story 1.15 (p99 cancel 343µs, 0 callbacks pós-cancel, 0 RSS delta)
- AR-3 (FFI memory) — RESOLVED Story 1.5 spike (0 leaks em 1000-iter + 60s live)

**Métricas baseline estabelecidas (Story 1.17):**
- Faithfulness: **0.90** (target ≥0.85)
- Relevance: **1.00** (target ≥0.80)
- Cost p50/p95: **$0.0017** (target ≤$0.05)
- 222 tests passando workspace-wide
- Baseline gravada em frontmatter `prompts/insights.v1.md` para Wave 6 CI gate

**Handoffs arquivados:** `.aiox/handoffs/_archive/wave1/` (6 YAMLs).

---

## Wave 2 — SwiftUI Skeleton ✅ DONE (2026-06-04, merge `dcb1177`, PR #12)

**Goal:** Formalizar estrutura `apple/Torven/` conforme ADR §4 + introduzir skeletons reusáveis (`MenuBarLabel`, `PopoverView`) sobre o app shell incidentalmente entregue por Story 1.5.

**Execução:** **1 sessão (~3h) com rajada paralela de orquestração** (3 sub-agents em fase 1 + 2 em fase 2 + 1 dev YOLO em fase 3).

**Re-escopo:** as stories originais 2.1/2.2/2.3 do handoff Wave 2 ("bootstrap + status item + popover stub") já estavam incidentalmente entregues pela Story 1.5. Foram re-escopadas para refactor ADR §4 puro via elicitação @sm + aprovação Lorenzo.

**Stories entregues:**

| Story | Title | Commit local | PO | QA |
|---|---|---|---|---|
| 2.1 | Formalizar estrutura `apple/Torven/` + remover stub VendorListView | `8eeb6d7` | GO 10/10 | PASS 10/10 |
| 2.2 | Extrair MenuBarLabel.swift (SF Symbol configurável) | `7fbcfc5` | GO 9.5/10 | PASS 9.5/10 |
| 2.3 | Introduzir PopoverView.swift (header + container vazio 360×420) | `ab53e28` | GO 10/10 | PASS 10/10 |

**Aggregate QA:** PASS 9.83/10, 0 issues, 222 tests passing.

**Housekeeping incluído no PR:** removal de `apple/Torven 2.xcodeproj/` (stale untracked artifact pré-Wave 1).

**Auto-decisões registradas (todas aceitas):**
1. @sm 2.2 — parâmetro `symbol: String` (não enum) + `static let defaultSymbol` (constante, não factory) — preserva API minimal para Wave 7 swap dinâmico
2. @sm 2.3 — frame size 360×420 como starting point (não 380×540) — sujeito a UX-Q1 closure em Wave 3
3. @dev 2.2 — smoke run individual omitido (redundante com 2.3, mesmo ícone status bar)
4. @dev 2.3 — `xcodebuild -project Torven.xcodeproj` explícito (ambiguidade resolvida via cleanup housekeeping)
5. @devops — housekeeping consolidado em commit `69eb9c9` (não criou commit separado para untracked file)

**Anomalia não-bloqueante:** CodeRabbit retornou "Review skipped" no PR #12 em vez de review substantivo. PR pequena pode estar sub-threshold. Flag para investigação futura.

**Files entregues (paths reais pós-merge):**
- `apple/Torven/MenuBar/MenuBarContent.swift` (criado)
- `apple/Torven/MenuBar/MenuBarLabel.swift` (criado, stateless, configurável)
- `apple/Torven/Popover/PopoverView.swift` (criado, vazio com header)
- `apple/Torven/Views/Preservation/VendorListView_Story15.swift` (movido via `git mv`)
- `apple/Torven/TorvenApp.swift` (refatorado: `MenuBarLabel(...)` label + `MenuBarContent()` body)
- `docs/qa/gates/2.{1,2,3}.*-gate.yaml` (3 gate files)
- `docs/stories/epics/epic-torven-v1/2.{1,2,3}.*.story.md` (3 stories Done)

**Métricas pós-Wave 2 (preservadas):**
- 222 tests workspace-wide (sem regressão)
- xcodebuild Debug BUILD SUCCEEDED
- Smoke runs documentados: ícone visível, popover abre vazio (regressão visual ESPERADA — vendor cards entram em Wave 3)

**Handoffs arquivados:** `.aiox/handoffs/_archive/wave2/` (10 YAMLs).

---

## Wave 3 — Vendor Cards + Account Picker (2026-06-04, merge `1d08ea7`, PR #13)

**Goal:** 5 vendor cards no popover + AccountPicker inline sheet + multi-account FFI real (Q5=A confirmado).

**Status:** DONE

**Stories entregues:**

| Story | Title | QA |
|---|---|---|
| 3.1 | VendorCard skeleton (OpenRouter first) | PASS 10/10 |
| 3.2 | ForEach 5 vendors + UX-Q1 closure (380×540) | PASS |
| 3.3 | AccountPicker inline sheet + set_active_account FFI | PASS |

**Decisões cravadas:**
- UX-Q1 fechada: frame 380×540 (Stories 3.2)
- Q5=A: FFI nova `AccountInfo` + `get_accounts_for_vendor` + `set_active_account` (Story 3.3)
- AR-8 (NSStatusItem multi-monitor): parkado para Wave 7

**Dispensation carregada para Wave 4:**
- Story 3.3: `set_active_account` in-memory only; persistência em config.toml → Story 4.0

**Métricas pós-Wave 3:** 225 tests passing (workspace), xcodebuild BUILD SUCCEEDED.

---

## Wave 4 — Main Window + Swift Charts — Draft (2026-06-05)

**Status:** Draft (aguardando @po validação)
**Stories:** 8 (4.0, 4.0.5, 4.1-4.6)
**Effort total:** ~6d (4-5 sessões)
**Decisões cravadas:** WAVE4-D1 a D7 (ver handoffs `handoff-2026-06-05-wave4-kickoff-master-to-sm.yaml` e `handoff-2026-06-05-wave4-story-4_0_5-master-to-sm.yaml`)

### Stories

| Story | Title | Effort |
|---|---|---|
| 4.0 | Persistir active-account map em config.toml via toml_edit | XS (~0.5d) |
| 4.0.5 | FFI ffi_query_aggregated — temporal-bucket aggregation (S) — owner: @data-engineer | S (~0.5d) |
| 4.1 | Main Window shell + invocação dupla (popover button + cmd+1) | M (~1d) |
| 4.2 | Date range picker (7d/30d/Custom) bound to HistoryQuery FFI | S (~0.5d) |
| 4.3 | Swift Charts foundation + Stacked area cost chart | M (~1d) |
| 4.4 | Per-vendor line chart grid (5 mini-charts) | M (~1d) |
| 4.5 | Request count chart (segunda métrica: Cost / Requests tab) | S (~0.5d) |
| 4.6 | Budget burn indicator (gauge progress) + budgets em config.toml | M (~1d) |

### Decisões Wave 4

- **D1:** Invocação Main Window = botão "Show History…" + cmd+1 via KeyboardShortcuts SPM
- **D2:** Story 4.0 fecha dispensation Wave 3 antes do Main Window
- **D3:** Date range = 7d / 30d / Custom (DatePicker .compact built-in)
- **D4:** Todos os 4 chart types: stacked area cost + per-vendor grid + request count + budget burn
- **D5:** KeyboardShortcuts (sindresorhus, SPM) para hotkey global
- **D6:** Budgets hard-coded em config.toml [budgets]; Wave 5 torna editável via Settings UI

### Riscos cross-story Wave 4

- ~~UDL shape para time-series: `AggregatedUsage` pode precisar extensão para suportar buckets temporais (Story 4.3)~~ **RESOLVIDO — ver Wave 4 Risk #1 abaixo**
- macOS Accessibility permission para hotkey global (Story 4.1)
- Timezone UTC vs local para reset mensal de budget (Story 4.6)
- Top bar com 3 pickers (DateRange + Aggregate/Per-vendor + Cost/Requests): layout pode ficar denso (Story 4.5)
- `Gauge` view macOS 13 compatibility (Story 4.6)

## Wave 4 — Risk #1 resolved (2026-06-05)

**Gap identificado:** A UDL original (`ffi_query_snapshots`) expõe apenas raw events paginados. Stories 4.3/4.4/4.5 precisavam de agregação temporal (buckets hora/dia/semana) para alimentar Swift Charts sem client-side aggregation em Swift — o que violaria NFR-1 (~43K events em 30d via loop paginado seria inaceitável).

**Decisão WAVE4-D7 (Lorenzo, cravada):** Path A — inserir Story 4.0.5 entre 4.0 e 4.1. Adicionar `ffi_query_aggregated(...)` ao `namespace torven_core {}` com `dictionary TimeBucket` + `enum BucketStrategy`. SQLite `GROUP BY bucket_start` server-side é ~100× mais performático que client-side e a mesma API será reutilizada por Wave 6 evals.

**Referência:** `docs/stories/epics/epic-torven-v1/4.0.5.ffi-query-aggregated.story.md` — owner: @data-engineer (Dara)

### Next

@po batch validation (story-draft-checklist 10-point) para as 8 stories 4.0, 4.0.5, 4.1-4.6.

## Wave 4 — PO batch validation (2026-06-05)

**Verdict agregado:** 8/8 stories **GO** (todas com score ≥ 7/10). 4 GO clean + 4 GO conditional pós-fix.

| Story | Score | Verdict | Pós-fix |
|---|---|---|---|
| 4.0 persist-active-account-config | 10/10 | GO clean | — |
| 4.0.5 ffi-query-aggregated | 10/10 | GO clean | — |
| 4.1 main-window-shell | 10/10 | GO clean | — |
| 4.2 date-range-picker | 8 → 10 | GO (F-1) | `MainWindowViewModel` = state-holder puro; FFI delegada para 4.3 |
| 4.3 swift-charts-stacked-area | 9 → 10 | GO (F-1) | `aggregate_by_vendor` → `ffi_query_aggregated`; `AggregatedSample.fromFFI([TimeBucket])` |
| 4.4 per-vendor-chart-grid | 10/10 | GO clean | — |
| 4.5 request-count-chart | 9 → 10 | GO (F-2) | Risk #1 reescrito: `TimeBucket.request_count` é a fonte canônica |
| 4.6 budget-burn-indicator | 8 → 10 | GO (F-1, F-3) | Rust-side aggregation reusando 4.0.5; dep 4.0.5 adicionada |

**Fixes aplicados:**
- **F-1** — Drift de FFI surface: removidas todas as referências a `aggregate_by_vendor` (função fictícia que nunca existiu no UDL); harmonizado para `ffi_query_aggregated` + `TimeBucket[]` (Story 4.0.5).
- **F-2** — Risk obsoleto da 4.5 sobre `requestCount` reescrito; fonte canônica documentada.
- **F-3** — Dependência 4.0.5 adicionada na 4.6.

**Todas as 8 stories agora estão Status: Ready.** Wave 4 desbloqueada para @dev. Owner sugerido para 4.0.5: @data-engineer (Dara); demais: @dev (Dex).

### Next

@dev inicia Wave 4 pela Story 4.0 (Rust-only, XS, fecha Wave 3 dispensation). Ou @data-engineer inicia em paralelo pela 4.0.5 (não há dependência técnica direta entre 4.0 e 4.0.5).

---

## Próximas waves (planejamento ativo)

| Wave | Goal | Status |
|---|---|---|
| **5** | AI Insights UI + Settings editável | pendente |
| **6** | CI eval gate + observability | pendente |
| **7** | Polish + Release (Sparkle + notarization) | pendente |

Detalhes em `memory/wave_plan.md`.
