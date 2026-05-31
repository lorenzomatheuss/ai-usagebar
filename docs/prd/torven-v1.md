# Torven v1.0 — Product Requirements Document (PRD)

> **Autor:** @pm (Bob — Strategist)
> **Data:** 2026-05-31
> **Status:** Draft v1 (awaiting @architect ADRs e @ux-design-expert spec)
> **Predecessores:** `docs/research/competitive-analysis.md`, `docs/research/naming-shortlist.md`
> **Sucessores:** ADRs do @architect (Tauri internals, SQLite schema, IPC), UX spec do @ux-design-expert, epics/stories do @po e @sm

---

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-05-31 | 0.1.0 | PRD inicial pós-rebrand, primeiro release nativo macOS | @pm |

---

## 1. Vision

**Torven** é o primeiro tracker de uso de LLMs local-first com **AI Insights eval-grade** e **multi-account por cliente**, distribuído como app nativo macOS (menu bar + popover + janela completa). Para AI engineers freelance, consultores e devs em times pequenos que usam Claude Code, Codex, Gemini, Z.AI e/ou OpenRouter no dia a dia, Torven responde duas perguntas que nenhum competidor responde com profundidade: **"por onde está vazando meu spend?"** (insight acionável gerado por LLM com pipeline de eval público) e **"quanto cada cliente me custa?"** (multi-account por API key com tagging por cliente). O produto existe porque o nicho está saturado de "menu bar tracker #14" sem inteligência e sem segmentação por contexto de trabalho — gaps que Torven ataca com structured outputs versionados, cost/latency budgets explícitos, e SQLite local sob controle total do usuário.

---

## 2. Goals & Non-Goals

### Goals (mensuráveis)

1. **Diferenciador AI Insights validado:** entregar pipeline de eval com dataset rotulado ≥30 casos, métricas (faithfulness, relevance, latency p50/p95, cost p50/p95) publicadas em CI, e prompts versionados em `prompts/insights.v{N}.md` git-tracked.
2. **Multi-account funcional em ≥2 vendors API-key (OpenRouter e Z.AI):** usuário cria N accounts por vendor, atribui `tag`, `budget_usd` e `description`, e vê breakdown por account no popover e janela detalhada.
3. **Performance percebida nativa-grade:** p95 popover open <100ms (cold start), refresh de menu bar <30s, bundle .app <25MB, idle CPU <0.5%, idle RAM <80MB.
4. **Distribuição zero-friction macOS:** binário notarizado Apple Developer + auto-update via GitHub Releases, suporte macOS 13.0+ (Ventura+), arm64 primary + x86_64 build.
5. **Portfolio signal para hiring AI Engineer:** README + landing page que comunicam profundidade LLM engineering (evals públicos, prompt versioning, structured outputs, cost budgets) em vez de "menu bar tracker".

### Non-Goals (explícitos no v1.0)

1. **Sem cloud sync.** Nenhum dado do usuário (histórico de uso, API keys, insights) sai da máquina exceto a chamada explícita para Anthropic API ao gerar um insight (e mesmo essa carrega apenas agregado anonimizado — ver §6). Cloud sync opcional fica para **v1.5**.
2. **Sem multi-account para vendors OAuth (Anthropic Claude Max, OpenAI Codex, Gemini).** Esses ficam single-account no v1.0. Multi-account via API key fallback fica para **v1.5**.
3. **Sem team accounts / multi-user / RLS / billing.** Torven v1.0 é desktop standalone single-user. Teams SaaS fica para **v2.0**.
4. **Sem suporte nativo Windows/Linux.** O TUI cross-platform existente (`src/tui/`) cobre Linux de forma gratuita; native UI Windows/Linux talvez nunca seja roadmap.
5. **Sem mobile companion.** Sem app iOS/iPadOS no v1.0. Avaliar em v2.0 se houver demanda.

---

## 3. Target Users (Personas)

### Persona 1 — Marina, freelance AI Engineer (São Paulo, BR)

- **Contexto:** 31 anos, 6 anos como dev (full-stack → AI eng pivot em 2024). Atende 3 clientes simultâneos via consultoria PJ. Cada cliente tem seu próprio OpenRouter account (compliance/billing separados) e usa Claude Code com a key dela mesma para protótipos antes de migrar para o ambiente do cliente.
- **Problema:** "Eu não sei quanto cada cliente me custou esse mês. Tenho que entrar no dashboard do OpenRouter três vezes, filtrar manualmente, somar. E quando vou cobrar overhead de tokens, sempre subestimo porque esqueço de incluir as sessões de exploração."
- **Jobs-to-be-done:**
  - JTBD-1: ver gasto agregado por cliente sem entrar em três dashboards.
  - JTBD-2: receber alerta proativo quando algum cliente estoura budget combinado.
  - JTBD-3: gerar relatório mensal de uso por cliente para anexar no invoice.
- **Sucesso medido:** Marina abre Torven 4+ vezes por semana, dispara 1-2 AI Insights por semana, e cita Torven como "essencial pro meu workflow" em uma review depois de 30 dias.

### Persona 2 — Alex, Senior Software Engineer em Cursor-like startup (San Francisco, US)

