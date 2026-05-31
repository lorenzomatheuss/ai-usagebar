# Torven v1.0 — ADR (DISCARDED — Tauri path)

> ⚠️ **STATUS: DISCARDED em 2026-05-31**
>
> Decisão de stack revisada após reflexão sobre qualidade Maestri-grade
> (R$99 commercial vibe) e gap real de fluidez Tauri vs SwiftUI nativo em
> menu bar apps. Stack mudou para **SwiftUI + Rust core via FFI**.
>
> **O que sobrevive deste ADR (ainda válido):** ADR-1 (workspace split),
> ADR-2 (Keychain), ADR-3 (Sparkle), ADR-5 (SQLite rusqlite),
> ADR-7 (Anthropic tool_use), ADR-8 (eval runner Rust).
>
> **O que muda:** ADR-4 (IPC vira Swift→Rust FFI), ADR-6 (charts vira
> Swift Charts framework). Adicionar ADR-9 (FFI binding tool) e ADR-10
> (Xcode project structure).
>
> Documento preservado como referência histórica e narrativa de portfolio
> ("aqui está como avaliei Tauri vs Swift, e por que escolhi Swift").
>
> ADR vigente: `docs/architecture/torven-v1-adr.md`.

---

# Torven v1.0 — Architecture Decision Record (ADR) [Tauri path — discarded]

> **Autor:** @architect (Aria — Visionary)
> **Data:** 2026-05-31
> **Status:** ⚠️ DISCARDED — substituído pela versão SwiftUI
> **Predecessores:** `docs/prd/torven-v1.md`, `docs/architecture/preservation-map.md`, `docs/research/competitive-analysis.md`
> **Sucessores:** UX spec (@ux-design-expert), epics e stories (@po + @sm)
> **Formato:** Michael Nygard ADR style — uma decisão por seção, com Context, Options, Decision, Consequences

---

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-05-31 | 0.1.0 | ADR inicial pós-PRD, resolve 8 open questions, define workspace skeleton | @architect |

---

## 1. Context

O projeto **Torven** (rebrandado de `ai-usagebar`) está executando um pivot de Waybar Linux widget para app nativo macOS, conforme escopo formalizado no PRD `docs/prd/torven-v1.md` §1-2. A stack base (Tauri 2 + React + TypeScript + Rust core) e a forma do produto (menu bar + popover + janela completa) já foram decididas — este ADR não revisita essas escolhas. O `docs/architecture/preservation-map.md` confirma que ~82% do código Rust atual (fetchers vendor + lógica de negócio + cache + tipos) é cross-platform e sobrevive ao pivot; os ~18% acoplados ao Waybar/Pango/Omarchy serão deletados.

Este ADR resolve as **8 open questions** levantadas no PRD §10, define o **cleanup plan** concreto (arquivos a deletar/migrar/criar), propõe o **workspace skeleton** pós-pivot, ordena a **migration sequence** que alimenta o @sm, e mapeia **riscos arquiteturais** + **open questions** que precisam ser fechadas pelo @ux-design-expert antes do início da implementação. O objetivo central: produzir um app que (a) suporta as FRs L0 do PRD com performance nativa-grade (NFR-1), (b) preserva o Rust core como portfolio signal, e (c) deixa headroom claro para a v1.5 (cloud sync) sem refactor traumático.

---

## 2. Decisões arquiteturais

### ADR-1 — Workspace Cargo: multi-crate split

**Status:** Accepted
**Date:** 2026-05-31

#### Context

O crate atual é flat: `src/` contém vendors (anthropic/openai/openrouter/zai/gemini), TUI, widget Waybar, e tipos compartilhados. Pós-pivot teremos três bins distintos:

- O app Tauri 2 (binário principal `.app`)
- O TUI ratatui (preservado como dev tool / Linux fallback — vide preservation-map)
- Um eval-runner para AI Insights (§6.1 do PRD)

A pergunta é: tudo num crate só com features flags, ou workspace com crates separados?

#### Options

**A. Mantém flat (status quo)** — um crate `torven`, features flags (`tauri`, `tui`, `evals`).
- Prós: build incremental simples, refactor zero-cost de strutos compartilhados, menos overhead Cargo.toml.
- Cons: src-tauri/ vira sub-pasta esquisita; eval-runner puxa tauri deps desnecessariamente; menos legível para um recrutador AI Engineer abrir o repo e entender "onde está o core de LLM eng".

**B. Workspace com 3 crates** — `torven-core` (lib pura: fetchers + history + insights + types), `torven-app` (bin Tauri 2 que importa core), `torven-tui` (bin ratatui que importa core), e um quarto bin/lib opcional `torven-evals` (runner do eval pipeline).
- Prós:
  - **Portfolio signal explícito**: o recrutador AI Engineer abre o repo, vê `crates/torven-core/src/insights/` isolado, e enxerga LLM engineering depth separada da camada Tauri. Vide @analyst seção 5.
  - **Reusabilidade clara**: `torven-core` é reutilizado por `torven-app`, `torven-tui` e `torven-evals` sem puxar deps desnecessárias. Quando v1.5 introduzir backend de cloud sync, o `torven-core` vira o cliente sync sem ajustes.
  - **CI paralelizável**: `cargo test -p torven-core` roda fetchers + insights sem build do webview Tauri (que é lento).
  - **Eval runner não puxa Tauri**: separa concerns; eval rodando em CI Linux/macOS não precisa de webkit2gtk nem CoreGraphics.
- Cons: overhead inicial de mover arquivos; `Cargo.lock` único mas três `Cargo.toml`; refactor de imports.

**C. Workspace com 2 crates** — `torven-core` lib + `torven-app` bin, com TUI como feature opcional no core e evals como feature em `torven-app`.
- Prós: menos overhead que B.
- Cons: TUI vira feature flag escondida (perde legibilidade); eval runner herda deps Tauri (mata o "rodar evals em CI sem webkit").

#### Decision

**Adotar Opção B — workspace com 3 crates** (`torven-core` + `torven-app` + `torven-tui`) **+ binário extra `torven-evals` dentro do `torven-core`** (via `[[bin]]` em `crates/torven-core/Cargo.toml`).

Racional curto: portfolio signal e CI parallelization compensam o overhead inicial. O eval runner vive dentro do `torven-core` (não num quarto crate) porque o dataset e os prompts são naturalmente parte do core de LLM eng — separar artificialmente vira ruído. A separação `app` vs `core` é a única que importa para o pitch: "here's the LLM engineering, here's the macOS shell that consumes it."

#### Consequences

- **Positivas:**
  - Story 1 da migration sequence (Bootstrap workspace) é o primeiro investimento que paga em legibilidade pelo resto do projeto.
  - v1.5 cloud sync entra como novo crate `torven-sync` no workspace sem mexer no core.
  - Recrutador abre `crates/torven-core/src/insights/` direto — vide narrative do @analyst seção 5.
- **Negativas:**
  - 5-10% mais tempo no Story 1 (Bootstrap workspace). Mitigado por uma única story bem-feita.
  - Devs precisam aprender `cargo -p <crate>` para targetar builds. Mitigação: documentar em `crates/README.md`.
- **Backward compatibility:** O TUI existente em `src/tui/` migra para `crates/torven-tui/src/` sem mudança funcional. Smoke tests em `tests/live.rs` ficam no workspace root e podem rodar contra `torven-core`.

---

### ADR-2 — macOS Keychain integration: per-vendor JSON blob

**Status:** Accepted
**Date:** 2026-05-31

#### Context

O FR-6 do PRD especifica multi-account para OpenRouter e Z.AI (v1.0), com API keys "criptografadas em rest via macOS Keychain". A questão é granularidade: cada account é 1 entry separada no Keychain (`torven.openrouter.account.ClienteAcme`), ou todas as accounts de um vendor são serializadas num único JSON blob (`torven.openrouter.accounts` → `[{name, api_key, ...}, ...]`)?

Risco R-6 do PRD já cita: "Keychain prompts repetitivos ao acessar N accounts → atrito". Validação pediu @architect.

#### Options

**A. Per-account (1 Keychain entry por account)** — `torven.openrouter.account.{name}` armazena só o api_key; metadados (tag, budget, description) ficam em SQLite.
- Prós:
  - Granularidade de revogação: usuário pode deletar 1 account específica via Keychain Access sem afetar outras.
  - Audit trail nativo do Keychain (acesso, modificação).
  - Schema simples (1 string por entry).
