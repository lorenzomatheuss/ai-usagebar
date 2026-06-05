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

## Próximas waves (planejamento ativo)

| Wave | Goal | Status |
|---|---|---|
| **3** | Vendor Cards + Account Picker | 🟡 KICKOFF (handoff master→sm pendente) |
| **4** | Main Window + Swift Charts | pendente |
| **5** | AI Insights UI + Settings | pendente |
| **6** | CI eval gate + observability | pendente |
| **7** | Polish + Release (Sparkle + notarization) | pendente |

Detalhes em `memory/wave_plan.md`.