- **Contexto:** 34 anos, 10 anos backend → 2 anos em IDE/AI tooling startup (~25 funcionários). Plano Claude Max pessoal para experimentação fora do trabalho + Codex pessoal + acesso a um OpenRouter team account compartilhado pela engenharia (8 devs). Pesquisa providers novos constantemente.
- **Problema:** "Tenho três planos diferentes rodando ao mesmo tempo. Claude Max reseta em janela de 5h, Codex tem cota mensal, e o OpenRouter team a gente compartilha. Eu nunca sei qual plano está quase estourando até o Claude responder 'rate limited'. E o time-line de spend do OpenRouter virou tribal knowledge — ninguém sabe quem está queimando o quê."
- **Jobs-to-be-done:**
  - JTBD-1: enxergar a janela de 5h Claude Max + cota mensal Codex + spend OpenRouter num só lugar, na menu bar.
  - JTBD-2: receber insight "você está 40% acima do pace habitual essa semana, principalmente em Opus entre 21h-23h" e poder agir.
  - JTBD-3: validar via eval pipeline público que o insight é confiável (Alex tem viés cético contra "AI gimmicks").
- **Sucesso medido:** Alex compartilha Torven num Slack interno do time, dois colegas instalam, e Alex publica um tweet/blog mencionando "AI Insights with public eval pipeline — finally a tracker that doesn't hand-wave the AI part".

---

## 4. Functional Requirements (FRs)

> Priority: **L0** = must-have v1.0 (release blocker), **L1** = should-have v1.0 (ship sem se necessário, com debt registrado), **L2** = nice-to-have (defer para v1.5+).

---

### FR-1 — Vendor: Anthropic (Claude Max + Console API)

- **Priority:** L0
- **Description:** Torven coleta uso atual da janela de 5h e contagem semanal/mensal de Claude via OAuth flow (Claude Max) ou API key (Console). Reutiliza fetcher Rust existente em `src/anthropic/`.
- **Acceptance Criteria:**
  - **Given** o usuário fez login OAuth com Claude Max
  - **When** Torven faz refresh (intervalo configurável, default 30s)
  - **Then** menu bar mostra `Claude: X% / 5h` com cor calm/amber/red conforme thresholds (75/90/95%), popover mostra detalhe (tokens, modelo Opus/Sonnet/Haiku, reset time)
  - **And** falha de rede ou token expirado mostra ícone `⚠` sem crashar o app
  - **And** OAuth credentials ficam em `~/Library/Application Support/Torven/anthropic.credentials.json` com permissões 600

---

### FR-2 — Vendor: OpenAI Codex

- **Priority:** L0
- **Description:** Torven coleta uso de cota mensal Codex via OAuth flow. Reutiliza fetcher Rust em `src/openai/`.
- **Acceptance Criteria:**
  - **Given** o usuário fez login OAuth com Codex
  - **When** Torven faz refresh
  - **Then** popover mostra `Codex: X / Y messages this month` + reset date
  - **And** suporta refresh de token expirado sem intervenção do usuário
  - **And** falha de auth mostra CTA "Re-login Codex" no popover

---

### FR-3 — Vendor: Google Gemini

- **Priority:** L0
- **Description:** Torven coleta uso Gemini via OAuth (5º vendor implementado, em `src/gemini/`).
- **Acceptance Criteria:**
  - **Given** o usuário fez login OAuth com Gemini
  - **When** Torven faz refresh
  - **Then** popover mostra cota Gemini (formato a ser confirmado pelo schema `src/gemini/types.rs`)
  - **And** suporta refresh de token e fallback gracioso em caso de drift de schema (capturado por `tests/live.rs`)

---

### FR-4 — Vendor: Z.AI

- **Priority:** L0
- **Description:** Torven coleta uso Z.AI via API key, com suporte a multi-account (ver FR-6). Reutiliza `src/zai/`.
- **Acceptance Criteria:**
  - **Given** o usuário configurou pelo menos uma Z.AI account (API key + name + tag)
  - **When** Torven faz refresh por account
  - **Then** popover mostra uso agregado total Z.AI + breakdown por account
  - **And** janela detalhada permite filtrar histórico por account específica

---

### FR-5 — Vendor: OpenRouter

- **Priority:** L0
- **Description:** Torven coleta uso OpenRouter via API key, com suporte a multi-account (ver FR-6). Reutiliza `src/openrouter/`.
- **Acceptance Criteria:**
  - **Given** o usuário configurou pelo menos uma OpenRouter account
  - **When** Torven faz refresh
  - **Then** popover mostra `$X.XX / $Y.YY (Z%)` por account + total agregado
  - **And** breakdown por modelo (claude-3.5-sonnet, gpt-4o, etc.) disponível na janela detalhada

---

### FR-6 — Multi-Account: configuração e seleção