- Cons:
  - **N prompts macOS no first run**: ao iniciar Torven com 5 OpenRouter accounts, o sistema pode pedir aprovação 5x (depende de "always allow this app").
  - Refresh de N accounts em paralelo dispara N keychain accesses.
  - Migração de schema (adicionar campo) vira "tocar N entries".

**B. Per-vendor blob (1 Keychain entry por vendor)** — `torven.openrouter.accounts` armazena JSON `[{name, api_key, tag?, budget_usd?, description?}, ...]`. SQLite mantém referência por `account_id` mas sem secrets.
- Prós:
  - **1 prompt macOS por vendor por sessão**: usuário aprova "Torven wants to access OpenRouter accounts" 1x e os N accounts são lidos juntos. Reduz friction drasticamente.
  - Operação atômica de "add/update account" = read blob, modify, write blob (com flock se concorrência).
  - Schema evolution simples (campo novo no JSON, sem migração Keychain).
- Cons:
  - Revogação de 1 account exige reescrever blob inteiro (não é problema real — a operação acontece pela UI Settings).
  - Tamanho do blob cresce com N accounts (irrelevante até N=100+).

**C. Híbrido (blob para secrets, SQLite para metadata)** — Keychain blob `[{name, api_key}, ...]`, SQLite `accounts(id, vendor, name, tag, budget_usd, description, ...)`.
- Prós: separa "what's secret" de "what's queryable"; metadata fica em SQL para queries de breakdown.
- Cons: dupla source-of-truth para `name` (Keychain + SQLite), risco de drift.

#### Decision

**Adotar Opção C — híbrido**: Keychain armazena 1 blob JSON por vendor com APENAS `[{account_id, api_key}, ...]` (uuid como id estável); SQLite armazena `accounts(account_id PRIMARY KEY, vendor, name, tag?, budget_usd?, description?, created_at, ...)` com `account_id` como foreign key para `usage_snapshots`.

Racional: a Opção B (blob puro) é o caminho de menor friction UX, mas mata queries SQL eficientes para breakdown por account (FR-6 AC: "vê lista com `name (tag) — $spent / $budget`"). Híbrido (C) tem o UX da Opção B (1 prompt por vendor) + a queryability da Opção A. A "dupla source-of-truth" temida é mitigada pelo uso de UUID estável: o id NUNCA muda; só metadata muda; queries são por id, não por name.

Implementação: usar crate `security-framework` (binding Rust oficial Apple para Security/Keychain Services APIs). Service name fixo `com.torven.app.{vendor}`, account name fixo `accounts-blob`. Versão do blob serializada (`{"version": 1, "accounts": [...]}`) para schema evolution futuro.

#### Consequences

- **Positivas:**
  - First-run UX é 5 prompts macOS no MÁXIMO (1 por vendor configurado), não N por account.
  - Queries de breakdown por account em SQL são triviais (`SELECT account_id, SUM(cost) FROM usage_snapshots WHERE vendor=? GROUP BY account_id`).
  - Schema evolution Keychain blob = bump `version` + migration code; SQLite metadata = standard `ALTER TABLE`.
- **Negativas:**
  - Código de "add account" toca dois storages atomicamente: Keychain blob write + SQLite insert. Mitigação: ordem write Keychain → SQLite (se SQLite falha, blob tem account órfã que aparece em UI mas funciona; melhor que vice-versa). Idempotência via UUID.
  - Sincronização Keychain ↔ SQLite na boot: Torven valida no startup que todo `account_id` em SQLite existe no blob Keychain; órfãos viram warning no `torven doctor` (FR-16).
- **Security:** API keys nunca tocam SQLite nem `config.toml`. Logs redatam keys (mesma disciplina do `src/anthropic/creds.rs`). Export CSV/JSON (FR-12) NÃO inclui api_keys.

---

### ADR-3 — Auto-update: Sparkle (macOS-native)

**Status:** Accepted
**Date:** 2026-05-31

#### Context

FR-15 + NFR-7 do PRD pedem auto-update via GitHub Releases. Duas opções principais: Sparkle (framework macOS-nativo padrão de fato, EdDSA signed appcast XML) ou tauri-plugin-updater (oficial Tauri 2, cross-platform).

#### Options

**A. Sparkle + sparkle-rs binding** — appcast.xml hospedado em GitHub Releases, EdDSA signature além da Apple Developer cert.
- Prós:
  - **Padrão de fato no ecossistema macOS** — ClaudeBar, Bartender, iStat Menus, milhares de apps macOS usam Sparkle.
  - UX consistente com o que usuário macOS espera (dialog Sparkle "A new version is available").
  - Defense-in-depth: EdDSA signature ALÉM do notarytool — exatamente o que R-3 do PRD recomenda.
  - Rollback nativo via Sparkle (`SUFeedURLForUpdater` pode apontar para versão anterior).
  - Maturidade: 17 anos de produção, encontra todos os edge cases de notarization/quarantine xattr.
- Cons:
  - Binding Rust (`sparkle-rs` ou FFI manual) é menos batido que Tauri updater oficial.
  - Cross-platform out: se v2.0 quiser Windows/Linux, troca obrigatória.

**B. tauri-plugin-updater** — plugin oficial Tauri 2, signed via Tauri's own signing scheme.
- Prós:
  - Plugin oficial, mantido pela equipe Tauri.
  - Cross-platform built-in (Windows/Linux/macOS).
  - Documentação Tauri integrada.
- Cons:
  - **Menos batido em macOS específico**: notarization + quarantine + Gatekeeper interactions menos testadas.
  - Não usa Sparkle UX patterns — pode surpreender usuário macOS power-user.
  - Defense-in-depth menor: usa só a Apple Developer signature; sem EdDSA layer.

**C. Custom updater rolling own** — Rust fetcher de GitHub Releases + spawn `installer` script.
- Cons: reinventar a roda. Não considerar.

#### Decision

**Adotar Opção A — Sparkle.**

Racional: o PRD é categórico — "macOS only" v1.0, com Linux/Windows fora de roadmap (NFR-4 + §7). Otimizar para cross-platform via Tauri updater seria YAGNI clássico. Sparkle resolve melhor o caso real (notarization friction de R-3 + UX nativo). Se v2.0 forçar cross-platform (improvável dado o PRD §7), aceitar o custo de troca.

Implementação: usar binding `sparkle-rs` quando estável OU FFI direto via `objc2` se precisarmos de bleeding-edge Sparkle 2.x. Appcast XML hospedado em `https://github.com/lorenzomatheuss/torven/releases/latest/download/appcast.xml`. EdDSA private key armazenada em GitHub Actions secret; public key embed no `Info.plist`.

#### Consequences

- **Positivas:**
  - First-run experience macOS é canônico (sem surpresas).
  - Defense-in-depth contra Apple Developer cert lapse (R-3): se Apple revoga, Sparkle EdDSA ainda valida bin.
  - Rollback automático se update falha (Sparkle salva versão anterior).
- **Negativas:**
  - `sparkle-rs` é menos maduro que tauri-plugin-updater. Mitigação: a story de update (Story 12 da migration sequence) deve incluir spike de 0.5d para validar binding antes do commit.
  - GitHub Actions release workflow precisa gerar `appcast.xml` em cada tag — adicionar step que assina EdDSA + escreve XML.
- **Future-proof:** se cloud sync v1.5 trouxer Windows/Linux por demanda, troca para Tauri updater é refactor isolado (1 story).

---

### ADR-4 — IPC Rust ↔ React: híbrido (commands para request/response, events para push streaming)

**Status:** Accepted
**Date:** 2026-05-31

#### Context

Tauri 2 oferece duas mecânicas de IPC: `invoke` commands (frontend → backend request/response, await pattern), e `emit`/`listen` events (broadcast bidirectional). FR-7 (AI Insights streaming) precisa de push para o frontend (tokens chegando). FR-9 (popover refresh) precisa de inicial load + reactive updates quando o core completa um refresh tick. FR-13 (Settings) é classic request/response.

#### Options

**A. Apenas commands** — frontend faz polling via `setInterval(invoke('get_snapshot'), 30s)`.
- Cons: polling é desperdício; latência depende de intervalo; AI Insights streaming exige round-trip por token (catastrófico).

**B. Apenas events** — backend faz broadcast contínuo via `emit('vendor-snapshot', payload)`.
- Cons: load inicial vira race (frontend monta antes do primeiro emit); semantics fica esquisita para Settings save (broadcast em vez de await).