- **Priority:** L0 (para OpenRouter e Z.AI)
- **Description:** Para vendors API-key (OpenRouter, Z.AI no v1.0), Torven suporta N accounts por vendor. Cada account tem `name` (free-form, único por vendor), `api_key` (criptografada em rest via macOS Keychain), `description?`, `budget_usd?`, `tag?` (`client` | `personal` | `team`). OAuth vendors (Anthropic, Codex, Gemini) ficam single-account no v1.0.
- **Acceptance Criteria:**
  - **Given** o usuário abre Settings → Accounts
  - **When** adiciona uma nova OpenRouter account preenchendo `name="ClienteAcme"`, `api_key=sk-or-...`, `tag=client`, `budget_usd=200`
  - **Then** a account é persistida (API key no Keychain, metadados em SQLite), aparece no popover account picker, e usage é coletado independentemente
  - **And** Given múltiplas accounts existentes, **When** usuário clica no account picker do popover, **Then** vê lista com `name (tag) — $spent / $budget` e pode filtrar visualização por account ou ver "All accounts" agregado
  - **And** budget_usd atingido dispara alerta (ver FR-10)
  - **And** API key é mostrada mascarada (`sk-or-****abc`) na UI; chave plaintext nunca aparece em logs nem export

---

### FR-7 — AI Insights (feature L0 crítica — spec detalhada em §6)

- **Priority:** L0
- **Description:** Botão "Insights" no popover dispara chamada à Anthropic Messages API que analisa histórico recente de uso (agregado, anonimizado) e retorna análise estruturada via JSON Schema. Streaming UI, prompts versionados, eval pipeline público, cost/latency budgets explícitos.
- **Acceptance Criteria:**
  - **Given** o usuário tem ≥7 dias de histórico SQLite
  - **When** clica em "Insights" no popover
  - **Then** request é enviado para Anthropic API (key do user lida do Keychain ou perguntada se ausente) com prompt `prompts/insights.v{N}.md` + payload agregado
  - **And** resposta é streaming, com primeiro token visível em <2s (p95)
  - **And** resposta final é JSON estruturado: `{ headline: string, insights: [{type, severity, message, evidence}], recommendation: string, cost_usd: f64 }`
  - **And** custo real da chamada é exibido (`"This insight cost $0.03"`) no UI
  - **And** prompt version usada (ex: `v3`) é mostrada no detalhe expandido
  - **And** budgets: input ≤8000 tokens, output ≤1000 tokens; ultrapassar dispara warning UI antes de enviar
  - **And** privacidade: payload enviado NÃO contém API keys, NÃO contém nomes de clientes (apenas `account_id` hash), NÃO contém prompts/outputs reais — apenas agregados numéricos por vendor + janela temporal
  - **And** falha de rede ou erro Anthropic exibe fallback gracioso com mensagem clara, não crasha o app

---

### FR-8 — Menu Bar (NSStatusBar)

- **Priority:** L0
- **Description:** Torven aparece na menu bar macOS com texto dinâmico curto + ícone. Click esquerdo abre popover, click direito abre menu contextual (Settings, Quit, Refresh, About).
- **Acceptance Criteria:**
  - **Given** Torven está rodando
  - **When** o usuário olha a menu bar
  - **Then** vê um label dinâmico com a métrica mais pressionada do momento (`Claude 87% 5h` ou `OpenRouter $42` etc.), conforme regra de prioridade `[PM-DECISION]`: vendor com maior `% usado / threshold` ganha o slot; em empate, prioridade fixa Anthropic > OpenRouter > Codex > Gemini > Z.AI
  - **And** ícone muda de cor (calm → amber em ≥75% / `>=80%` budget → red em ≥95% / budget atingido)
  - **And** label é truncado para max 18 chars na menu bar
  - **And** click esquerdo abre popover ancorado abaixo do ícone (~360x420px)
  - **And** click direito abre menu contextual com `Open Detailed View`, `Settings...`, `Refresh Now (Cmd+R)`, `Quit Torven (Cmd+Q)`

---

### FR-9 — Popover (vendor cards + account picker + Insights)

- **Priority:** L0
- **Description:** Popover compacto mostra todos os 5 vendors em cards densos + account picker (quando multi-account configurado) + botão "Insights" no topo + footer com link para janela detalhada.
- **Acceptance Criteria:**
  - **Given** o usuário abriu o popover
  - **When** vê a tela
  - **Then** vê header com botão "Insights" (Cmd+I), botão "Refresh" (Cmd+R), botão "Settings" (Cmd+,)
  - **And** vê 5 cards (um por vendor configurado; vendors não-configurados ficam acinzentados com CTA "Login")
  - **And** se vendor tem multi-account ativo, card mostra account picker dropdown no topo do card OU lista compacta de accounts (decisão de UX detalhada delegada para @ux-design-expert)
  - **And** keyboard navigation funciona: Cmd+1..5 foca vendor N, ↑↓ navega accounts dentro do card focado
  - **And** popover open p95 <100ms (cold), <50ms (warm) — instrumentado e visível em `torven doctor`
  - **And** popover fecha em Esc, click fora, ou click no ícone da menu bar novamente

---

### FR-10 — Janela detalhada (full window)

- **Priority:** L0
- **Description:** Janela completa (não popover) acessada via menu contextual ou Cmd+D no popover. Mostra tabs por vendor com histórico, charts, breakdown por account/modelo, e seção de Settings.
- **Acceptance Criteria:**
  - **Given** o usuário abriu a janela detalhada
  - **When** vê a tela
  - **Then** vê sidebar com lista de vendors + seção "Settings" + seção "Insights History"
  - **And** ao clicar em um vendor, vê tabs: `Overview` (uso atual), `History` (chart últimos 30 dias), `Accounts` (se multi-account), `Settings` (API key, refresh interval)
  - **And** charts são SVG/Canvas (zero dependência de Chart.js bloated — `[needs decision: @architect escolhe render lib]`)
  - **And** janela é redimensionável, min 800x600px, default 1024x720px
  - **And** janela persistir tamanho/posição entre runs

---

### FR-11 — Budget Alerts (notification)

- **Priority:** L0
- **Description:** Quando `budget_usd` de uma account é atingido ou ultrapassado, Torven dispara notificação macOS nativa (UNUserNotificationCenter).
- **Acceptance Criteria:**
  - **Given** usuário configurou `budget_usd=100` em OpenRouter account "ClienteAcme"
  - **When** spend atinge 80% ($80)
  - **Then** notificação macOS aparece: "ClienteAcme at 80% ($80 / $100)"
  - **And** thresholds default: 80%, 95%, 100%, 120% (acima do budget)
  - **And** cada threshold dispara no máximo 1x por dia (anti-spam)
  - **And** usuário pode desabilitar notificações por account em Settings
  - **And** click na notificação abre a janela detalhada filtrada por essa account

---

### FR-12 — History Persistence (SQLite local)

- **Priority:** L0
- **Description:** Torven persiste snapshots de uso por vendor/account em SQLite local em `~/Library/Application Support/Torven/history.db`. Schema git-tracked, migrações versionadas.
- **Acceptance Criteria:**
  - **Given** Torven está coletando uso
  - **When** completa um refresh bem-sucedido
  - **Then** insere row em `usage_snapshots(vendor, account_id, timestamp, raw_payload_json, derived_metrics_json)`
  - **And** retention default: 90 dias (config em Settings)
  - **And** schema migrations rodam automaticamente no startup, com fallback de erro claro
  - **And** export CSV/JSON do histórico disponível em Settings → Data
  - **And** schema detalhado é `[needs decision: @data-engineer / @architect — incluir tabelas accounts, vendors, insights_history]`

---

### FR-13 — Settings UI (TOML editado via UI)

- **Priority:** L0
- **Description:** Settings é uma tab/seção dentro da janela detalhada que edita o `config.toml` via UI (não só CLI). Reaproveita lógica existente em `src/tui/settings.rs` (toml_edit-backed, auto-signal post-save).
- **Acceptance Criteria:**
  - **Given** o usuário abriu Settings
  - **When** edita refresh interval, thresholds, accounts, retention, AI Insights config
  - **Then** mudanças são persistidas em `~/Library/Application Support/Torven/config.toml` via toml_edit (preserva comentários)
  - **And** mudanças entram em efeito sem restart (signal interno equivalente ao SIGUSR1 do Waybar)
  - **And** UI valida inputs antes de salvar (ex: refresh interval ≥10s, budget_usd ≥0)
  - **And** "Edit config.toml directly" opcional abre arquivo no editor padrão (`open -t`)

---

### FR-14 — Onboarding / first-run experience

- **Priority:** L1
- **Description:** Primeiro launch mostra wizard guiado: "Choose your vendors → Login/add key → Done".
- **Acceptance Criteria:**
  - **Given** usuário abriu Torven pela primeira vez (sem config.toml)
  - **When** janela inicial aparece
  - **Then** wizard com 5 cards de vendor, cada um com CTA "Login" (OAuth) ou "Add API key" (API-key vendors)
  - **And** "Skip for now" disponível em todos os steps
  - **And** ao concluir wizard, popover é mostrado uma vez automaticamente com tooltip "This is your menu bar app"

---

### FR-15 — Auto-update

- **Priority:** L1
- **Description:** Torven verifica updates em GitHub Releases periodicamente, baixa e instala signed `.app` via Sparkle ou Tauri updater.
- **Acceptance Criteria:**
  - **Given** Torven está rodando
  - **When** versão nova é detectada (intervalo: 24h, configurável)
  - **Then** notification "Torven {new_version} available — Restart to update" aparece
  - **And** assinatura Apple Developer é verificada antes de instalar
  - **And** rollback possível se update falhar (preservar versão anterior)
  - **And** decisão Sparkle vs Tauri updater: `[needs decision: @architect — Sparkle é macOS-nativo padrão, Tauri updater é cross-platform mas menos batido em macOS]`

---

### FR-16 — Diagnostics command (`Torven Doctor`)

- **Priority:** L1
- **Description:** Botão "Run Diagnostics" em Settings ou comando CLI `torven doctor` mostra estado interno (vendors configurados, último refresh por vendor, schema version SQLite, AI Insights eval status, current prompt version).
- **Acceptance Criteria:**
  - **Given** o usuário roda diagnostics
  - **When** completa
  - **Then** vê relatório markdown com seções: Vendors, Accounts, Storage, AI Insights, System (macOS version, app version, bundle path)
  - **And** botão "Copy to clipboard" facilita envio de bug reports
  - **And** zero PII (API keys, names) no output

---

## 5. Non-Functional Requirements (NFRs)

### NFR-1 — Performance