**C. Híbrido** — commands para operações request/response (`get_snapshot`, `save_settings`, `add_account`, `list_accounts`); events para push streaming (`vendor-snapshot-updated`, `ai-insights-token`, `ai-insights-done`, `budget-alert`).
- Prós: cada padrão usa a mecânica certa; idiomático Tauri 2; reduz desperdício.

#### Decision

**Adotar Opção C — híbrido**. Mapa de IPC abaixo.

**Commands (frontend → backend, await):**

```
get_vendor_snapshot(vendor: VendorId, account_id?: String) -> VendorSnapshot
get_all_snapshots() -> Vec<VendorSnapshot>          // initial load do popover
list_accounts(vendor?: VendorId) -> Vec<Account>
add_account(vendor: VendorId, params: AddAccountParams) -> Account
update_account(account_id: String, params: UpdateAccountParams) -> Account
delete_account(account_id: String) -> ()
get_settings() -> Settings
save_settings(patch: SettingsPatch) -> Settings
get_history(filter: HistoryFilter) -> Vec<UsageSnapshot>
request_insights(context: InsightsContext) -> InsightsRequestId   // returns id, streaming via events
cancel_insights(request_id: InsightsRequestId) -> ()
run_doctor() -> DoctorReport
trigger_refresh() -> ()                              // user clicked Cmd+R
```

**Events (backend → frontend, push):**

```
vendor-snapshot-updated      { vendor, account_id?, snapshot, ts }
budget-alert                 { vendor, account_id, threshold, current_usd, budget_usd }
ai-insights-token            { request_id, token, accumulated_text }
ai-insights-structured-ready { request_id, structured: InsightsOutput }
ai-insights-error            { request_id, error_kind, message }
config-reloaded              { reason }                  // post-save signal
auth-required                { vendor, reason }          // OAuth re-login needed
```

#### Consequences

- **Positivas:**
  - AI Insights tem latência de primeiro token <2s (NFR-2) sem polling.
  - Popover load é instantâneo (single command `get_all_snapshots`) e em seguida reativo (listens em `vendor-snapshot-updated`).
  - Idiomático para React: events viram React hooks customizados (`useVendorSnapshot(vendor)` internamente faz `listen`).
- **Negativas:**
  - Frontend precisa managear dois canais de comunicação. Mitigação: criar wrapper `crates/torven-app/web/src/lib/tauri.ts` type-safe que esconde a dicotomia (devs chamam `useVendorSnapshot()` sem saber se vem de command ou event).
  - Type contracts entre Rust e TS precisam ser sincronizados. Mitigação: usar `ts-rs` ou `specta` crate para gerar `.d.ts` a partir de `#[derive(TS)]` em Rust types.

---

### ADR-5 — History storage: SQLite via rusqlite

**Status:** Accepted
**Date:** 2026-05-31

#### Context

FR-12 do PRD especifica persistência local de snapshots em `~/Library/Application Support/Torven/history.db`. AI Insights (FR-7) precisa de queries temporais (last-7-days agregado, breakdown por account). Retention default 90 dias. Schema versionado, migrations.

#### Options

**A. SQLite via `rusqlite`** — bundled SQLite, query SQL, migrations via `refinery` ou `rusqlite_migration`.
- Prós:
  - **Queries de agregação são triviais**: `SELECT vendor, account_id, SUM(cost_usd) FROM snapshots WHERE ts > ? GROUP BY vendor, account_id` — exatamente o que AI Insights consome.
  - Index em `(vendor, account_id, ts)` faz queries de 90d em <10ms para datasets típicos (<100k rows).
  - SQLite é battle-tested para dados local-first (1Password, WhatsApp, browsers todos usam).
  - 1 dependência nativa (libsqlite3 estático bundle = ~1MB no `.app`).
- Cons: schema migrations exigem disciplina; SQL injection risk se queries não parametrizadas (mitigável via `rusqlite::params!`).

**B. JSONL append-only diário** — `history-YYYY-MM-DD.jsonl`, zero-dep, parse on read.
- Prós: simplicidade extrema; merge entre devices futuro (v1.5 cloud sync) é trivial via union de arquivos.
- Cons:
  - **Queries temporais viram tragédia**: AI Insights last-7-days exige ler 7 arquivos, parsear N rows cada, filtrar em memória. Para 30s refresh interval × 5 vendors × 90 dias = ~1.3M rows → 100ms+ só para parse.
  - Sem index, breakdown por account é O(N).
  - Append-only não permite UPDATE de snapshot (necessário? talvez não).

**C. SQLite via `sqlx`** — async, compile-time query checks.
- Prós: async-first, query macros.
- Cons: pesado (puxa async runtime), overkill para single-user local DB; `rusqlite` é mais idiomático para Tauri side.

#### Decision

**Adotar Opção A — SQLite via `rusqlite`** com migrations via `rusqlite_migration` crate.

Racional: AI Insights é a feature L0 crítica do produto e depende fortemente de queries temporais agregadas. JSONL mata performance dessa query path. SQLite é a tool certa; o "overhead" de 1MB bundled vale a pena dentro do NFR-5 (<25MB). `sqlx` é overkill para single-user.

**Schema proposto (v1):**

```sql
-- v1 migration
CREATE TABLE accounts (
    account_id TEXT PRIMARY KEY,            -- UUID
    vendor TEXT NOT NULL,                   -- 'anthropic'|'openai'|'openrouter'|'zai'|'gemini'
    name TEXT NOT NULL,                     -- 'ClienteAcme', 'personal', etc.
    tag TEXT,                               -- 'client'|'personal'|'team'|NULL
    budget_usd REAL,                        -- NULL = no budget
    description TEXT,
    created_at INTEGER NOT NULL,            -- unix epoch
    deleted_at INTEGER                      -- soft delete
);

CREATE INDEX idx_accounts_vendor ON accounts(vendor) WHERE deleted_at IS NULL;

CREATE TABLE usage_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vendor TEXT NOT NULL,
    account_id TEXT,                        -- NULL for OAuth single-account vendors
    ts INTEGER NOT NULL,                    -- unix epoch
    raw_payload_json TEXT NOT NULL,         -- full vendor response (audit + schema drift recovery)
    cost_usd REAL,                          -- derived
    tokens_used INTEGER,                    -- derived
    pct_used REAL,                          -- 0.0..1.0 for window-based vendors
    metric_kind TEXT,                       -- '5h_window'|'monthly_quota'|'usd_spent'|...
    FOREIGN KEY (account_id) REFERENCES accounts(account_id)
);

CREATE INDEX idx_snapshots_vendor_ts ON usage_snapshots(vendor, ts DESC);
CREATE INDEX idx_snapshots_account_ts ON usage_snapshots(account_id, ts DESC) WHERE account_id IS NOT NULL;

CREATE TABLE insights_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts INTEGER NOT NULL,
    prompt_version TEXT NOT NULL,           -- 'v1', 'v2', ...
    schema_version TEXT NOT NULL,
    input_payload_hash TEXT,                -- audit: hash of payload sent to Anthropic
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_usd REAL,
    latency_ms INTEGER,
    structured_output_json TEXT,            -- full {headline, insights, recommendation, ...}
    user_feedback TEXT                      -- 'thumbs_up'|'thumbs_down'|NULL
);

CREATE TABLE schema_migrations (
    version INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);
```

**Retention job:** scheduled task no startup + a cada 24h: `DELETE FROM usage_snapshots WHERE ts < ?` (90d retention configurável). VACUUM ocasional (semanal) para reclaim disk.

#### Consequences

- **Positivas:**
  - AI Insights query last-7-days é `<10ms` no caso típico — atende NFR-2 (primeiro token <2s tem espaço orçamentário).
  - Export CSV/JSON (FR-12) é trivial: `SELECT * FROM ...` → CSV.
  - `torven doctor` (FR-16) reporta schema_version, row count, latest snapshot per vendor.
- **Negativas:**
  - Bundle +1MB. Aceito dentro de NFR-5.
  - Schema migration discipline obrigatória. Mitigação: cada migration é arquivo numerado em `crates/torven-core/src/history/migrations/`; CI gate verifica que migrations são forward-only.
- **Recovery:** se SQLite corrompe (raro), Torven detecta no startup, renomeia para `history.db.corrupt-{ts}`, cria novo arquivo vazio, e aplica migrations. Usuário perde histórico mas app funciona. Toast informa.

---

### ADR-6 — Chart render lib: Recharts (React-native)

**Status:** Accepted
**Date:** 2026-05-31

#### Context