- p95 popover open <100ms cold, <50ms warm
- p99 popover open <200ms cold
- Menu bar refresh interval default 30s, mínimo configurável 10s
- p95 menu bar label update após refresh <500ms (refresh API call não bloqueia render)
- Cold start app launch <800ms (binário até primeira renderização do menu bar)

### NFR-2 — AI Insights Cost & Latency

- Custo máximo por insight: **$0.05** (Anthropic Claude 3.5 Sonnet ou equivalente). Acima disso, warning UI antes de enviar e botão "Send anyway"
- Cost mensal estimado por usuário típico (10 insights/mês): <$0.50
- AI Insights p95 end-to-end <8s; primeiro token streaming visível em <2s
- Rate limit: max 1 insight por minuto por usuário (anti-abuse e cost guard)

### NFR-3 — Privacy

- Zero telemetria. Sem analytics, sem error reporting automático (a menos que usuário opte-in explicitamente em Settings — opt-in OFF por default).
- Zero envio de dados além de: (a) requests para APIs dos próprios vendors (uso legítimo) e (b) chamada Anthropic API para AI Insights com payload anonimizado e agregado (ver §6).
- API keys criptografadas em rest via macOS Keychain (não plaintext em config.toml)
- Logs em `~/Library/Logs/Torven/` redatam keys (mesma disciplina do `src/anthropic/creds.rs`)
- Crash dumps (se habilitados) NÃO incluem API keys nem histórico de uso detalhado

### NFR-4 — macOS Support

- macOS 13.0+ (Ventura) minimum
- arm64 primary build (M-series Macs)
- x86_64 build (Intel Macs) — universal binary preferido se tamanho permitir
- Funciona em macOS dark/light mode, segue accent color do sistema
- Suporte a multiple displays (popover ancora no display do clique)
- Notarização Apple Developer obrigatória para release público (signed + notarized via `notarytool` em GitHub Actions)

### NFR-5 — Bundle Size

- `.app` bundle <25MB (incluindo webview Tauri 2 + Rust core + assets)
- Stretch goal: <18MB
- Tauri 2 lazy loading de frontend assets (vide best practices)

### NFR-6 — Reliability

- App nunca deve crashar em decorrência de drift de API de vendor (fallback `⚠` icon + erro logado, padrão herdado do Waybar)
- Atomic writes em config.toml e SQLite (tempfile + persist, herdado do `src/widget/run.rs`)
- Cache de uso sobrevive a quits/restarts (last-known-good state restaurado se refresh falhar no startup)
- Smoke tests `make smoke` cobrem cada vendor contra endpoints reais (existente, manter)

### NFR-7 — Updates & Distribution

- Distribuição primária: download `.dmg` em github.com/.../releases
- Distribuição secundária: Homebrew Cask (futuro pós-v1.0)
- Mac App Store NÃO é roadmap v1.0 (sandbox restringe demais multi-account Keychain access; reavaliar v1.5+)
- Updates auto via Sparkle ou Tauri updater (`[needs decision: @architect]`)

### NFR-8 — Internationalization

- v1.0: **English only** (UI strings em inglês). PRD escrito em PT-BR mas produto é EN.
- Strings extraídas em arquivo único (`src/i18n/en.json` ou equivalente Tauri) para facilitar i18n futura
- Datas/números seguem locale do sistema macOS

### NFR-9 — Accessibility

- Target: **WCAG AA** (contraste, keyboard navigation, focus indicators)
- VoiceOver compatibilidade nos elementos críticos (vendor cards, account picker, insights output)
- Atalhos de teclado documentados (Cmd+R, Cmd+I, Cmd+D, Cmd+,, Cmd+Q, Cmd+1..5)

### NFR-10 — Observability (internal)

- Logs estruturados (JSON ou key-value) em `~/Library/Logs/Torven/`
- Log levels: ERROR / WARN / INFO / DEBUG (DEBUG off por default, toggle em Settings)
- AI Insights latência e custo logados por chamada (para eval pipeline e cost tracking pessoal do usuário)
- Métricas internas exposed via `torven doctor` (ver FR-16)

---

## 6. AI Insights — capítulo dedicado (feature L0 crítica)

Esta seção expande FR-7 e é o coração defensável do Torven vs. competição. Tudo aqui é **release-blocker**.

### 6.1 Eval Pipeline

- **Dataset:** mínimo **30 casos rotulados** no v1.0 (target stretch: 100 no v1.0, 200 no v1.5). Cada caso é tupla `(usage_snapshot_anonimizado, expected_insight_categoria, ideal_response_partial)`.
- **Localização:** `evals/insights/dataset.jsonl` (git-tracked). Schema git-tracked em `evals/insights/schema.md`.
- **Métricas computadas em CI a cada PR que toca `prompts/insights.v*.md` ou código de insights:**
  - **Faithfulness:** % de claims no output que são suportados por evidência no input (medido via judge LLM call ou regex/heurística com fallback humano em batch)
  - **Relevance:** % de outputs categorizados como "actionable" pelo eval (label binário ou Likert 1-5)
  - **Latency p50/p95** (sintético em CI usando mock LLM com latency distribuída)
  - **Cost p50/p95** (calculado via token counter, sem chamada real em CI)
- **Test runner:** comando `cargo test --features evals -- --test-threads=1 eval_insights` ou equivalente Tauri-side. Rodado em GitHub Actions a cada PR.
- **Baseline e target publicados em README:**
  - v1.0 baseline target (a estabelecer no primeiro run): faithfulness ≥0.85, relevance ≥0.80
  - Regressão >5% bloqueia merge

### 6.2 Prompt Versioning

- **Localização:** `prompts/insights.v{N}.md` (git-tracked, semver-style: v1, v2, v3...)
- **Estrutura de cada prompt:**
  - YAML frontmatter: `version`, `created_at`, `author`, `superseded_by?`, `eval_baseline_score`
  - Body markdown com placeholders `{{usage_payload}}`, `{{user_context}}`, etc.
- **Changelog:** `prompts/CHANGELOG.md` com bullets por versão (por que mudou, métrica que melhorou)
- **Active prompt:** definido em `config.toml` (`ai_insights.prompt_version = "v3"`) — permite usuário avançado escolher versão alternativa
- **UI:** versão usada por chamada exibida no detalhe expandido do insight (`Generated with prompt v3 — see prompts/insights.v3.md`)

### 6.3 Structured Outputs

- **Modo:** Anthropic Messages API com `tools` ou JSON Schema enforcement (decisão @architect: tool-use é mais robusto que prompted JSON em Claude 3.5+)
- **Schema (JSON):**

```json
{
  "headline": "string (max 120 chars, primary takeaway)",
  "insights": [
    {
      "type": "trend | anomaly | budget_risk | optimization_opportunity",
      "severity": "info | warning | critical",
      "message": "string (max 280 chars)",
      "evidence": "string (cites specific numbers from input)"
    }
  ],
  "recommendation": "string (max 280 chars, single actionable next step)",
  "cost_usd": "number (real cost of this generation)"
}
```

- **Validação:** parsing falha grácil → fallback UI "Couldn't parse insight. Raw output saved to logs." com botão "Retry"
- **Schema version:** versionada junto com prompt (`prompts/insights.v3.schema.json`)

### 6.4 Cost / Latency Budgets

- **Input budget:** 8000 tokens (agregado payload + prompt). Tokens contados antes de enviar; ultrapassar dispara warning UI com opção truncate ou cancel.
- **Output budget:** 1000 tokens. `max_tokens=1000` no request.
- **Cost ceiling por insight:** $0.05. Calculado antes do envio (estimativa baseada em token count + preço do modelo selecionado). Acima → warning + "Send anyway" CTA.
- **Latency budget:** abort após 15s sem resposta (timeout); UI mostra "Took too long, try again."
- **Display:** UI mostra `Cost: $0.0X | Latency: Xs | Prompt: vN | Schema: vN` em cada insight gerado.

### 6.5 Privacy do Payload

- Request para Anthropic **NÃO inclui:**
  - API keys do usuário (de nenhum vendor)
  - Conteúdo de prompts/outputs do usuário (não temos esses dados — só uso agregado)
  - Nomes de accounts/clientes em plaintext (substituídos por `account_id` hash determinístico)
  - Path local, hostname, username macOS
- Request para Anthropic **inclui:**
  - Agregados numéricos por vendor (tokens usados, custo, janela temporal)
  - Tags genéricas (`client`, `personal`, `team` — sem nome do cliente)
  - Histórico last-N-days (default 7) com snapshots em granularidade horária
- **Auditoria:** payload exato logado em `~/Library/Logs/Torven/ai-insights-payload.jsonl` (rotacionado, ≤30 dias) para usuário inspecionar — mesma disciplina dos smoke tests Rust

### 6.6 Failure Modes

| Failure | UX |
|---|---|
| Anthropic API key não configurada | CTA modal: "Configure your Anthropic API key in Settings to use AI Insights" + link |
| Sem histórico (<7 dias) | Disabled state no botão + tooltip "Need 7+ days of usage data" |
| Cost budget excedido | Warning modal "This insight will cost ~$0.08, above your $0.05 limit. Continue?" |
| Rate limit Anthropic (429) | Toast "Rate limited. Try again in N seconds." + retry button |
| Malformed JSON response | "Couldn't parse insight" + raw output saved + Retry CTA |
| Network failure | "Offline. Insights need internet." + retry button |
| Schema validation fail | Same as malformed JSON |

---

## 7. Out of Scope (v1.0 → v1.5 / v2.0)

Lista explícita do que **NÃO** entra no v1.0 com seu destino:

| Item | Destino | Motivo |
|------|---------|--------|
| Cloud sync (cross-device history) | **v1.5** | Requer backend; quebra local-first guarantee se mal feito |
| Multi-account para OAuth vendors via API key fallback (Anthropic, Codex, Gemini com N keys cada) | **v1.5** | OAuth flow é single-account por design; API key path para multi-account é refactor não-trivial |
| Team accounts / RLS / multi-user | **v2.0** | Requer backend, autenticação, billing — pivot de modelo |
| Billing / Stripe / paid tiers | **v2.0** | Torven v1.0 é OSS gratuito + portfolio piece; monetização exige produto-mercado validado |
| Windows / Linux native UI | **Talvez nunca** | TUI cross-platform existente já cobre Linux com Waybar widget; Windows native sem case claro |
| Mobile companion (iOS/iPadOS) | **v2.0+ ou nunca** | Sem case validado v1.0 |
| Vendor coverage além de 5 (Cursor, Copilot, Amp, Kimi, etc.) | **v1.5+ via plugin architecture** | v1.0 foca em depth (eval + multi-account) não breadth |
| Forecasting heurístico (à la Tokens 4 Breakfast) | **v1.5** | AI Insights cobre o caso de uso "what should I do?" — forecasting puro é diferente vetor |
| AI Insights local (Ollama / on-device LLM) | **v2.0** | Local LLM tem quality gap real para análise narrativa; reavaliar quando modelos pequenos forem suficientes |
| Plugin/extension marketplace | **v1.5+** | OpenUsage já oferece — gap de breadth para fechar pós-v1.0 |
| Subscription tracking (Claude Pro mensal, ChatGPT Plus, etc.) | **v1.5** | Tokens 4 Breakfast cobre — não é core do diferenciador |

---

## 8. Success Metrics

### Product Metrics (medidos via opt-in telemetria leve em v1.5 OU survey manual em v1.0)

- **Engagement:** usuário típico abre popover ≥3x/dia (mediana)
- **AI Insights adoption:** 30% dos usuários ativos disparam ≥1 insight/semana
- **AI Insights quality:** thumbs-up/thumbs-down opcional no UI; target ≥70% positivo
- **Retention 30d:** ≥40% dos installs ativos após 30 dias
- **Multi-account adoption:** entre usuários com OpenRouter ou Z.AI, ≥25% configuram ≥2 accounts

### Engineering Metrics (medidos automaticamente)

- **Eval pipeline score:** faithfulness ≥0.85, relevance ≥0.80 (v1.0 release blocker)
- **Bundle size:** <25MB (release blocker)
- **Cold start:** <800ms (release blocker)
- **Popover open p95:** <100ms (release blocker)
- **Crash-free sessions:** ≥99.5% (release blocker — medido se opt-in crash reporter ligado)

### Portfolio Metrics (Alex/Marina personas + hiring manager audience)

- GitHub stars: 500+ em 90 dias pós-lançamento (target ambicioso vs. baseline competitor)
- Show HN post: ≥150 upvotes (signal de novel angle resoante)
- Mentions em ≥3 hiring threads ou AI eng newsletters em 6 meses
- ≥1 entrevista AI Engineer onde Torven é citado como case de profundidade (qualitativo)

---

## 9. Risks & Mitigations

### R-1 — OpenUsage stack-identical convergence (HIGH)
- **Risco:** OpenUsage (Tauri 2 + React, idêntico) já é "vibe-coded" mas com tração; pode adicionar AI Insights antes do Torven shipar.
- **Mitigação:** Eval pipeline público + prompt versioning são moats sustentáveis (não dá pra vibe-code uma eval suite com 30+ casos rotulados em uma noite). Multi-account é vetor ortogonal que OpenUsage ainda não tem. Manter changelog honesto e attributions claras vs. "AI-generated codebase".

### R-2 — Anthropic API cost explosion (MEDIUM-HIGH)
- **Risco:** Usuário curioso clica "Insights" 50x/dia → $5+ no dia → churn imediato.
- **Mitigação:** Rate limit max 1 insight/minuto; cost budget $0.05/insight com warning; UI display de cost por insight ("This cost $0.03"); cumulative cost visível em Settings → AI Insights ("$1.43 spent this month").

### R-3 — macOS notarization friction (MEDIUM)
- **Risco:** Apple Developer cert lapses, notarytool falha, Gatekeeper bloqueia install no first-launch → churn.
- **Mitigação:** GitHub Actions automation com Apple Developer cert em secrets; smoke test no CI que baixa .dmg, monta e verifica `spctl -a` retorna OK; documentação clara "if you see 'damaged app' message, run xattr -dr com.apple.quarantine /Applications/Torven.app". Considerar Sparkle EdDSA signature além da Apple cert para defense-in-depth.

### R-4 — AI Insights perceived as gimmick (MEDIUM-HIGH)
- **Risco:** Reviewer técnico (Hacker News, Twitter) testa AI Insights, recebe insight banal, escreve "more AI snake oil" — destrói pitch.
- **Mitigação:** Eval pipeline PÚBLICO no repo é counterpunch direto (link visível no UI: "How we measure quality →"). Prompt versioning git-tracked mostra disciplina iterativa. README abre com screenshot do insight + métricas de eval, não com promessas. Tutorial blog post "How I built evals for a personal LLM tool" precede o launch.

### R-5 — Single-developer pace (MEDIUM)
- **Risco:** 1 dev (usuário) construindo isso enquanto trabalha + entrevistando — scope creep mata o release.
- **Mitigação:** Roadmap fases v1.0 → v1.5 → v2.0 explícito com out-of-scope list mantida disciplinadamente (§7). L0 vs L1 vs L2 priority em cada FR. Stories pequenas (≤1d), QA gates rigorosos, sem feature flags abertas.

### R-6 — macOS multi-account Keychain UX (MEDIUM)
- **Risco:** Keychain prompts repetitivos ao acessar N accounts → atrito.
- **Mitigação:** Single Keychain item por vendor com payload JSON `[{name, api_key, ...}]` em vez de N items separados. Usuário aprova acesso 1x por vendor por sessão. `[needs validation @architect]`.