FR-10 (janela detalhada) precisa de charts de histórico (30 dias por vendor, breakdown por modelo, accounts). PRD §10 question 5: SVG hand-rolled vs Recharts vs Visx vs nivo vs Tremor. Constraints: NFR-5 bundle <25MB, dev velocity (single-dev pace, R-5), UX polish.

#### Options

**A. Recharts** — React wrapper de D3, declarativo, ~95KB gzip.
- Prós: API React-idiomática (`<LineChart><Line dataKey="x" /></LineChart>`); copy-paste de exemplos abundante; mantida há 9 anos; suficiente para line/bar/area charts do v1.0.
- Cons: API antiga, alguns rough edges em customization; performance OK para ~1k pontos (suficiente para 30d × 24h).

**B. Visx (Airbnb)** — primitives D3 + React, ~50KB gzip por chart type usado.
- Prós: power D3 + control React; melhor performance que Recharts para grandes datasets; design system-friendly.
- Cons: API low-level — cada chart é uma micro-arquitetura; dev velocity menor; overkill para necessidades do v1.0.

**C. nivo** — wrapper D3 mais moderno que Recharts, ~80KB gzip.
- Prós: bonito out-of-box; bom default styling.
- Cons: menos exemplos copy-paste; menos comunidade em 2026.

**D. Tremor (HQ)** — chart kit baseado em Recharts + Tailwind, ~120KB gzip.
- Prós: design system pronto, looks polidos imediatamente.
- Cons: amarra a Tailwind hard; opinionated demais; "another dashboard kit" smell.

**E. SVG hand-rolled** — D3 puro ou primitive SVG.
- Prós: zero dep, controle total, bundle <5KB.
- Cons: dev velocity catastrófica para single-dev; line+area+bar charts viram 500 LOC fácil.

#### Decision

**Adotar Opção A — Recharts.**

Racional: o trade-off é **dev velocity > polish marginal** para um projeto de portfolio single-dev (R-5 do PRD). Recharts entrega 90% do que Visx entrega em 20% do tempo de dev. ~95KB gzip dentro de NFR-5 (`.app` <25MB; 95KB JS é irrisório). Charts no Torven são suporte (histórico, breakdown) — não são o produto. O produto é AI Insights + multi-account UX.

Stretch: se UX revelar gap visual sério, é refactor isolado (cada chart vira componente próprio) — pode migrar para Visx em v1.5 sem dor.

#### Consequences

- **Positivas:**
  - Implementação de FR-10 charts em 1-2 stories (não 4-5).
  - Comunidade enorme = stack overflow rico para edge cases.
- **Negativas:**
  - Look não é "premium-indie tier" out-of-box. Mitigação: theme custom (cores Torven, tipografia SF Mono no axis) absorve 80% do gap visual.
  - Performance teto se v1.5 trouxer históricos longos. Mitigação: paginação no SQL + downsample no Rust side antes de mandar pro chart.

---

### ADR-7 — JSON Schema enforcement (AI Insights): Anthropic tool_use mode

**Status:** Accepted
**Date:** 2026-05-31

#### Context

FR-7 + §6.3 do PRD: AI Insights retorna JSON estruturado `{headline, insights[], recommendation, cost_usd}`. Duas formas em Anthropic Messages API: tool_use mode (force_choice tool) ou prompt-only com retry-on-parse-fail.

#### Options

**A. tool_use mode (Anthropic native)** — definir 1 tool com `input_schema`, fazer request com `tool_choice: {type: "tool", name: "submit_insight"}`. Claude OBRIGATORIAMENTE retorna `tool_use` content block validado contra schema.
- Prós:
  - **Schema enforcement no servidor Anthropic** — Claude 3.5+ é muito robusto nesse modo; falha de parse é virtualmente zero.
  - Latência ligeiramente menor (sem retry).
  - Streaming de tokens ainda funciona via `content_block_delta` events.
  - Token efficient: não precisa do "respond only in JSON" boilerplate no prompt.
- Cons:
  - Schema definition é mais rígido (alguns padrões JSON Schema não suportados: refs profundos, `oneOf` complexos).
  - Lock-in pequeno em Anthropic-specific API (mas Claude é fixo no Torven; sem multi-LLM no AI Insights).

**B. Prompt-only com retry** — prompt diz "respond ONLY in this JSON schema: ...", parse cliente, retry se falha (max 2x).
- Prós:
  - Portável para qualquer LLM (não importa para Torven — vide acima).
  - Schema arbitrário (qualquer JSON Schema).
- Cons:
  - **Custo** — retries são $$. Eval pipeline mede e mostra: prompt-only com Claude 3.5 falha parse ~3-5% das vezes; 2 retries dobram custo nesses casos.
  - Latência variável.
  - Mais código de parse/validate/retry no Rust side.

**C. Híbrido (tool_use primário, prompt-only fallback)** — tenta tool_use, se falha (Anthropic API error específico), fallback para prompt-only.
- Prós: defense-in-depth.
- Cons: complexidade extra sem ganho real (tool_use praticamente nunca falha no modo).

#### Decision

**Adotar Opção A — tool_use mode** com schema versionado em `prompts/insights.v{N}.schema.json`.

Racional: PRD §6.3 já indica essa direção ("decisão @architect: tool-use é mais robusto que prompted JSON em Claude 3.5+"). É a opção tecnicamente certa. O lock-in Anthropic é zero (o AI Insights é projetado especificamente para Claude — sem ambiguidade de "trocar provider"). Eval pipeline (§6.1) valida que tool_use é confiável; se métricas mostrarem regressão, refactor para B é isolado.

Implementação:

- Definir tool `submit_insight` com `input_schema` JSON Schema mapeando o output spec do PRD §6.3.
- Request com `tool_choice: {type: "tool", name: "submit_insight"}` + `max_tokens: 1000` + `stream: true`.
- Streaming: capturar `content_block_delta` events do tipo `input_json_delta` (Anthropic streaming format para tool_use) e parsear partial JSON com `partial-json` crate ou similar — emit `ai-insights-token` event a cada chunk.
- Final: ao receber `message_stop`, validar schema final (defensive), emit `ai-insights-structured-ready` event.
- Failure: API error (rate limit, network) → emit `ai-insights-error` event com kind. Schema validation fail (improvável) → log raw output, fallback UI.

#### Consequences

- **Positivas:**
  - Parse failures viram <0.5% — eval pipeline pode focar em faithfulness/relevance em vez de parse robustness.
  - Token budget rebaixa (sem "respond ONLY in JSON" boilerplate).
  - Schema versionado git-tracked = portfolio signal forte (vide @analyst seção 5: "structured output via JSON schema").
- **Negativas:**
  - Schema JSON Schema features são subset (sem refs profundos). Mitigação: design schema simples e flat — alinha com PRD §6.3 que já é flat.
- **Eval impact:** eval runner roda os 30 casos rotulados contra tool_use mode; baseline target faithfulness ≥0.85, relevance ≥0.80 (PRD §6.1).

---

### ADR-8 — Eval runner: Rust nativo (binário em `torven-core`)

**Status:** Accepted
**Date:** 2026-05-31

#### Context

§6.1 do PRD: pipeline de eval com 30+ casos rotulados, métricas (faithfulness, relevance, latency p50/p95, cost p50/p95), rodado em CI. PRD §10 question 8: Rust nativo OU Node.js (ecossistema promptfoo/langfuse SDK).

#### Options

**A. Rust nativo (binário em `torven-core`)** — `[[bin]] name="torven-evals"`, dataset JSONL em `crates/torven-core/evals/dataset.jsonl`, runner em Rust usa o MESMO código de produção (chama `insights::generate(payload)`).
- Prós:
  - **Single source of truth**: o código que roda em prod é o código que roda no eval. Zero drift entre eval result e prod behavior.
  - CI integration trivial: `cargo run -p torven-core --bin torven-evals -- --dataset evals/dataset.jsonl` direto em GitHub Actions.
  - Métricas (cost, latency, tokens) já são tracked internamente — eval só agrega.
  - Mock LLM client trivial: trait `LlmClient` com impl `MockLlmClient` (lat distribuída via sleep).
  - Reusa types: `InsightsOutput`, `InsightsContext` ficam consistentes.
- Cons:
  - Sem ecossistema pronto (promptfoo tem UI bonita, comparison views). Para portfolio é gap visível? Não — pipeline público em CI + métricas no README cumprem o pitch.
  - Judge LLM call (faithfulness avaliada por outro LLM) precisa ser implementada — mas é mais 1 chamada Claude com prompt judge, ~50 LOC.

**B. Node.js com promptfoo** — `promptfoo eval -c evals/promptfooconfig.yaml`.
- Prós:
  - Ecossistema pronto, UI HTML output, comparison de prompts.
  - Community standard em LLM eval circa 2026.
- Cons:
  - **Dual stack drift**: o "código de eval" chama prompt diretamente via API, não passa pelo path de prod (no `insights::generate` Rust). Risk de diferença sutil (e.g. tokenization, retry behavior).
  - +1 runtime no CI (npm install).
  - Não trackeia cost_usd integrado com o resto do app.
  - Tooling de dataset JSONL é portável; promptfoo tem seu YAML próprio (lock-in pequeno).

**C. Node.js custom (sem promptfoo)** — script TS lê JSONL + chama Anthropic API + computa métricas.
- Cons: dual stack drift sem o benefício do ecossistema. Não considerar.

#### Decision

**Adotar Opção A — Rust nativo** com binário `torven-evals` dentro do crate `torven-core`.

Racional: "single source of truth" é decisivo. AI Insights É o diferenciador do produto (vide @analyst). Qualquer drift entre eval result e prod behavior MATA a credibilidade do pitch ("evals públicos") — e o risco existe em B. Opção A elimina o risco por construção. O gap UX (promptfoo tem HTML output bonito) é mitigado por: (1) runner Rust gera markdown report compilável (publicado em `evals/results/`); (2) GitHub Actions PR comment com diff de métricas.

Implementação:

- `crates/torven-core/evals/dataset.jsonl` — 30+ casos com schema documentado em `crates/torven-core/evals/schema.md`.
- `crates/torven-core/src/bin/torven-evals.rs` — CLI com flags `--dataset PATH --prompt-version vN --mock-llm BOOL --output md|json`.
- Trait `LlmClient` no `torven-core` permite swap `RealAnthropicClient` ↔ `MockLlmClient` no eval.
- Judge LLM (para faithfulness): segundo Claude call com prompt judge separado (`prompts/judge.v1.md` — git-tracked, versionado).
- Métricas computadas: faithfulness, relevance, latency p50/p95, cost p50/p95 — saída markdown table + JSON.
- CI gate: regressão >5% vs baseline bloqueia merge (GitHub Actions check).

#### Consequences

- **Positivas:**
  - Eval e prod compartilham código → eval result PREDIZ prod behavior. Portfolio signal máximo.
  - CI workflow só precisa Rust toolchain (já presente). Zero npm/node setup.
  - Dataset format é dataset format — promptfoo-compatible JSONL se um dia formos exportar.
- **Negativas:**
  - Não tem UI HTML out-of-box. Mitigação: markdown report bonito + PR comment automation = suficiente para portfolio.
  - Implementar judge LLM caller (~80 LOC) é trabalho extra vs promptfoo built-in. Aceitável.

---

## 3. Cleanup plan

Lista concreta de deleções, migrações e renames pós-pivot. Base: `docs/architecture/preservation-map.md` + repo state atual (5 commits ahead `origin/main` com rebrand commitado).

### DELETE

| Arquivo / Pasta | Razão | Impacto em tests/builds |
|---|---|---|
| `src/waybar.rs` | WaybarOutput JSON + SIGRTMIN signaling — Waybar não existe em macOS | Remove imports em `src/lib.rs`. Tests: nenhum (não há test direto). Build: módulo sai do bin tree. |
| `src/pango.rs` | Pango markup helpers — substituído por React JSX | Idem. Pango render funcs tinham snapshot tests em `tests/anthropic_e2e.rs`? Verificar e remover assertions Pango-specific se houver. |
| `src/tooltip.rs` | Bordered-box Pango tooltip — substituído por popover Tauri | Tests: snapshot tests de tooltip rendering em `tests/` precisam ser deletados (não há equivalente em React UI nesta fase). |
| `src/theme.rs` | Detecção Omarchy — não existe em macOS; appearance vem de macOS dark/light | Remove imports. Tests: existe `tests/theme.rs`? Se sim, deletar. |
| `src/active.rs` | Scroll-cycle do Waybar status bar — sem scroll na menu bar macOS | Remove imports. ActiveVendor state migra para Tauri state (sem persistência cross-session). |
| `src/widget/mod.rs`, `src/widget/cli.rs`, `src/widget/pretty.rs`, `src/widget/render.rs`, `src/widget/run.rs` | Shell completo do Waybar widget | Remove `pub mod widget` em `src/lib.rs`. Bin `src/bin/torven.rs` que importava `widget::run::main` é deletado também. |
| `src/bin/torven.rs` | Entry point do widget Waybar | Substituído por `crates/torven-app/src-tauri/src/main.rs`. Tests: nenhum (era main). |
| `packaging/aur/` (toda) | AUR é Arch Linux apenas | Build: AUR PKGBUILD deletados. CHANGELOG menciona migration. Stories de release v0.x para v1.0 removem AUR menções. |
| `.github/workflows/release.yml` (versão atual) | Builds Linux x86_64/aarch64 | **REWRITE** (não delete) — vide MIGRATE. |
| `Makefile` (parcial — targets Linux) | Targets como `make smoke` permanecem; targets de instalação Linux/AUR saem | Audit linha-a-linha do Makefile na story de cleanup. |
| `config.example.toml` (atual, formato Waybar-oriented) | Substituído por novo formato com `accounts: [[openrouter]]` etc. | Story de migração de config gera novo `config.example.toml`. |

### MIGRATE

| Arquivo | O que muda | Impacto |
|---|---|---|
| `src/lib.rs` | Remove `pub mod {pango,theme,tooltip,waybar,widget,active}`. Adiciona `pub mod insights`, `pub mod history`. Move arquivos para `crates/torven-core/src/lib.rs` em workspace. | Imports nos vendor modules continuam — fetchers cross-platform. |
| `src/format.rs` | Atualmente retorna strings Pango markup. **Migrar para:** retornar `FormattedSnapshot { color_kind: ColorKind, label_text: String, ... }` onde `ColorKind` é enum semântico (`Calm`, `Mid`, `High`, `Critical`). React aplica cores. | Tests de format precisam ser atualizados — passam de assert strings Pango para assert struct shape. |
| `src/config.rs` | Adicionar `Config::to_json() -> serde_json::Value` para expor ao frontend via Tauri command. Adicionar `accounts: HashMap<VendorId, Vec<AccountRef>>` campo. Adicionar campos `ai_insights: AiInsightsConfig`, `history: HistoryConfig`. | Schema TOML evolui (com migration code para configs antigos — manter backward compat por 1 release). |
| `src/anthropic/creds.rs` e similares | Default path muda de `~/.config/...` (XDG Linux) para `~/Library/Application Support/Torven/{vendor}.credentials.json` (macOS convention). | Helper `default_creds_path()` retorna macOS path. OAuth flow continua igual. |
| `src/tui/*` | Move para `crates/torven-tui/src/`. Re-aponta imports para `torven_core::*`. | TUI bin continua funcional como dev tool / fallback. |
| `src/{anthropic,openai,gemini,zai,openrouter}/*` | Move para `crates/torven-core/src/vendors/{vendor}/`. Re-aponta imports. | Zero mudança funcional. Smoke tests `make smoke` continuam contra os mesmos endpoints. |
| `src/{cache,countdown,error,pacing,usage,vendor}.rs` | Move para `crates/torven-core/src/`. | Zero mudança funcional. |
| `tests/anthropic_e2e.rs` | Move para `crates/torven-core/tests/`. | Mockito-based, continua funcional. |
| `tests/live.rs` | Move para workspace-level `tests/live.rs` rodando contra `torven-core`. | `make smoke` continua. |
| `.github/workflows/release.yml` | **REWRITE**: build matrix muda de `[linux-x86_64, linux-aarch64]` para `[macos-aarch64, macos-x86_64]`; usa `tauri-action` para build .app + .dmg; integra notarytool (Apple ID + app-specific password em secrets); gera + assina appcast.xml para Sparkle (EdDSA signature key em secret). | Story de migração de release pipeline (Story 12). |
| `CLAUDE.md` (release checklist) | Atualizar checklist v0.x (cargo build + AUR pin) para v1.0 (cargo build workspace + Tauri bundle + notarize + Sparkle appcast). | Story 13 (CHANGELOG + README + CLAUDE.md rewrite). |
| `README.md` | Rewrite completo — narrativa AI Insights primária, screenshots, eval metrics table, install via .dmg. | Story 13. |
| `CHANGELOG.md` | Marca v1.0 como pivot completo macOS. Mantém histórico Waybar como predecessor (`### Predecessor — Waybar widget v0.x`). | Story 13. |