### R-7 — Tauri webview performance perception (LOW-MEDIUM)
- **Risco:** Reviewers comparam com Swift nativo (ClaudeBar, Tokens 4 Breakfast) e percebem latência popover.
- **Mitigação:** Tauri 2 lazy-load + prefetch + pre-warm worker. Métricas concretas publicadas no README (popover p95 <100ms). Pequeno write-up "Why Tauri 2 here" em docs/ explicando o trade-off vs. Swift.

### R-8 — Anthropic API breaking change (LOW)
- **Risco:** Anthropic muda Messages API ou JSON Schema mode shape; insights quebram.
- **Mitigação:** Same playbook do `tests/live.rs` smoke pattern: contract test em CI que pinga Anthropic com um sample request semanal, alerta em drift. Prompts versionados ajudam: novo schema = bump major em prompts.v{N}.md.

---

## 10. Open Questions for @architect

Estas perguntas precisam virar ADRs antes de @sm criar stories de implementação:

1. **Workspace Cargo (mono vs multi)?** Manter `src/` flat (atual) ou split em `crates/torven-core`, `crates/torven-app`, `crates/torven-evals`? Implica refactor mas permite reaproveitar `torven-core` em testes/CLI sem puxar Tauri.

2. **IPC pattern Tauri → React?** Commands síncronos via `invoke()` ou events streaming via `emit()`? AI Insights é streaming-first — provavelmente events. Mas snapshots de uso (refresh tick) → commands. Confirmar.

3. **SQLite vs JSONL para history persistence?** SQLite dá queries por range/account/vendor mas adiciona dependência (rusqlite ou sqlx). JSONL é simples append-only mas pior pra agregação. PRD assumiu SQLite (FR-12) — confirmar ou desafiar.

4. **macOS Keychain integration: per-account ou per-vendor JSON blob?** R-6 acima propôs blob; confirmar viabilidade Tauri 2 + `security-framework` crate ou bridge Swift.

5. **Render lib para charts (FR-10)?** SVG hand-rolled? Recharts/Visx (React)? Canvas custom? Trade-off bundle size vs. dev velocity vs. UX polish.

6. **Auto-update mecanismo (FR-15)?** Sparkle (macOS-nativo, padrão de fato, EdDSA + appcast XML) OU Tauri updater (cross-platform-friendly, talvez menos batido em macOS)? PRD não decide.

7. **JSON Schema enforcement no AI Insights (FR-7 / §6.3)?** Anthropic `tools` API (tool-use forçado, mais robusto em Claude 3.5+) OU prompted JSON com validação client-side? PRD recomenda tool-use mas é decisão de impl.

8. **Eval pipeline runner (§6.1)?** Roda como `cargo test --features evals` (Rust-side) OU como script TS/Node lendo dataset JSONL e chamando LLM mockado? Implicações em CI duration e developer ergonomics.

---

## Appendix A — Inputs Consultados

- `docs/research/competitive-analysis.md` (@analyst, 2026-05-30) — pesquisa de 13+ competidores, gaps de AI Insights, defensible positioning
- `docs/research/naming-shortlist.md` (@analyst, 2026-05-30) — origem do nome "Torven"
- `CLAUDE.md` (raiz) — release checklist, hard invariants, secret discipline
- Codebase atual `src/{anthropic,openai,gemini,zai,openrouter}/` — fetchers Rust existentes, smoke tests, OAuth flows
- `src/tui/settings.rs` — pattern de TOML editing via UI (reaproveitar)

---

## Appendix B — Decisões marcadas como `[PM-DECISION]` (justificativas)

- **FR-8 prioridade vendor na menu bar em empate:** `Anthropic > OpenRouter > Codex > Gemini > Z.AI`. Racional: Anthropic é maior cohort dos personas (Marina e Alex ambos usam Claude pesado); OpenRouter é segundo porque é o multi-account vendor primário; Codex/Gemini/Z.AI mais nicho. Pode ser overridado pelo usuário em Settings → Display em v1.5.
- **NFR-2 cost ceiling $0.05/insight:** baseado em estimativa Claude 3.5 Sonnet ~$3/$15 per 1M tokens × 8K input + 1K output ≈ $0.039. Margin de 25% para variação modelo/preço futuro.
- **NFR-8 EN-only:** PT-BR docs internos mas produto é EN para audiência primária (hiring managers US). i18n PT-BR/ES considerar v1.5 se houver tração BR.
- **§6.1 dataset mínimo 30 casos:** abaixo de 30 estatísticas perdem signal; acima de 100 vira projeto-de-si-mesmo. 30 é o mínimo viável para metric stability.

---

**FIM do PRD v1.0 draft. Próximos passos:**
1. @architect resolve as 8 open questions e produz ADRs
2. @ux-design-expert produz UX spec (mockups popover, janela detalhada, account picker)
3. @po valida o PRD via `po-master-checklist`
4. @pm + @po dividem em epics (sugestão: E1 Vendors+Multi-Account, E2 AI Insights, E3 macOS Shell, E4 Settings+History, E5 Distribution)
5. @sm cria stories a partir das epics