### NEW

| Path | Função |
|---|---|
| `crates/torven-core/Cargo.toml` | Lib + bin `torven-evals` |
| `crates/torven-core/src/insights/` | Cliente Anthropic Messages API com tool_use, streaming, eval-instrumented |
| `crates/torven-core/src/history/` | SQLite via rusqlite, migrations |
| `crates/torven-core/src/insights/llm_client.rs` | Trait `LlmClient` + `RealAnthropicClient` + `MockLlmClient` |
| `crates/torven-core/src/keychain/` | macOS Keychain integration via `security-framework` crate |
| `crates/torven-core/src/bin/torven-evals.rs` | Binário eval runner |
| `crates/torven-core/evals/dataset.jsonl` | Dataset rotulado (30+ casos v1.0) |
| `crates/torven-core/evals/schema.md` | Schema do dataset doc |
| `crates/torven-app/Cargo.toml` | Bin Tauri 2 |
| `crates/torven-app/src-tauri/` | Tauri 2 app (tauri.conf.json + main.rs com commands) |
| `crates/torven-app/web/` | React + TS + Vite + Recharts |
| `crates/torven-app/web/src/lib/tauri.ts` | Wrapper type-safe dos commands + listeners |
| `crates/torven-tui/Cargo.toml` | Bin ratatui (preserved) |
| `prompts/insights.v1.md` | Primeiro prompt versionado |
| `prompts/insights.v1.schema.json` | Schema JSON do tool_use |
| `prompts/judge.v1.md` | Judge LLM prompt para faithfulness |
| `prompts/CHANGELOG.md` | Histórico de versões de prompt |
| `evals/results/` | Outputs de eval runs (gitignored — só baseline em README) |
| `Cargo.toml` (root) | Workspace manifest |

---

## 4. Workspace skeleton

Tree esperado pós-pivot:

```
torven/                                # repo root
├── Cargo.toml                         # workspace manifest, members = [crates/*]
├── Cargo.lock
├── rust-toolchain.toml
├── README.md                          # rewrite v1.0
├── CHANGELOG.md
├── CLAUDE.md                          # updated release checklist
├── Makefile                           # macOS targets only
├── .gitignore
├── config.example.toml                # new schema with accounts
│
├── crates/
│   ├── torven-core/                   # ★ CORE: vendor fetchers + history + insights + keychain
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs                 # re-exports everything
│   │   │   ├── config.rs              # MIGRATED
│   │   │   ├── cache.rs               # PRESERVED
│   │   │   ├── countdown.rs           # PRESERVED
│   │   │   ├── error.rs               # PRESERVED
│   │   │   ├── pacing.rs              # PRESERVED
│   │   │   ├── usage.rs               # PRESERVED (VendorSnapshot enum)
│   │   │   ├── vendor.rs              # PRESERVED
│   │   │   ├── format.rs              # MIGRATED (structured, not Pango)
│   │   │   ├── vendors/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── anthropic/         # PRESERVED + creds path migrated
│   │   │   │   ├── openai/
│   │   │   │   ├── openrouter/
│   │   │   │   ├── zai/
│   │   │   │   └── gemini/
│   │   │   ├── insights/              # NEW
│   │   │   │   ├── mod.rs
│   │   │   │   ├── client.rs          # Anthropic tool_use streaming
│   │   │   │   ├── llm_client.rs      # trait + real + mock
│   │   │   │   ├── schema.rs          # InsightsOutput, validation
│   │   │   │   ├── eval.rs            # eval runner support
│   │   │   │   └── budget.rs          # cost/latency budget guards
│   │   │   ├── history/               # NEW
│   │   │   │   ├── mod.rs
│   │   │   │   ├── db.rs              # rusqlite wrapper
│   │   │   │   ├── migrations/
│   │   │   │   │   ├── 001_initial.sql
│   │   │   │   │   └── mod.rs
│   │   │   │   ├── retention.rs
│   │   │   │   └── export.rs
│   │   │   ├── keychain/              # NEW (macOS Keychain)
│   │   │   │   ├── mod.rs
│   │   │   │   └── blob.rs            # per-vendor JSON blob
│   │   │   └── bin/
│   │   │       └── torven-evals.rs    # NEW eval runner binary
│   │   ├── evals/
│   │   │   ├── dataset.jsonl
│   │   │   ├── schema.md
│   │   │   └── results/               # gitignored
│   │   └── tests/
│   │       ├── anthropic_e2e.rs       # MOVED from tests/
│   │       ├── insights_e2e.rs        # NEW
│   │       └── history_e2e.rs        # NEW
│   │
│   ├── torven-app/                    # ★ APP: Tauri 2 bin + React frontend
│   │   ├── Cargo.toml                 # depends on torven-core
│   │   ├── src-tauri/
│   │   │   ├── Cargo.toml
│   │   │   ├── tauri.conf.json
│   │   │   ├── build.rs
│   │   │   ├── icons/
│   │   │   ├── Info.plist             # Sparkle EdDSA public key, app metadata
│   │   │   └── src/
│   │   │       ├── main.rs            # tauri::Builder, register commands + events
│   │   │       ├── commands/          # IPC commands (vide ADR-4)
│   │   │       │   ├── mod.rs
│   │   │       │   ├── snapshots.rs
│   │   │       │   ├── accounts.rs
│   │   │       │   ├── settings.rs
│   │   │       │   ├── insights.rs
│   │   │       │   ├── history.rs
│   │   │       │   └── doctor.rs
│   │   │       ├── events/            # event emitters
│   │   │       │   ├── mod.rs
│   │   │       │   └── stream.rs
│   │   │       ├── tray/              # NSStatusBar bridge
│   │   │       │   └── mod.rs
│   │   │       ├── window/            # popover + main window
│   │   │       │   └── mod.rs
│   │   │       └── updater/           # Sparkle integration
│   │   │           └── mod.rs
│   │   └── web/
│   │       ├── package.json
│   │       ├── vite.config.ts
│   │       ├── tsconfig.json
│   │       ├── index.html
│   │       └── src/
│   │           ├── main.tsx
│   │           ├── App.tsx
│   │           ├── popover/
│   │           │   ├── Popover.tsx
│   │           │   ├── VendorCard.tsx
│   │           │   ├── AccountPicker.tsx
│   │           │   └── InsightsButton.tsx
│   │           ├── window/
│   │           │   ├── MainWindow.tsx
│   │           │   ├── VendorView.tsx
│   │           │   ├── HistoryChart.tsx     # Recharts
│   │           │   ├── AccountsList.tsx
│   │           │   ├── SettingsView.tsx
│   │           │   └── InsightsHistory.tsx
│   │           ├── insights/
│   │           │   ├── InsightsPanel.tsx
│   │           │   ├── InsightsStream.tsx
│   │           │   └── InsightCard.tsx
│   │           ├── tray/
│   │           │   └── (rendered via Tauri tray API; no React here)
│   │           ├── lib/
│   │           │   ├── tauri.ts             # typed wrapper
│   │           │   ├── format.ts            # color kind → CSS
│   │           │   ├── hooks.ts             # useVendorSnapshot, useInsights, ...
│   │           │   └── types.ts             # generated via ts-rs from Rust
│   │           └── i18n/
│   │               └── en.json
│   │
│   └── torven-tui/                    # PRESERVED: ratatui TUI (dev tool / Linux fallback)
│       ├── Cargo.toml                 # depends on torven-core
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── panels.rs
│           ├── settings.rs
│           └── view.rs
│
├── prompts/                           # NEW
│   ├── insights.v1.md
│   ├── insights.v1.schema.json
│   ├── judge.v1.md
│   └── CHANGELOG.md
│
├── docs/
│   ├── prd/torven-v1.md
│   ├── architecture/
│   │   ├── preservation-map.md
│   │   └── torven-v1-adr.md           # this file
│   ├── research/
│   │   ├── competitive-analysis.md
│   │   └── naming-shortlist.md
│   ├── stories/                       # @sm output
│   └── qa/
│
├── tests/                             # workspace-level
│   └── live.rs                        # smoke tests, make smoke
│
└── .github/
    └── workflows/
        ├── ci.yml                     # cargo test workspace + npm test + evals (mocked)
        ├── release.yml                # tag-driven: build .app + sign + notarize + Sparkle appcast
        └── eval-gate.yml              # PR gate: run evals if prompts/* or insights/* changed
```

ASCII data flow (popover open path):

```
User clicks tray icon
     ↓
Tauri NSStatusBar handler  ─── opens popover window
     ↓
React Popover mounts
     ↓
invoke('get_all_snapshots') ──────► torven-app commands::snapshots
                                      ↓
                                   torven-core::cache::read_or_fetch
                                      ↓ (per vendor in parallel)
                                   vendors::{anthropic,...}::fetch
                                      ↓
                                   format::FormattedSnapshot
                                      ↓
                              ◄────── Vec<VendorSnapshot> serialized
     ↓
React renders VendorCard × 5
     ↓
React listens to 'vendor-snapshot-updated' events
                                   ▲
                                   │ (background tick every 30s)
                                   │
                              torven-core::scheduler emits via Tauri
```

---

## 5. Migration sequence

Stories ordenadas que o @sm vai transformar em arquivos `docs/stories/`. Cada story em 1 linha + dependências. Mantenho granularidade ≤1d de @dev (R-5 do PRD).

| # | Story | Depende de | Output |
|---|---|---|---|
| 1 | **Bootstrap Cargo workspace** — root `Cargo.toml` workspace, move `src/` → `crates/torven-core/src/`, criar `crates/torven-tui/`, `crates/torven-app/Cargo.toml` skeleton, validar `cargo build --workspace` verde. | — | Workspace funcional, TUI binary buildable, core lib buildable. |
| 2 | **Cleanup Linux-coupled code** — deletar `waybar.rs`, `pango.rs`, `tooltip.rs`, `theme.rs`, `active.rs`, `widget/`, `bin/torven.rs`, `packaging/aur/`. Atualizar `lib.rs`. Confirmar `cargo build --workspace -p torven-core` verde. | 1 | Core lib limpo, sem deps Linux. |
| 3 | **Migrate format.rs to structured output** — substituir Pango strings por `FormattedSnapshot { color_kind, label_text, ... }`. Atualizar TUI para consumir nova shape (pequena adaptação). Tests atualizados. | 2 | format.rs cross-platform, TUI continua verde. |
| 4 | **Migrate config.rs schema** — adicionar `accounts: HashMap<VendorId, Vec<AccountRef>>`, `ai_insights`, `history`. Implementar `to_json()`. Backward-compat migration de config antigo. | 3 | Novo schema TOML, exemplo em `config.example.toml`. |
| 5 | **Macros for cross-Rust-TS types** — integrar `ts-rs` ou `specta` em `torven-core` para derivar TS types das structs Rust. Output em `crates/torven-app/web/src/lib/types.ts` via build script. | 4 | Type-safe IPC contract estabelecido. |
| 6 | **Tauri 2 skeleton in torven-app** — `src-tauri/Cargo.toml`, `tauri.conf.json` (window config, tray, bundle identifier `com.torven.app`), `main.rs` com 1 command dummy (`ping`). Hello world `.app` rodável via `cargo tauri dev`. | 5 | Tauri shell rodando, tray icon visível. |
| 7 | **NSStatusBar tray + label rendering** — registrar tray icon, label dinâmico (lê de torven-core scheduler), context menu (Refresh / Open / Settings / Quit). | 6 | Menu bar visível com label que atualiza. |
| 8 | **Implement core Tauri commands** — `get_all_snapshots`, `get_vendor_snapshot`, `list_accounts`, `trigger_refresh`, `get_settings`, `save_settings`. Wrapper TS em `web/src/lib/tauri.ts`. | 7 | Frontend pode chamar core via invoke. |
| 9 | **Implement event emitters** — scheduler de refresh tick emite `vendor-snapshot-updated`; `budget-alert` quando threshold cruzado. React hooks `useVendorSnapshot`. | 8 | Reatividade backend → frontend funcionando. |
| 10 | **React Popover shell** — `Popover.tsx` com header (Insights / Refresh / Settings buttons), 5 VendorCards (data via hooks). Sem multi-account yet. Cmd+1..5 nav. | 9 | Popover funcional com vendors single-account. |
| 11 | **Keychain integration (per-vendor blob)** — implementar `crates/torven-core/src/keychain/blob.rs` via `security-framework`. Migration helper para portar keys existentes de `~/.config/...` → Keychain. | 4 | API keys secure, comando `add_account` funcional. |
| 12 | **Multi-account UI in popover** — `AccountPicker.tsx` dentro de VendorCard. Listagem + select. Reage a `vendor-snapshot-updated` per account. | 10, 11 | OpenRouter e Z.AI multi-account UX completo. |
| 13 | **SQLite history layer** — `crates/torven-core/src/history/` com rusqlite + migration 001. Scheduler grava snapshot a cada refresh. Comando `get_history(filter)`. Retention job 90d. | 4 | history.db sendo populado, queryable. |
| 14 | **Main window + sidebar + history charts** — `MainWindow.tsx` com sidebar (vendors + settings + insights history). Tabs Overview/History/Accounts/Settings por vendor. Recharts integration. | 12, 13 | Janela detalhada funcional. |
| 15 | **AI Insights eval-grade implementation** — `crates/torven-core/src/insights/` com Anthropic tool_use streaming. Trait LlmClient + Real + Mock. Prompt `prompts/insights.v1.md` + schema `v1.schema.json`. Budget guards (cost $0.05, input 8K, output 1K). Privacy redaction (account_id hash). | 13 | Backend insights pipeline pronto. |
| 16 | **AI Insights frontend integration** — `InsightsPanel.tsx` no popover (Cmd+I trigger). Streaming render token-by-token via event listener. Cost/latency display. Failure modes do PRD §6.6. | 15 | UX completo de insights. |
| 17 | **Eval runner + dataset** — `crates/torven-core/src/bin/torven-evals.rs`. Dataset inicial 30 casos rotulados em `evals/dataset.jsonl`. Judge LLM prompt em `prompts/judge.v1.md`. Saída markdown report. | 15 | Eval pipeline executável local. |
| 18 | **CI eval gate** — `.github/workflows/eval-gate.yml` roda evals em PR que toca `prompts/` ou `insights/`. Bloqueia merge se regressão >5%. PR comment com métricas. | 17 | Quality gate ativo. |
| 19 | **Settings UI in main window** — `SettingsView.tsx` editando config.toml via UI (TOML edit via toml_edit no backend, preserva comentários). Validação de inputs. Hot-reload via `config-reloaded` event. | 14 | Settings funcional sem restart. |
| 20 | **Budget alerts (UNUserNotificationCenter)** — bridge Tauri para notificações macOS nativas. Trigger via `budget-alert` event. Throttle 1x/dia por threshold. | 13 | Alertas funcionais. |
| 21 | **Diagnostics command (torven doctor)** — `run_doctor` command agrega: vendors, accounts, storage stats, schema version, eval status. UI button + copy-to-clipboard. | 13, 15 | FR-16 funcional. |
| 22 | **Sparkle auto-update integration** — `crates/torven-app/src-tauri/src/updater/`. EdDSA key generation, embed public in Info.plist, sparkle-rs binding (ou FFI). Appcast.xml hosted via GH Releases. | 6 | Auto-update funcional contra GH Releases. |
| 23 | **Release pipeline rewrite** — `.github/workflows/release.yml`: build matrix macOS x86_64 + aarch64, `tauri-action`, notarytool via secrets (Apple ID + app-specific password), Sparkle EdDSA sign, .dmg gen, appcast.xml publish. | 22 | tag push → release publicado, signed, notarized. |
| 24 | **First-run onboarding wizard (FR-14)** — `OnboardingWizard.tsx` first launch detection, 5 vendor cards com Login/Add key CTAs, Skip for now. | 12, 19 | Onboarding funcional. |
| 25 | **CHANGELOG + README + CLAUDE.md rewrite for v1.0** — rewrite completo do README (AI Insights hero, screenshot, eval table, install via .dmg, comparison table do @analyst). CHANGELOG marca v1.0 pivot. CLAUDE.md release checklist updated. | 23 | Portfolio-ready artifacts. |

**Total estimado:** 25 stories. R-5 do PRD pede ≤1d por story; algumas (15, 17, 22, 23) podem partir em 2 substories. @sm decide granularidade final.

---

## 6. Risks (arquiteturais)

Riscos técnicos do pivot. Não inclui riscos de produto (esses estão no PRD §9).

### AR-1 — Tauri 2 maturity em macOS específico (MEDIUM)
- Tauri 2 lançou stable em out/2024. Algumas APIs (`tray-icon`, `window-state`, `notification` plugins) ainda evoluem. Risk de breaking changes em minor versions.
- **Mitigação:** pin de versão exato em `Cargo.toml` (não `^2.0`, mas `=2.X.Y`). Monitorar Tauri release notes. CI gate `cargo update --dry-run` semanal.

### AR-2 — `security-framework` crate cobertura macOS Keychain (MEDIUM)
- Crate é o binding oficial Rust para Apple Security APIs. Versão 2.x estável mas APIs Apple subjacentes (SecItem) têm edge cases de ACL (e.g. "always allow this app" prompt behavior).
- **Mitigação:** spike de 0.5d na Story 11 para validar add/read/update/delete blob com 5 accounts antes do commit. Fallback documented: se `security-framework` quebrar em macOS 14+, alternativa é FFI direto via `objc2` + Security.framework — refactor isolado.

### AR-3 — Universal binary build complexity (LOW-MEDIUM)
- macOS aarch64 + x86_64 universal binary via `cargo build --target aarch64-apple-darwin && cargo build --target x86_64-apple-darwin && lipo -create ...` ou via `cargo-bundle`. Tauri 2 + GitHub Actions tem patterns conhecidos via `tauri-apps/tauri-action`.
- **Mitigação:** seguir tauri-action universal target. Story 23 inclui smoke test que verifica `lipo -info Torven.app/Contents/MacOS/torven` lista ambas as architectures.

### AR-4 — Notarization gotchas (MEDIUM)
- Apple notarytool exige hardened runtime entitlements corretas, app-specific password, Apple ID with paid Developer Program. Falhas comuns: missing `NSAppleEventsUsageDescription`, missing `com.apple.security.cs.allow-jit` se webkit precisa JIT.
- **Mitigação:** Story 23 inclui notarization dry-run em CI antes do primeiro release real. Documentação explícita em `crates/torven-app/src-tauri/entitlements.plist`. Smoke test `spctl -a -v Torven.app` post-notarize.

### AR-5 — SQLite migration safety across versions (LOW)
- Schema migrations forward-only via `rusqlite_migration`. Risk: bug em migration v2 corrompe DB v1 do usuário.
- **Mitigação:** disciplina de migrations: cada migration tem teste em `crates/torven-core/tests/history_e2e.rs` que cria DB v(N-1), aplica migration, valida shape. Backup automático da DB antes de cada migration (`history.db.backup-{ts}`) — restaurável via `torven doctor`.

### AR-6 — Anthropic API streaming + tool_use partial JSON parsing (MEDIUM)
- Anthropic streaming envia `input_json_delta` events com chunks parciais JSON dentro do tool_use. Parse partial JSON é não-trivial (chaves não fechadas).
- **Mitigação:** usar crate `partial-json` ou `streaming-parser` que tolera unbalanced braces. Para UI streaming, basta exibir o `accumulated_text` (sem parse estrutural até final). Só faz parse estrutural no `message_stop` event. Tests em `crates/torven-core/tests/insights_e2e.rs` com mockito gravando responses Anthropic reais.

### AR-7 — React popover positioning relative to NSStatusBar item (MEDIUM)
- Tauri 2 tray API permite criar popover window mas posicionamento relativo ao tray icon não é built-in cross-platform. Em macOS especificamente, precisa cálculo manual via NSStatusBar bounds.
- **Mitigação:** usar plugin `tauri-plugin-positioner` (cross-platform tray-relative positioning) ou bridge custom Objc se plugin não cobre. Spike na Story 7. Fallback: window posicionada por absolute coordinates calculadas via Tauri runtime info.

### AR-8 — Bundle size creep beyond 25MB NFR (LOW-MEDIUM)
- NFR-5: bundle <25MB. Tauri 2 webview bundle baseline ~10-12MB. React + Recharts + dependencies ~500KB gzip. Rust binary ~5-8MB stripped. Total estimado ~17-21MB inicial.
- **Mitigação:** monitor em CI (build step que reporta `.app` size em PR comment). Cargo profile `release` com `strip = "symbols"`, `lto = "fat"`, `codegen-units = 1`. Vite production build com tree-shaking + code-splitting. Se ultrapassar 22MB, audit deps (substituir Recharts por Visx min-cost, deferir features).

---

## 7. Open questions para @ux-design-expert

Estas decisões precisam ser fechadas pelo @ux-design-expert antes de @sm criar stories de UI específicas (Stories 10, 12, 14, 19, 24 da migration sequence).

### UX-Q1 — Densidade do popover: 5 vendors visíveis sem scroll?
O PRD pede 5 cards no popover ~360×420px. Cada VendorCard típico (header com nome + ícone + status pill + delta + main metric) tem altura natural ~80-100px. 5 × 80 = 400px + header 40px = 440px → spill. Solução: cards colapsáveis (1 expanded + 4 collapsed compactos a ~40px), ou popover ligeiramente maior (480px height) ou scroll vertical (UX feio em popover). @ux decide.

### UX-Q2 — Account picker shape dentro de VendorCard
Quando OpenRouter tem 4 accounts, como mostrar? Opções: (a) combobox dropdown no topo do card; (b) segmented control horizontal (4 chips clicáveis); (c) expandable list embaixo do card; (d) "All accounts" agregado por default + link "View per account →" abre janela detalhada. Trade-off densidade × clarity × clicks-to-task.

### UX-Q3 — AI Insights: input ou output puro?
PRD §6 não decide se o usuário pode passar contexto custom ao Insights ("foca em última semana", "ignora Codex"). Opções: (a) zero input — clique em Insights, payload é fixed `last-7-days all-vendors`; (b) textarea opcional pra "context"; (c) preset selector (last 7d / last 30d / specific vendor / specific account). Implica latência (mais tokens input) e complexidade UI.

### UX-Q4 — Menu bar label format multi-account aggregation
Quando OpenRouter tem 4 accounts, label da menu bar mostra o quê? (a) agregado total `OpenRouter $142`; (b) account em maior pressão `ClienteAcme $89/$100`; (c) rotação a cada N segundos. PRD FR-8 deixa isso em aberto para vendors API-key. @ux decide.

### UX-Q5 — Budget alert UX surface
PRD FR-11: notificação macOS nativa via UNUserNotificationCenter. Mas também precisa de in-app surface (banner no popover? badge no VendorCard? badge no tray icon? toast no main window?). Quando usuário tem o popover aberto e budget cruza threshold, qual interação preferida?

---

## Resumo executivo (para o user)

**Decisões finais (ADR-1 a ADR-8):**

1. Workspace Cargo split em 3 crates: `torven-core` + `torven-app` + `torven-tui` (eval runner como `[[bin]]` em core).
2. macOS Keychain: blob JSON per vendor (1 prompt/sessão) + SQLite para metadata queryable — híbrido com UUID como bridge.
3. Auto-update via **Sparkle** (macOS-native, EdDSA defense-in-depth contra Apple cert lapse).
4. IPC híbrido: Tauri commands para request/response, events para streaming (AI Insights tokens + snapshot push).
5. History storage em **SQLite via rusqlite**, schema versionado, retention 90d, queries temporais otimizadas para AI Insights.
6. Charts em **Recharts** (dev velocity > polish marginal para single-dev pace).
7. AI Insights JSON Schema via **Anthropic tool_use mode** (parse failures ~0%, tokens economy, portfolio signal).
8. Eval runner em **Rust nativo** dentro de `torven-core` (single source of truth com prod, zero drift, CI cargo-only).

**Top-3 riscos arquiteturais:**

- **AR-4 (Notarization gotchas)**: Apple notarytool friction é o blocker clássico de macOS release. Mitigação obrigatória: dry-run em CI antes do primeiro release real (Story 23).
- **AR-7 (Popover positioning vs NSStatusBar)**: Tauri 2 não tem solução cross-platform clean; pode exigir bridge Objc custom. Spike na Story 7.
- **AR-2 (security-framework Keychain coverage)**: spike 0.5d na Story 11 antes de comprometer com blob JSON design.

**Próximos passos:**

- **@ux-design-expert** precisa fechar 5 questões (UX-Q1 a UX-Q5) antes do @sm criar stories de UI específicas (Stories 10, 12, 14, 19, 24). Principais: densidade popover vs 5 vendors, shape do account picker, AI Insights input model, label format multi-account, budget alert surface.
- **@po** valida o ADR via `po-master-checklist` em paralelo à UX spec.
- **@sm** pode começar Story 1 (Bootstrap workspace) imediatamente — não depende de UX.

**FIM do ADR v1.0 draft.**
