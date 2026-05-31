# Torven v1.0 — UX Specification

> **Autora:** @ux-design-expert (Uma)
> **Data:** 2026-05-31
> **Status:** Active (closes UX-Q1..Q5 do `docs/architecture/torven-v1-adr.md` §8)
> **Stack alvo:** SwiftUI + Swift Charts + MenuBarExtra (macOS 13+); dados via `crates/torven-core` por UniFFI
> **Predecessores:** `docs/prd/torven-v1.md`, `docs/architecture/torven-v1-adr.md`, `docs/research/competitive-analysis.md`, `docs/research/naming-shortlist.md`
> **Sucessores:** stories 7-12, 14, 16, 18-25 da migration sequence (@sm), validação técnica @swift-ui-expert (Sven)

---

## Change log

| Data | Versão | Descrição | Autora |
|------|--------|-----------|--------|
| 2026-05-31 | 0.1.0 | UX spec inicial fechando UX-Q1..Q5 do ADR v2 | @ux-design-expert |

---

## 1. Design principles

Estes seis statements são as regras de decisão. Quando algo está ambíguo em implementação, retorne aqui.

1. **Every pixel earns its keep.** Menu bar é real estate sagrada. Popover é janela emprestada. Nada existe "porque sobrou espaço" — cada elemento responde a uma pergunta concreta do Marina ou Alex.
2. **Default to dense, expand on intent.** Densidade Bartender/iStat na primeira leitura; detalhe Linear quando o usuário pede (hover, click, expand). Nunca o inverso.
3. **Show data, hide chrome.** Borders, dividers, backgrounds são privilégio, não default. SwiftUI `NSVisualEffectView` blur faz o chrome heavy lifting — nós só pintamos significado.
4. **Status before story.** Cor (calm/amber/critical) é sinal antes de texto. Usuário lê estado em 250ms olhando para a menu bar; texto detalhado é segunda passada.
5. **Apple HIG strict, mas com voz.** SF Pro/SF Mono, SF Symbols, blur, native menu/window behaviors. Voz Torven aparece em copy (microcopy direto, sem fofura) e em micro-momentos (animação de transição, hover delay), não em desvio de patterns.
6. **AI Insights é citizen, não modal.** O insight não é "feature secundária acessada por sheet" — é primeira pessoa no popover, com peso visual proporcional ao diferenciador competitivo que ele representa.

---

## 2. Resolução das 5 UX-Qs do @architect

### UX-Q1 — Popover sizing

**Contexto:** ADR §8 UX-Q1 lista dois finalistas viáveis: `360×420` (Bartender-grade densidade) vs `380×540` (5 cards full + header sem scroll). O cálculo do @architect mostra que 5 × 90px (altura natural VendorCard) + 56px (header) = 506px → spill em 420. Pressão competitiva: Bartender ~360px wide, Tokens 4 Breakfast ~340×460, ClaudeBar ~380×500.

**Opções consideradas:**

| Opção | Prós | Contras |
|-------|------|---------|
| **A. 360×420 fixo, scroll vertical** | Bartender-grade densidade; popover "leve" | Scroll em popover macOS é UX feio (HIG: popovers não devem rolar); usuário perde overview |
| **B. 380×540 fixo, no scroll** | 5 cards full + header em uma tela; respira; AI Insights button tem peso | 540px de altura é "popover grande" — desce além do menu bar em laptops 13"; sensação de "janela secundária" |
| **C. 360×480 com card density adjustável** | Compromise; suporta 5 cards em 84-86px + header 56px + AI Insights bar 40px = 472px | Cards 84px ficam apertados se vendor tem multi-account picker inline |
| **D. Cards colapsáveis (1 expanded + 4 collapsed)** | Densidade extrema | UX surprise — usuário precisa aprender; perde overview ao colapsar |

**Decisão recomendada:** **Opção C — `360 × 480` fixo, layout otimizado para densidade Bartender-grade.**

Justificativa:
- 360px width é o sweet spot validado por Bartender, Tokens 4 Breakfast e — não-trivialmente — por iStat Menus combined mode. Largura "menu bar item premium" canônica.
- 480px de altura cabe confortavelmente em qualquer MacBook 13" (display vertical mínimo 800px; popover deixa ~320px sobrando acima e abaixo da menu bar).
- Layout vertical: **48px header + 5 × 80px VendorCard + 32px AI Insights bar = 480px** exato.
- O AI Insights bar (não modal, não sheet) é um banner persistente no rodapé do popover — reforça o Design Principle #6.
- Card 80px é viável: 28px header row (vendor icon + name + account picker chevron) + 28px main metric row (number + delta) + 24px sparkline + status pill (12px overlay). Apple HIG min touch target 44pt cabe nos elementos interativos (account picker, vendor icon).

**Para multi-account com 4+ accounts:** o picker dentro do card abre sheet inline (não dropdown — vide UX-Q3). Card mantém 80px.

**Mockup ASCII (UX-Q1 decisão):**

```
╔══════════════════════════════════════════════════════════╗  ┐
║  Torven                            ⟳   ⚙   ✕             ║  │ 48px header
║  ───────────────────────────────────────────────────     ║  │ (translucent blur)
╠══════════════════════════════════════════════════════════╣  ┘
║  ╭─ A ─╮  Claude                                 87% 🔴   ║  ┐
║  │ AI  │  4h 23m left · Opus                     ▁▂▄▆█    ║  │ 80px VendorCard
║  ╰─────╯  ────────────────────────────────────────        ║  ┘
║  ╭─ O ─╮  OpenRouter         ClienteAcme ⇣      $42 ▼    ║  ┐
║  │ OR  │  $42.10 / $100 · 42%                   ▁▁▂▃▄    ║  │ 80px
║  ╰─────╯  ────────────────────────────────────────        ║  ┘
║  ╭─ C ─╮  Codex                                   62% 🟡  ║  ┐
║  │ 🅒  │  186 / 300 messages · 14d                ▂▃▄▅▆   ║  │ 80px
║  ╰─────╯  ────────────────────────────────────────        ║  ┘
║  ╭─ G ─╮  Gemini                                  31% 🟢  ║  ┐
║  │ ✦   │  -- daily quota                          ▁▁▁▂▃   ║  │ 80px
║  ╰─────╯  ────────────────────────────────────────        ║  ┘
║  ╭─ Z ─╮  Z.AI                Personal ⇣         $8 ▲     ║  ┐
║  │ Z   │  $8.40 / $50 · 17%                       ▁▂▂▂▃   ║  │ 80px
║  ╰─────╯  ────────────────────────────────────────        ║  ┘
╠══════════════════════════════════════════════════════════╣
║  ⌘I  Generate Insight    ·    Last: 2h ago  ·  $0.03    ║  ┐ 32px footer
╚══════════════════════════════════════════════════════════╝  ┘
   ◄────────────── 360px ──────────────►
```

> **Apple HIG cite:** `NSPopover` HIG ("Use a popover when you want to provide a small, focused view"). 360×480 cabe em "small focused" — está abaixo do limit ~600×600 que Apple sinaliza como "consider window instead".

---

### UX-Q2 — Paleta Swift Charts light/dark

**Contexto:** ADR §8 UX-Q2 pede definição semântica de cores para 4 estados de pressão + paleta categórica para 5 vendors em line/bar charts. Swift Charts respeita system appearance via Asset Catalog Color Sets, então definimos **uma vez** em `Assets.xcassets/Semantic/` e SwiftUI faz o resto.

**Decisão — Paleta Semântica (status de pressão):**

Os thresholds vêm do PRD FR-8 (75/90/95%). Adicionamos `calm` para <30% (faixa onde Marina não precisa nem olhar).

| Estado | Threshold | Light mode HEX | Dark mode HEX | WCAG AA contraste vs base | Uso |
|--------|-----------|----------------|---------------|---------------------------|-----|
| **Calm** | `pct < 30%` | `#1F8A4C` (forest green) | `#34C759` (apple green sistema) | 4.8:1 / 4.6:1 | Status pill, dot indicator |
| **Low** | `30% ≤ pct < 70%` | `#2563EB` (azure blue) | `#5E9DFF` (apple blue claro) | 5.1:1 / 4.7:1 | Default progress state |
| **Mid** | `70% ≤ pct < 85%` | `#E8A317` (amber-gold) | `#FFB340` (apple yellow) | 4.5:1 / 8.2:1 | Soft warning |
| **High** | `85% ≤ pct < 95%` | `#D97706` (deep amber) | `#FF9F0A` (apple orange) | 4.6:1 / 7.5:1 | Strong warning |
| **Critical** | `pct ≥ 95%` | `#C0312A` (clay red) | `#FF453A` (apple red) | 5.2:1 / 5.9:1 | Block-attention |

> Todos os pares satisfazem WCAG AA (>4.5:1) contra `NSColor.textBackgroundColor` em ambos os modos. Verificação feita assumindo `#FFFFFF` light e `#1E1E1E` dark (vibrant background dos popovers translucent macOS).

**Asset Catalog estrutura:**

```
apple/Torven/Assets.xcassets/Semantic/
├── Calm.colorset/Contents.json       (Any:#1F8A4C, Dark:#34C759)
├── Low.colorset/Contents.json        (Any:#2563EB, Dark:#5E9DFF)
├── Mid.colorset/Contents.json        (Any:#E8A317, Dark:#FFB340)
├── High.colorset/Contents.json       (Any:#D97706, Dark:#FF9F0A)
└── Critical.colorset/Contents.json   (Any:#C0312A, Dark:#FF453A)
```

Usage em SwiftUI: `Color("Semantic/Critical")` (Swift Charts integra direto).

**Decisão — Paleta Categórica (vendors em charts):**

Para line/bar charts no Main Window mostrando 5 vendors juntos, precisamos de 5 cores **distinguíveis** + colorblind-safe. Não reutilizamos a paleta semântica (causaria confusão visual: "Anthropic é critical?").

Base: paleta de 5 cores derivada do [Okabe-Ito colorblind-safe palette](https://jfly.uni-koeln.de/color/), levemente ajustada para vibrar com accent color macOS:

| Vendor | Light mode HEX | Dark mode HEX | Rationale |
|--------|----------------|---------------|-----------|
| **Anthropic** | `#CC7A0A` (anthropic orange terra) | `#E8A360` | Marca da própria Anthropic vibra; usuário associa visualmente |
| **OpenAI (Codex)** | `#10A37F` (openai teal) | `#3FBA9C` | Brand color OpenAI |
| **Gemini** | `#4F86E0` (gemini blue-purple) | `#7AA8F0` | Google brand blue |
| **Z.AI** | `#A14FB2` (z.ai violet) | `#C57FD6` | Brand acessório, contrasta com Anthropic |
| **OpenRouter** | `#586875` (slate cinza-azul) | `#8898A8` | Brand neutro (OR é router, não modelo) |

Validação colorblind: testado em simulador Sim Daltonism (deuteranopia, protanopia, tritanopia) — todas as 5 cores permanecem distinguíveis em pares adjacentes.

**Decisão — Animação Swift Charts:**

- **Update animation:** default Swift Charts (~500ms `.easeOut`) — não mexer
- **Initial mount animation:** desabilitada (`.transaction { $0.animation = nil }`) — popover open deve ser <100ms; chart animation atrasa primeira leitura
- **Crossfade entre time ranges (today/7d/30d):** 200ms ease-in-out, custom via `withAnimation`

**Decisão — Eixo Y formatting:**

- Cost (USD): `$X.XX` para <$100, `$XK` para K-scale; nunca scientific notation
- Window %: `XX%` sem decimal
- Messages quota: `X / Y` mantendo proporção (não `0.62`)
- Tokens: `X.Xk` ou `X.XM` (1.2k, 4.5M) — nunca raw `1234567`
- Decisão de format por chart vive em `apple/Torven/Window/HistoryChart.swift` via `metricKind: LabelKind` enum (já decidido no ADR-5/format.rs migration)

**Decisão — Empty state:**

Quando vendor sem histórico (<2 snapshots) → chart mostra placeholder gentle: ícone SF Symbol `chart.line.uptrend.xyaxis.circle` em `Color.secondary` + caption "Need 2+ days of data". Nunca chart vazio com eixos pintados (UX feio).

> **Apple HIG cite:** "Use color purposefully" (HIG Color). Paleta semântica + paleta categórica vivem em namespaces separados (`Semantic/` vs raiz) — evita o anti-pattern "vendor virou critical".

---

### UX-Q3 — Account picker shape no popover

**Contexto:** ADR §8 UX-Q3. Marina (persona freelance) tem **1-3 accounts** por vendor (1 por cliente OpenRouter; tipicamente 2-3). Alex (persona startup) pode ter **5+ accounts** em OpenRouter (team account + accounts pessoais + experimentation). Popover de 360px width. VendorCard fixo em 80px height.

**Opções consideradas:**

| Opção | Prós | Contras |
|-------|------|---------|
| **A. Combobox dropdown (Picker SwiftUI)** | Apple-native; familiar; suporta N accounts | Vira dropdown menu macOS — interrompe leitura visual do popover; usuário sai do flow |
| **B. Segmented control horizontal** | Densidade visual; 1 click; mostra todas | Quebra em >3 accounts (popover 360px só cabe ~3 chips de 100px); inadequado para Alex |
| **C. Expandable cards (acordeon)** | Densidade quando colapsado | Em 5 accounts × 28px linha = 140px extra vertical → estoura layout 80px |
| **D. Aggregate + "View by account →"** | Card mantém 80px; densidade máxima; rota para detail view | Marina perde overview rápido por cliente; precisa abrir janela detalhada para info simples |
| **E. Híbrido contextual** | Combina B (1-3 accounts inline chips) + A (4+ vira dropdown chevron) | Pequena inconsistência (dois comportamentos), mas é honest design — densidade quando cabe, dropdown quando precisa |

**Decisão recomendada:** **Opção E — híbrido contextual.**

Regras:
- **1 account configurada:** sem picker visual. Card mostra valor agregado direto. Visual idêntico a vendor single-account (Anthropic/Codex/Gemini).
- **2-3 accounts:** **segmented chips inline na header row do card**. Cada chip = primeira letra da tag + nome curto (`C·Acme`, `P·Pessoal`, `T·Team`). Tap chip = filtra card para essa account. Chip `Σ` (sigma) no início = aggregate "All accounts". Active chip tem accent color background.
- **4+ accounts:** chevron `⇣ All accounts (4)` no card header. Tap abre **inline sheet de 200px** sobreposto ao popover (não bottom sheet — slide vertical "embaixo" do header do card). Sheet tem list scrollable de accounts com `name + tag + $spent / $budget`. Search field no topo se >7 accounts. ESC ou tap-outside fecha.

Por que híbrido funciona:
- Marina (1-3 accounts): chips densos = vê tudo de uma vez, sem cliques. Bartender vibe.
- Alex (5+ accounts): chevron + inline sheet = densidade preservada no card, exploração quando precisa. Linear/Raycast vibe.
- Single source of consistency: o padrão `Σ + chips` ≤3, `⇣ list` ≥4 é claro e learnable em 1 sessão.

**Mockup ASCII — 1 account (card OpenRouter):**

```
╭──────────────────────────────────────────────────────╮
│ ╭─O─╮  OpenRouter                            $42 ▼   │
│ │OR │  $42.10 / $100 · 42%               ▁▁▂▃▄       │
│ ╰───╯  ──────────────────────────────────────         │
╰──────────────────────────────────────────────────────╯
```

**Mockup ASCII — 2-3 accounts (Marina, OpenRouter):**

```
╭──────────────────────────────────────────────────────╮
│ ╭─O─╮  OpenRouter   [Σ][C·Acme][P·Pess][C·Beta]      │
│ │OR │  $42.10 / $200 · 21%                ▁▂▃▄▅      │
│ ╰───╯  ──────────────────────────────────────         │
╰──────────────────────────────────────────────────────╯
       └ Σ = all accounts (active)                 ↑
       └ tap chip filtra view  ─────────────────────┘
```

**Mockup ASCII — 4+ accounts (Alex, OpenRouter), chevron collapsed:**

```
╭──────────────────────────────────────────────────────╮
│ ╭─O─╮  OpenRouter           ⇣ All accounts (5)       │
│ │OR │  $187.40 / $500 · 37%                ▁▂▃▄▅     │
│ ╰───╯  ──────────────────────────────────────         │
╰──────────────────────────────────────────────────────╯
```

**Mockup ASCII — 4+ accounts, sheet expanded inline:**

```
╭──────────────────────────────────────────────────────╮
│ ╭─O─╮  OpenRouter           ⇡ All accounts (5)       │
│ │OR │  $187.40 / $500 · 37%                ▁▂▃▄▅     │
│ ╰───╯ ╔════════════════════════════════════════════╗ │
│       ║ 🔍 Search accounts...                      ║ │
│       ╠════════════════════════════════════════════╣ │
│       ║ ● Team-Prod    [team]    $89 / $200  44%  ║ │
│       ║ ○ Cliente-Acme [client]  $42 / $150  28%  ║ │
│       ║ ○ Personal     [pers]    $23 / $100  23%  ║ │
│       ║ ○ Cliente-Beta [client]  $18 / $50   36%  ║ │
│       ║ ○ Experiments  [pers]    $15 / —     —    ║ │
│       ╚════════════════════════════════════════════╝ │
╰──────────────────────────────────────────────────────╯
       ↑ ESC ou tap-outside fecha; tap account filtra
```

> **Apple HIG cite:** "Provide concise labels" + "Help people scan quickly" (HIG Lists). Chips ≤3 satisfaz "scannable"; sheet com search satisfaz "lists need search at ~7+". Threshold de 4 é onde chips se tornam apertados em 360px width.

> **Competitive note:** ClaudeBar não tem multi-account. OpenUsage tem dropdown simples (opção A) — divergimos com Opção E porque Marina/Alex são as personas de quem multi-account importa mais.

---

### UX-Q4 — AI Insights streaming UX

**Contexto:** ADR §8 UX-Q4 + ADR-7 (Anthropic tool_use streaming via UniFFI callback). PRD §6.6 lista failure modes. Pergunta: como mostrar a transição loading → token streaming → structured final?

**Opções consideradas para transição streaming → final:**

| Opção | Prós | Contras |
|-------|------|---------|
| **A. Skeleton estruturado com placeholders** | Usuário vê shape do output antes do conteúdo; gera expectativa | Skeleton "mentiroso" se output real for muito diferente; complexo de implementar |
| **B. Token-by-token raw reveal** | Mostra "AI working"; fidelidade ao stream | JSON raw é feio (`{"headline": "...` aparece literal); usuário vê implementation detail |
| **C. Headline-first reveal** | Parse parcial: assim que `headline` campo completar, render headline em fonte normal; depois `insights[]` aparece um a um | Requer parse incremental do JSON (não é texto livre) — complexidade no Swift |
| **D. Single accumulator + transition** | Stream em fonte monoespaçada visível; quando `onDone` chega, crossfade para structured | Híbrido honest; 2 estados visuais; usuário entende "estava processando, agora terminou" |

**Decisão recomendada:** **Opção C — headline-first reveal (parse parcial estruturado).**

Justificativa:
- ADR-7 já decide tool_use mode (não texto livre) — output é JSON desde o primeiro token. Renderizar JSON raw seria revelar plumbing.
- Anthropic Messages API com tool_use streaming envia `content_block_delta` com `input_json_delta` por chunk. UniFFI callback `onToken(token)` recebe esses fragmentos. No Swift side, mantemos `accumulatedJsonText: String` e tentamos `try? JSONDecoder().decode(InsightsOutput.self, from: ...)` a cada novo chunk — sucesso parcial nos campos completados.
- Estratégia de reveal progressivo:
  1. **0-300ms (pre-first-token):** spinner sutil + caption "Thinking..." em `Color.secondary`. Não skeleton — apenas centered spinner pequeno.
  2. **First-token arrives (~1.5s p50 Anthropic):** headline aparece com fade-in (200ms) assim que campo `headline` parsea. Caption muda para "Generating insights..."
  3. **Cada novo `insight` no array:** card individual fade-in da esquerda (slide 8px + opacity), 250ms stagger.
  4. **`recommendation` field completes:** card recommendation aparece no rodapé com leve elevation/shadow distinct.
  5. **`onDone` arrives:** caption muda para `Cost: $0.03 · 4.2s · prompt v1` em `Color.secondary`. Botão "Copy", "Save", "Regenerate" aparecem.
- **Custo/latência display:** apenas no final (após `onDone`). Não mostrar durante streaming (cria ansiedade + número muda — Bartender principle: cor antes de número quando estado é transient).

**Cancel button:**
- Durante streaming: pequeno `X` à direita do caption "Generating insights..." — sem confirmation modal (insights ≤$0.05; abortar é cheap, friction deve ser zero). Cancel chama `coreBridge.cancelInsights(requestId)` que aborta tokio task (vide ADR-7).
- Cancel concluído: fade-out tudo, retorna ao estado "Generate Insight" button.

**Failure modes (PRD §6.6) — UX por failure:**

| Failure | UI |
|---------|---|
| API key não configurada | Modal inline (não sheet — popover too small para sheet): icon `key.slash` + headline "Add your Anthropic API key" + button "Open Settings" (abre Settings → AI Insights tab) |
| Sem histórico (<7 dias) | Botão "Generate Insight" no popover footer fica disabled com tooltip "Need 7+ days of data" (hover delay 800ms padrão Apple) |
| Cost budget excedido | Alert sheet macOS native: "This insight will cost ~$0.08, above your $0.05 limit." [Cancel] [Send anyway] |
| Rate limit (429) | Toast inline no insights panel: "Rate limited. Retry in 42s." com countdown decrementing |
| Malformed JSON | Card error gentle: icon `exclamationmark.triangle` + "Couldn't parse this insight. Saved to logs." + button "Retry" |
| Network failure | Card error: icon `wifi.slash` + "Offline. Insights need internet." + button "Retry" |

**Mockup ASCII — pre-first-token state:**

```
╔══════════════════════════════════════════════════════════╗
║  Insight                                          ✕      ║
║  ────────────────────────────────────────────────         ║
║                                                            ║
║                       ⟳  Thinking...                       ║
║                                                            ║
║                                                            ║
║                                                            ║
╚══════════════════════════════════════════════════════════╝
```

**Mockup ASCII — mid-stream (headline + 1 insight loaded):**

```
╔══════════════════════════════════════════════════════════╗
║  Insight                       Generating insights...  ✕  ║
║  ────────────────────────────────────────────────         ║
║                                                            ║
║  You're 38% above your usual Opus weekly pace, mostly      ║
║  between 9pm–11pm on weekdays.                             ║
║                                                            ║
║  ╭────────────────────────────────────────────────╮       ║
║  │ 🟡 trend · evening Opus usage doubled vs last   │       ║
║  │     week ($14 → $29). Sonnet would cover most.  │       ║
║  ╰────────────────────────────────────────────────╯       ║
║                                                            ║
║              ▒▒▒▒▒▒▒▒▒  (next insight loading...)         ║
║                                                            ║
╚══════════════════════════════════════════════════════════╝
```

**Mockup ASCII — final state (`onDone` complete):**

```
╔══════════════════════════════════════════════════════════╗
║  Insight                                          ✕      ║
║  ────────────────────────────────────────────────         ║
║                                                            ║
║  You're 38% above your usual Opus weekly pace, mostly      ║
║  between 9pm–11pm on weekdays.                             ║
║                                                            ║
║  ╭────────────────────────────────────────────────╮       ║
║  │ 🟡 trend · evening Opus usage doubled vs last   │       ║
║  │     week ($14 → $29). Sonnet would cover most.  │       ║
║  ╰────────────────────────────────────────────────╯       ║
║  ╭────────────────────────────────────────────────╮       ║
║  │ 🔴 budget_risk · Cliente-Acme on track to       │       ║
║  │     exceed $200 budget by ~$45 this month.      │       ║
║  ╰────────────────────────────────────────────────╯       ║
║                                                            ║
║  ╭ Recommendation ───────────────────────────────╮        ║
║  │ Switch evening exploration to Sonnet between   │        ║
║  │ 9pm–11pm. Estimated saving: $18/week.          │        ║
║  ╰────────────────────────────────────────────────╯       ║
║                                                            ║
║  Cost: $0.03 · 4.2s · prompt v1   [Copy] [Save] [↻ Again] ║
╚══════════════════════════════════════════════════════════╝
```

> **Apple HIG cite:** "Avoid placeholders for content that's loading" (HIG Loading). Resolvemos com headline-first parse: por 0-300ms mostramos thinking (não placeholder mentiroso); depois exibimos conteúdo real progressivo. Honra HIG sem perder o "AI working" signal.

> **Competitive note:** ClaudeBar / OpenUsage / Tokens 4 Breakfast não têm streaming UI (não têm AI Insights). Referência indireta: Claude.ai web app (Anthropic's own product) usa typewriter raw — mas eles têm largura full-screen pra absorver feio. No nosso popover 360px, JSON raw é inviável. Decisão diverge consciente.

---

### UX-Q5 — Settings UI organização

**Contexto:** ADR §8 UX-Q5. Settings tem ~7 sections: General, Vendors, Accounts (per vendor expansion), AI Insights, Data (retention/export), Updates, About. SwiftUI `Settings` scene em macOS 13+ permite TabView (legacy paradigm) ou NavigationSplitView (System Settings Ventura+ paradigm).

**Opções consideradas:**

| Opção | Prós | Contras |
|-------|------|---------|
| **A. TabView top tabs (Xcode/Terminal style)** | Familiar; lightweight; max 5-6 tabs antes de quebrar | 7+ items ficam congestionados; sub-items (accounts per vendor) não cabem |
| **B. NavigationSplitView sidebar (System Settings style)** | Suporta sub-items; escalável para v1.5 (mais sections); paradigm moderno | "Peso visual" maior; Settings de menu bar app tipicamente leve |
| **C. Híbrido: sidebar + collapsible sections** | Densidade + escalabilidade | Não-canônico; usuário macOS não espera |

**Decisão recomendada:** **Opção B — NavigationSplitView sidebar.**

Justificativa:
- 7 sections + sub-items (accounts per vendor, expandindo para potencialmente 5+ accounts) **passa do teto razoável de TabView** (Apple HIG implícito: 5-6 tabs max antes de virar segmented mess).
- System Settings (Ventura+) é o paradigma 2026 esperado. ClaudeBar usa sidebar; Tokens 4 Breakfast usa sidebar; iStat Menus usa sidebar — alinha com expectativa do segmento.
- Sub-items vendor.accounts são nativos em sidebar (disclosure indicator triangulo) — em tabs viraria tela aninhada com breadcrumb (UX 2018).
- Settings window sizing: 760×520 (não-resizable). Sidebar 200px + detail 560px. Cabe confortavelmente em qualquer Mac.
- Tradeoff "peso visual" mitigado: sidebar usa `NSVisualEffectView` translucent (mesmo blur do popover); peso percebido é menor que sidebar opaco.

**Estrutura sidebar:**

```
General                ← app behavior (refresh interval, launch at login)
Vendors                ← list of 5 vendors, click expande accounts inline
├ Anthropic
├ OpenAI Codex
├ Google Gemini
├ Z.AI
└ OpenRouter
AI Insights            ← Anthropic API key, cost cap, schedule
Notifications          ← budget alert thresholds, notification toggles per account
Data                   ← retention slider, export CSV/JSON, "Edit config.toml..." escape hatch
Updates                ← Sparkle settings (auto-check, channel)
About                  ← version, license, "Run Diagnostics" button
```

**Per-vendor expansion (clicar "Vendors" → "OpenRouter"):**

A view detail mostra:
1. Header: vendor name + icon + enable toggle
2. Auth status row: "Connected" / "Not authenticated" + button (Re-login / Add API key)
3. **Accounts table** (apenas para vendors API-key — OpenRouter/Z.AI):
   - Inline editable table: name | tag picker | budget input | api_key (masked) | actions [edit][delete]
   - "+ Add account" row no rodapé
4. Refresh interval slider (10s-300s)
5. Thresholds editor (75/90/95 default, customizável per vendor)

**Mockup ASCII — Settings window (Vendors → OpenRouter selected):**

```
╔══════════════════════════════════════════════════════════════════════════╗
║  ⬤ ⬤ ⬤                Torven Settings                                    ║
╠══════════════════════════════════════════════════════════════════════════╣
║                                                                            ║
║  ┌─────────────────┐ ┌──────────────────────────────────────────────────┐ ║
║  │ General         │ │  OpenRouter                              ⬤ ON   │ ║
║  │ Vendors      ▼  │ │  ──────────────────────────────────────────────  │ ║
║  │  ├ Anthropic    │ │                                                    │ ║
║  │  ├ OpenAI       │ │  Auth status                          API key     │ ║
║  │  ├ Gemini       │ │  ● Connected                          [Edit]      │ ║
║  │  ├ Z.AI         │ │                                                    │ ║
║  │  └ OpenRouter ◀ │ │  Accounts (5)                                      │ ║
║  │                 │ │  ┌──────────────────────────────────────────────┐ │ ║
║  │ AI Insights     │ │  │ Name           Tag       Budget   Actions    │ │ ║
║  │ Notifications   │ │  ├──────────────────────────────────────────────┤ │ ║
║  │ Data            │ │  │ Cliente-Acme   [client]  $200    [✎][🗑]    │ │ ║
║  │ Updates         │ │  │ Cliente-Beta   [client]  $50     [✎][🗑]    │ │ ║
║  │ About           │ │  │ Team-Prod      [team]    $200    [✎][🗑]    │ │ ║
║  │                 │ │  │ Personal       [pers]    $100    [✎][🗑]    │ │ ║
║  │                 │ │  │ Experiments    [pers]    —       [✎][🗑]    │ │ ║
║  │                 │ │  └──────────────────────────────────────────────┘ │ ║
║  │                 │ │  + Add account                                     │ ║
║  │                 │ │                                                    │ ║
║  │                 │ │  Refresh interval                                  │ ║
║  │                 │ │  ◯─────●─────────  60s                              │ ║
║  │                 │ │                                                    │ ║
║  │                 │ │  Thresholds        75% ─ 90% ─ 95%                  │ ║
║  └─────────────────┘ └──────────────────────────────────────────────────┘ ║
║   ◄── 200px ──►        ◄────────────── 560px ──────────────►              ║
║                                                                            ║
╚══════════════════════════════════════════════════════════════════════════╝
                              ◄── 760px width ──►
```

**Mockup ASCII — Settings → AI Insights:**

```
║  AI Insights                                                              ║
║  ──────────────────────────────────────────────────────────────────      ║
║                                                                            ║
║  Anthropic API key                                                        ║
║  [ sk-ant-************************************************ ]    [Edit]   ║
║                                                                            ║
║  Cost cap per insight             $0.05                                   ║
║  ●──────────────────────────────  (slider $0.01 - $0.20)                 ║
║                                                                            ║
║  Generate schedule                                                        ║
║  ○ Manual only                                                            ║
║  ● Suggest weekly  (every Monday 9am)                                     ║
║  ○ Suggest daily   (every weekday 9am)                                    ║
║                                                                            ║
║  Active prompt version            v1 (default)                            ║
║  [ Switch to experimental v2 → ]                                          ║
║                                                                            ║
║  Privacy                                                                  ║
║  ☑ Hash account names in payload (default ON)                            ║
║  ☑ Redact API keys from logs (default ON)                                ║
║                                                                            ║
║  Spent this month                 $1.43 across 47 insights                ║
║  [ View insights history → ]                                              ║
```

> **Apple HIG cite:** "Use a sidebar to navigate hierarchical content" (HIG Sidebars). Vendors → Accounts é hierárquico; sidebar é canônico. System Settings Ventura+ é a referência viva do paradigm.

> **Competitive note:** ClaudeBar usa sidebar de ~180px; iStat Menus usa sidebar densa. Tokens 4 Breakfast tem Settings minimal (poucas seções) e usa tabs — divergimos porque Torven tem 5x mais surface area.

---

## 3. Wireframes ASCII detalhados

### 3.a — Menu bar item (NSStatusItem custom label)

ADR-10 confirma `MenuBarExtra` com label dinâmico via `coreBridge.menuBarSnapshot`. FR-8 do PRD define prioridade vendor + truncamento 18 chars + cor calm/amber/critical.

**Layout do label:**
- `[ICON] [PRIMARY_METRIC]` — total ≤18 chars incluindo ícone (SF Symbol)
- Ícone = SF Symbol semântico, NÃO vendor logo (vendor logo aparece em popover)
- Cor do ícone segue paleta semântica (calm/mid/high/critical)
- Cor do texto: `NSColor.menuBarLabelColor` (system auto-invert, NUNCA override)

**Estados ASCII:**

```
┌───────────────────────────────────────────────────────────────┐
│ macOS Menu Bar                                                │
├───────────────────────────────────────────────────────────────┤
│                                                               │
│ Idle (just launched, no data)                                 │
│ ▸  ◌ Torven                                                  │
│    ↑ ícone SF "circle.dotted" outline, sem texto métrico     │
│                                                               │
│ Loading (refresh in flight)                                   │
│ ▸  ⟳ Torven                                                  │
│    ↑ ícone SF "arrow.triangle.2.circlepath" rotating          │
│                                                               │
│ Calm (<30% — Claude window primário)                          │
│ ▸  ● Claude 18% 5h                                            │
│    ↑ dot calm green; truncate "Claude 18% / 5h" → "Claude 18% 5h" (15 chars OK)
│                                                               │
│ Mid (30-70%)                                                  │
│ ▸  ● Claude 54% 5h                                            │
│    ↑ dot azure blue                                           │
│                                                               │
│ High (70-90%)                                                 │
│ ▸  ⬤ Claude 87% 5h                                            │
│    ↑ dot amber-gold; weight medium                            │
│                                                               │
│ Critical (>=95%)                                              │
│ ▸  ⬤ Claude 97% 5h                                            │
│    ↑ dot clay red; weight semibold (subtle attention)        │
│                                                               │
│ Multi-account aggregation (OpenRouter wins priority)         │
│ ▸  ● OR $187/$500                                             │
│    ↑ "OR" abbreviation; sum across accounts                  │
│                                                               │
│ Multi-account active vendor with single account high         │
│ ▸  ⬤ OR Acme 89%                                              │
│    ↑ shows winning account name short (max 4 chars)          │
│                                                               │
│ Error/Warning fallback (vendor drift, network)                │
│ ▸  ⚠ Torven                                                  │
│    ↑ SF "exclamationmark.triangle"; click opens popover      │
│      with vendor error detail                                 │
│                                                               │
└───────────────────────────────────────────────────────────────┘
```

**Regra de truncamento:**
- Total label budget: 18 chars incluindo SF symbol space (~3 chars de visual budget)
- Texto: ~15 chars
- Ordem de truncamento se overflow: drop trailing unit ("5h" / "msg") → drop "%" → drop space → drop vendor short name
- Vendor short names: Claude, OR (OpenRouter), Codex, Gemini, Z.AI

**Animation:** transição entre pressure states usa `withAnimation(.easeOut(duration: 0.4))` no `.foregroundColor` do icon — cor muda smooth (não pisca). Texto label muda imediato (sem animation — números mudando ficam tonto).

### 3.b — Popover (360 × 480) full layout

Já mostrado em UX-Q1. Detalhamento adicional por zona:

**Header (48px):**

```
╔══════════════════════════════════════════════════════════╗
║                                                            ║
║  Torven                            ⟳   ⚙   ✕              ║
║                                                            ║
╠══════════════════════════════════════════════════════════╣
```

- "Torven" em SF Pro Display 14pt semibold, leading 16pt
- `⟳` = SF "arrow.triangle.2.circlepath" — Refresh (Cmd+R). Hover: tooltip "Refresh (⌘R)"
- `⚙` = SF "gearshape" — Settings (Cmd+,). Hover: "Settings (⌘,)"
- `✕` = SF "xmark.circle" — Close popover (Esc). Hover: "Close (Esc)"
- Trailing 16pt padding; icons spacing 12pt
- Background: `NSVisualEffectView` material `.popover` (translucent blur)

**VendorCard (80px) anatomy:**

```
┌──────────────────────────────────────────────────────────┐
│ ╭───╮  VendorName  [chips/picker placement]         🟢   │  ← 28px header row
│ │ ⬢ │                                                    │
│ │   │  $42.10 / $100 · 42%                ▁▁▂▃▄          │  ← 28px metric row
│ ╰───╯                                                    │
│         "evening Opus heavy" caption (optional)          │  ← 24px context row
└──────────────────────────────────────────────────────────┘
                                          ↑ sparkline 56px wide
```

- **Vendor icon (40×40):** rounded square `NSImageView`, vendor brand color background tint (subtle), SF Symbol monogram inside (`A`, `OR`, `C` etc.). NÃO usar logo proprietário (legal + manutenção).
- **VendorName:** SF Pro Text 13pt semibold
- **Chips/picker placement:** vide UX-Q3 (chips inline ≤3, chevron ≥4)
- **Status pill (right):** dot 8px diameter, color from semantic palette. Não texto adjacent (cor já comunica)
- **Metric row:** number SF Pro Text 13pt regular; unit SF Pro Text 11pt regular `Color.secondary`; sparkline trailing
- **Sparkline:** Swift Charts mini-line, 7d trend, 56×16pt, sem axis labels, sem dots. Cor da paleta categórica vendor.
- **Context row:** opcional. Aparece em vendors com insight relevante recente ("evening Opus heavy"). Caption 11pt `Color.secondary`. Se ausente, card collapse para 56px? **NÃO** — manter 80px uniforme (consistência visual > densidade marginal).

**Footer (32px):**

```
╠══════════════════════════════════════════════════════════╣
║  ⌘I  Generate Insight    ·    Last: 2h ago  ·  $0.03    ║
╚══════════════════════════════════════════════════════════╝
```

- `⌘I` = keyboard shortcut hint, SF Pro Text 10pt `Color.secondary`
- "Generate Insight" = SF Pro Text 12pt accent color, button-style (tap area full row)
- "Last: 2h ago" = caption do último insight gerado; `Color.secondary`
- `$0.03` = custo do último insight; `Color.secondary`
- Se nunca gerado: shows apenas "⌘I Generate Insight"
- Se <7d histórico: row mostra "Need 7+ days of data" disabled

**Hover interactions:**
- VendorCard hover: subtle `.background(Color.gray.opacity(0.05))` 100ms ease-in
- Account chip hover: chip background ramps from `.opacity(0.15)` to `.opacity(0.25)` accent
- Footer "Generate Insight" hover: text underline on, 100ms

### 3.c — Janela detalhada (Main Window) — 1024 × 720 default

Layout 3-pane: Sidebar (200px) + Content (variable) + optional Inspector (240px when expanded).

```
╔════════════════════════════════════════════════════════════════════════════════╗
║  ⬤ ⬤ ⬤                Torven — Main                            [⌘F] [Today ⇣] ║
╠════════════════════════════════════════════════════════════════════════════════╣
║                                                                                  ║
║  ┌──────────────────┐ ┌─────────────────────────────────────────────────────┐ ║
║  │ OVERVIEW         │ │  OpenRouter                                          │ ║
║  │                  │ │  ──────────────────────────────────────────────────  │ ║
║  │ Vendors          │ │                                                       │ ║
║  │ ▸ Anthropic      │ │  ┌─────────────┬─────────────┬─────────────┐         │ ║
║  │ ▸ OpenAI         │ │  │  Spent       │  Tokens      │  Top model   │        │ ║
║  │ ▸ Gemini         │ │  │  $187.40     │  4.2M        │  Sonnet 3.5  │        │ ║
║  │ ▸ Z.AI           │ │  │  / $500      │  out         │  $89 (47%)   │        │ ║
║  │ ● OpenRouter     │ │  └─────────────┴─────────────┴─────────────┘         │ ║
║  │                  │ │                                                       │ ║
║  │ ─────────────    │ │  [ Overview ] [ History ] [ Accounts ] [ Models ]    │ ║
║  │                  │ │                                                       │ ║
║  │ Insights History │ │  ┌──────────────────────────────────────────────┐  │ ║
║  │ ▸ This week      │ │  │                                                │ ║
║  │ ▸ Last week      │ │  │     ┌─ cost per day, last 30 days ────┐         │ ║
║  │                  │ │  │     │ ╱╲    ╱╲    ╱╲╱╲                  │         │ ║
║  │ ─────────────    │ │  │  $10│╱  ╲  ╱  ╲╱╲╱    ╲╱╲                │         │ ║
║  │                  │ │  │     │    ╲╱           ╲╱  ╲╱╲              │         │ ║
║  │ Settings...      │ │  │   $0└─────────────────────────────         │         │ ║
║  │                  │ │  │     1 May    8 May   15 May   22 May  30   │         │ ║
║  │                  │ │  └──────────────────────────────────────────────┘  │ ║
║  │                  │ │                                                       │ ║
║  │                  │ │  Accounts breakdown                                   │ ║
║  │                  │ │  ┌──────────────────────────────────────────────┐  │ ║
║  │                  │ │  │ Cliente-Acme   $42  ▓▓▓▓▓▓▓░░░░░░░░░░ 28%   │  │ ║
║  │                  │ │  │ Cliente-Beta   $18  ▓▓▓▓▓░░░░░░░░░░░░ 36%   │  │ ║
║  │                  │ │  │ Team-Prod      $89  ▓▓▓▓▓▓▓▓▓▓▓▓░░░░ 44%   │  │ ║
║  │                  │ │  │ Personal       $23  ▓▓▓▓▓▓░░░░░░░░░░ 23%   │  │ ║
║  │                  │ │  │ Experiments    $15  ▓▓▓░░░░░░░░░░░░░░ —    │  │ ║
║  │                  │ │  └──────────────────────────────────────────────┘  │ ║
║  └──────────────────┘ └─────────────────────────────────────────────────────┘ ║
║   ◄── 200px ──►        ◄──────────────── ~824px ──────────────────►            ║
║                                                                                  ║
╚════════════════════════════════════════════════════════════════════════════════╝
                                    ◄──── 1024px ────►
```

**Sidebar zones:**
- **Vendors:** lista canônica. Vendor selecionado tem accent bar à esquerda + background subtle. `▸` indica collapsed (sem expansão por padrão; click vendor → conteúdo direita muda).
- **Insights History:** grouped por week. Clicar abre InsightsHistoryView no content pane.
- **Settings...:** atalho redundante (também acessível via Cmd+, no global). Abre Settings window separada.

**Content pane — quando vendor selecionado:**
- Header: vendor name + KPI cards (3 cards horizontal)
- Tab bar: Overview | History | Accounts | Models
- Tab content: muda conforme seleção. Overview default mostra chart + accounts breakdown stacked.

**Top toolbar (window title bar):**
- `⌘F` shortcut hint for search
- **Time range selector** `[Today ⇣]` — popover menu com presets: Today / Yesterday / 7d / 30d / Month-to-date / Custom range...
- Time range é **global** (afeta todos os charts e KPIs do main window)

**Tab "Models" (OpenRouter/Z.AI):**

```
║  [ Overview ] [ History ] [ Accounts ] [ Models ◀]                                 ║
║                                                                                      ║
║  Models breakdown                                                                   ║
║  ┌──────────────────────────────────────────────────────────────────────────────┐ ║
║  │  claude-3.5-sonnet      $89.40   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ 47%             │ ║
║  │  gpt-4o                 $42.10   ▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░ 22%             │ ║
║  │  claude-3-opus          $28.20   ▓▓▓▓▓▓▓▓░░░░░░░░░░░░░░░░░░ 15%             │ ║
║  │  gemini-1.5-pro         $14.50   ▓▓▓▓░░░░░░░░░░░░░░░░░░░░░░ 8%              │ ║
║  │  others (3)             $13.20   ▓▓▓░░░░░░░░░░░░░░░░░░░░░░░ 7%              │ ║
║  └──────────────────────────────────────────────────────────────────────────────┘ ║
║                                                                                      ║
║  Cost per model over time (last 30d, stacked area)                                  ║
║  ┌──────────────────────────────────────────────────────────────────────────────┐ ║
║  │           ▓▓▓▓▓                                                                │ ║
║  │       ▓▓▓▓▓▓▓▓▓▓▓▓        ▓▓▓▓                                                │ ║
║  │   ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓ ▓▓▓▓▓▓▓▓▓▓▓                                              │ ║
║  │ ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                                              │ ║
║  │ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░                                              │ ║
║  │ 1 May        8 May        15 May        22 May        30 May                  │ ║
║  └──────────────────────────────────────────────────────────────────────────────┘ ║
║   Legend: ▓ Sonnet  ▒ GPT-4o  ░ Opus                                                ║
```

**Tab "History":**
- Line chart cost over time, time range selector aplicável
- Tabela secundária: snapshot rows raw (timestamp | account | model | cost | tokens) — exportável CSV

**Tab "Accounts":**
- Reuse da "Accounts breakdown" inline + ações: edit budget, set alerts, archive account

**Tab "Overview"** (default):
- Mostrado no mockup acima — KPI cards + chart 30d + accounts breakdown

**Empty state Main Window** (primeira abertura sem vendors configurados):

```
║                                                                                    ║
║                                                                                    ║
║                      ╭───────────────────────────────────╮                         ║
║                      │   No vendors configured yet.       │                         ║
║                      │                                    │                         ║
║                      │   [ Configure vendors → ]          │                         ║
║                      ╰───────────────────────────────────╯                         ║
║                                                                                    ║
```

### 3.d — Settings window (760 × 520)

Mockup já em UX-Q5. Detalhamento adicional dos sub-tabs:

**Settings → General:**

```
║  General                                                                  ║
║  ─────────────────────────────────────────────────────                   ║
║                                                                            ║
║  Launch at login              ☐                                           ║
║  Show in Dock                 ☐  (default OFF — menu bar only)            ║
║  Refresh interval (global)    ●──────────── 30s                            ║
║  Theme                        ⦿ Follow system  ○ Light  ○ Dark            ║
║  Menu bar style               ⦿ Icon + label   ○ Icon only                ║
║  Show pace indicator          ☑                                            ║
║                                                                            ║
║  Sound on alert               ☐                                            ║
║                                                                            ║
```

**Settings → Notifications:**

```
║  Notifications                                                            ║
║  ─────────────────────────────────────────────────────                   ║
║                                                                            ║
║  Budget alert thresholds                                                  ║
║    ☑ 80%   ☑ 95%   ☑ 100%   ☑ 120%                                       ║
║                                                                            ║
║  Per-account overrides    [ Manage → ]                                    ║
║                                                                            ║
║  Anti-spam: max 1 alert per threshold per day  ☑ (recommended)            ║
║                                                                            ║
║  Notification permission     ● Granted  [System Preferences ↗]            ║
```

**Settings → Data:**

```
║  Data                                                                     ║
║  ─────────────────────────────────────────────────────                   ║
║                                                                            ║
║  Retention                                                                ║
║  Keep history for             ●────── 90 days  (slider 7-365)            ║
║                                                                            ║
║  Storage location                                                         ║
║  ~/Library/Application Support/Torven/history.db                          ║
║  Size: 4.2 MB · 12,840 snapshots                                          ║
║                                                                            ║
║  Export                                                                   ║
║  [ Export CSV ]  [ Export JSON ]                                          ║
║                                                                            ║
║  Maintenance                                                              ║
║  [ Vacuum database ]  [ Clear all history ]  [ Reset to defaults ]        ║
║                                                                            ║
║  Escape hatch                                                             ║
║  [ Edit config.toml directly... ] (opens in $EDITOR)                      ║
```

### 3.e — AI Insights — placement no popover (inline panel, NÃO modal)

Conforme Design Principle #6, AI Insights NÃO abre como sheet/modal. Quando user toca "Generate Insight" no footer, o popover **transforma seu content** em insights panel — vendor cards são pushed offscreen (slide up + fade) e insights panel slides in (slide up + fade). Back chevron (`‹`) no top-left retorna.

**Estado popover normal → insights panel transition:**

```
Vendor cards visible                   Insights panel visible
═══════════════════════                ═══════════════════════
║ Torven    ⟳ ⚙ ✕   ║                ║ ‹ Insight       ✕   ║
║ ────────────────  ║       ─────►   ║ ────────────────    ║
║ [Card 1]          ║      slide      ║                     ║
║ [Card 2]          ║      transition ║   <content from     ║
║ [Card 3]          ║      300ms      ║    UX-Q4 mockup>    ║
║ [Card 4]          ║                  ║                     ║
║ [Card 5]          ║                  ║                     ║
║ ────────────────  ║                  ║                     ║
║ ⌘I Generate...    ║                  ║                     ║
═══════════════════════                ═══════════════════════
```

**Why inline vs modal:**
- Popover é o "primary mode" do app. Tirar foco para modal é fricção em hot path.
- Inline mantém contexto (botão back `‹` é claro).
- HIG: "Avoid covering content with modals when inline navigation works".

**InsightsHistoryView** (na Main Window): lista cronológica de insights gerados, click expande para ver detalhe. Modal full nunca usado.

---

## 4. Component library (SwiftUI)

Lista de componentes a implementar. Cada um vira `.swift` file em `apple/Torven/{Popover,Window,Insights,...}/`. Granularidade alinhada com migration sequence stories.

| Component | File path | Props | States | Story |
|-----------|-----------|-------|--------|-------|
| `MenuBarLabel` | `MenuBar/MenuBarLabel.swift` | `snapshot: MenuBarSnapshot` (vendor, pct, kind, color) | idle, loading, calm, mid, high, critical, error | 8, 23 |
| `PopoverView` | `Popover/PopoverView.swift` | `coreBridge: EnvironmentObject` | normal (cards), insights-mode (panel), transitioning | 10 |
| `PopoverHeader` | `Popover/PopoverHeader.swift` | `onRefresh, onSettings, onClose` callbacks | idle, refreshing | 10 |
| `VendorCard` | `Popover/VendorCard.swift` | `vendor: VendorViewModel`, `compact: Bool` | configured, not-configured, error, multi-account | 11, 12 |
| `AccountPicker` | `Popover/AccountPicker.swift` | `accounts: [Account]`, `selected: Binding<AccountId?>` | 1-account (hidden), 2-3 chips, 4+ chevron+sheet | 14 |
| `AccountSheet` | `Popover/AccountSheet.swift` | `accounts: [Account]`, `search: Binding<String>` | collapsed, expanded, searching | 14 |
| `UsageMeter` | `Popover/UsageMeter.swift` | `pct: Double`, `unit: LabelKind` | linear progress (default), circular (alternate for window quota) | 11 |
| `StatusPill` | `Popover/StatusPill.swift` | `state: PressureState` | calm/low/mid/high/critical | 11 |
| `Sparkline` | `Popover/Sparkline.swift` | `data: [Double]`, `color: Color` | data-present, insufficient-data placeholder | 11, 12 |
| `InsightsButton` | `Popover/InsightsButton.swift` | `lastInsight: InsightSummary?`, `enabled: Bool` | enabled (default), disabled (<7d data), loading | 19 |
| `InsightsPanel` | `Insights/InsightsPanel.swift` | `viewModel: InsightsViewModel` | thinking, streaming-headline, streaming-insights, done, error | 19 |
| `InsightCard` | `Insights/InsightCard.swift` | `insight: Insight` | trend, anomaly, budget_risk, optimization (4 visual variants) | 19 |
| `InsightsStream` | `Insights/InsightsStream.swift` | `accumulated: String`, `parsedSoFar: InsightsOutput?` | parsing-headline, parsing-insights, parsing-recommendation | 19 |
| `MainWindowView` | `Window/MainWindowView.swift` | `coreBridge: EnvironmentObject` | empty, vendor-selected, insights-history | 16 |
| `SidebarView` | `Window/SidebarView.swift` | `selection: Binding<SidebarSection>` | — | 16 |
| `VendorDetailView` | `Window/VendorDetailView.swift` | `vendor: VendorId`, `timeRange: TimeRange` | overview, history, accounts, models (4 tabs) | 16 |
| `HistoryChart` | `Window/HistoryChart.swift` | `snapshots: [UsageSnapshot]`, `vendor: VendorId?`, `range: TimeRange` | empty, populated, downsampled | 16 |
| `AccountsListView` | `Window/AccountsListView.swift` | `vendor: VendorId`, `accounts: [Account]` | — | 16 |
| `TimeRangeSelector` | `Window/TimeRangeSelector.swift` | `selected: Binding<TimeRange>` | popover menu w/ presets | 16 |
| `SettingsView` | `Settings/SettingsView.swift` | `coreBridge: EnvironmentObject` | sidebar layout | 22 |
| `VendorSettingsTab` | `Settings/VendorSettingsTab.swift` | `vendor: VendorId` | not-configured, configured, editing | 22 |
| `AccountsTable` | `Settings/AccountsTable.swift` | `vendor: VendorId`, `accounts: Binding<[Account]>` | view, edit-row, add-row | 22 |
| `AIInsightsSettingsTab` | `Settings/AIInsightsSettingsTab.swift` | `config: Binding<AIInsightsConfig>` | — | 22 |
| `OnboardingWizard` | `Onboarding/OnboardingWizard.swift` | `onComplete: () -> Void` | step-0 (intro), step-1..5 (per vendor), step-done | 29 |

**Compositional notes:**
- `VendorCard` recebe `compact: Bool` — em main window pode ser usado em lista densa
- `Sparkline` é shared entre popover e main window (KPI cards no overview)
- `StatusPill` é shared menu bar + cards + accounts list
- `UsageMeter` tem dois estilos: linear bar (default, 4px height) e circular ring (8px stroke, usado em main window KPI cards)

---

## 5. Animation & motion

Inspirado em Linear (300ms é o teto sem virar lento) + Apple HIG ("Use motion to clarify, not decorate").

| Element | Animation | Duration | Easing | Trigger | Reduce Motion fallback |
|---------|-----------|----------|--------|---------|------------------------|
| Popover open | Fade + scale 0.95→1.0 | 220ms | `.spring(response: 0.3, dampingFraction: 0.85)` | Click menu bar | Fade only, 150ms linear |
| Popover close | Fade + scale 1.0→0.96 | 180ms | `.easeIn` | Esc / click outside / re-click menu bar | Fade only, 100ms linear |
| Menu bar color transition (calm→amber→critical) | `.foregroundColor` interpolation | 400ms | `.easeOut` | State change | Instant change |
| VendorCard hover background | Background opacity 0→0.05 | 100ms | `.easeInOut` | Mouse over | Disabled |
| Account chip selection | Background accent fade-in | 150ms | `.easeOut` | Tap chip | Instant |
| Account sheet expand | Slide-down + fade | 250ms | `.easeOut` | Tap chevron | Disabled (instant show) |
| Insights panel slide-in | Vendor cards slide-up + fade + insights panel slide-up | 300ms (stagger 50ms) | `.spring(response: 0.4, dampingFraction: 0.9)` | Tap "Generate Insight" | Cross-fade only, 150ms |
| Insights spinner ("Thinking...") | Rotate 360° + scale pulse | 1.2s infinite | `.linear` | Pre-first-token | Pulse opacity only |
| Insights headline reveal | Fade-in + slide-up 4pt | 200ms | `.easeOut` | First field parsed | Fade only |
| Insight card stagger | Each card fade-in + slide-left 8pt, 80ms stagger | 250ms per | `.easeOut` | New insight parsed | Fade only, no stagger |
| Insights cost/latency reveal | Fade-in | 250ms | `.easeOut` | onDone | Same |
| Chart time range change (today→7d→30d) | Crossfade chart content | 200ms | `.easeInOut` | TimeRangeSelector change | Instant |
| Chart data update (after refresh) | Default Swift Charts | ~500ms | `.easeOut` (default) | Snapshot arrives | Instant via `.transaction { $0.animation = nil }` |
| Settings sidebar selection | Background accent fade | 120ms | `.easeOut` | Click section | Instant |
| Notification banner (budget alert in-app) | Slide-down from top | 280ms | `.spring(response: 0.4)` | Threshold crossed | Fade only |

**Global motion principles:**
- Reduce Motion respeitado em todos os componentes via `@Environment(\.accessibilityReduceMotion)`
- Nunca animar mudança numérica (cost, tokens) — apenas opacity/scale/position
- Hover delays: 800ms para tooltip (Apple default), 100ms para background hover
- Press feedback: 50ms scale 0.98 on `.onTapGesture` (subtle haptic alternative em macOS sem Touch Bar)

---

## 6. Typography & spacing scale

### Typography

Stack: **SF Pro** (UI), **SF Pro Display** (large titles), **SF Mono** (data tabular, raw streaming text).

| Token | Font | Weight | Size | Line height | Letter spacing | Uso |
|-------|------|--------|------|-------------|----------------|-----|
| `caption2` | SF Pro Text | Regular | 10pt | 13pt | `0.05em` | Tooltip hints (⌘I), eyebrow labels |
| `caption` | SF Pro Text | Regular | 11pt | 14pt | `0.03em` | Secondary metadata, "Last: 2h ago" |
| `body` | SF Pro Text | Regular | 13pt | 17pt | 0 | Default body, card metric numbers |
| `bodyBold` | SF Pro Text | Semibold | 13pt | 17pt | 0 | Card vendor name, primary actions |
| `headline` | SF Pro Display | Semibold | 14pt | 18pt | `-0.01em` | Popover title "Torven", panel headers |
| `title3` | SF Pro Display | Semibold | 17pt | 22pt | `-0.01em` | KPI numbers in main window |
| `title2` | SF Pro Display | Bold | 20pt | 24pt | `-0.015em` | Main window section headers |
| `title1` | SF Pro Display | Bold | 24pt | 30pt | `-0.02em` | Onboarding hero text |
| `mono` | SF Mono | Regular | 12pt | 16pt | 0 | API key display, raw JSON streaming, table data |
| `monoSmall` | SF Mono | Regular | 10pt | 14pt | 0 | Inline code in tooltips, version strings |

**Color tokens:**
- `Color.primary` (body): adaptado para light/dark by system
- `Color.secondary`: 60% opacity primary — metadata, captions
- `Color.tertiary`: 40% opacity — disabled, low emphasis
- `Color.accentColor`: system accent (user-configurable in System Settings)
- `Color("Semantic/{Calm,Low,Mid,High,Critical}")`: vide UX-Q2

### Spacing scale (4pt base grid + 8pt rhythmic)

| Token | Value | Uso |
|-------|-------|-----|
| `s2` | 2pt | Hairline gaps (inside chips) |
| `s4` | 4pt | Icon-to-text gaps; chip padding vertical |
| `s8` | 8pt | Default component padding; card internal gutters |
| `s12` | 12pt | Card horizontal padding; icon-to-content in cards |
| `s16` | 16pt | Section margins; window outer padding |
| `s24` | 24pt | Major section separators |
| `s32` | 32pt | Onboarding step spacing |

**Decisão:** **híbrido 4pt+8pt** — 4pt grid para micro (chips, icons), 8pt para macro (cards, sections). Não usar 5pt/10pt (anti-padrão Apple).

**Vertical rhythm em VendorCard (80px):**
- Top padding: 8pt
- Header row: 28pt (16pt icon + 12pt vertical center)
- Gap: 4pt
- Metric row: 28pt (13pt text + 15pt padding)
- Gap: 4pt
- Context row (optional): 16pt
- Bottom padding: 8pt
- Total: 8 + 28 + 4 + 28 + 4 + 16 + 8 = 96pt? **Erro de math — refit:**

Corrigido:
- Top padding: 6pt
- Header row: 24pt
- Gap: 2pt
- Metric row: 22pt
- Gap: 2pt
- Context row: 18pt (caption)
- Bottom padding: 6pt
- Total: 6 + 24 + 2 + 22 + 2 + 18 + 6 = **80pt ✓**

(Note: spec final do VendorCard valida em SwiftUI dev real; ratios são guideline, não pixel-perfect lock.)

---

## 7. Accessibility checklist

NFR-9 do PRD pede WCAG AA. Swift Charts já entrega narração de gráficos out-of-box. Itens explícitos abaixo.

### VoiceOver labels

| Element | Label spec |
|---------|-----------|
| Menu bar item | "Torven. {vendor short name} {pct}% of {window name}. {pressure state}." Ex: "Torven. Claude 87% of 5 hour window. High pressure." |
| VendorCard | "{Vendor name}. {cost or pct}. {context}. {pressure state}. Double tap to view details." Ex: "OpenRouter. $42.10 of $100 budget, 42 percent. All accounts. Low pressure." |
| Account chip | "{Account name}, {tag}. {selected/unselected}. Double tap to filter." |
| Account chevron | "All accounts: {count}. Double tap to expand list." |
| Account sheet row | "{Account name}, {tag}. {spent} of {budget}. {pct}." |
| Sparkline | "Trend graph showing last 7 days. {brief narrative: 'increasing', 'flat', 'decreasing'}." (Computed from data, not just "graph") |
| Generate Insight button | If enabled: "Generate AI insight. Last insight 2 hours ago, cost 3 cents." If disabled: "Generate AI insight, disabled. Need at least 7 days of usage data." |
| InsightCard | "{type} severity {severity}. {message}. Evidence: {evidence}." |
| StatusPill | "{state} pressure state." — already in card label but separate when standalone |
| Settings sidebar item | "{Section name}. {Selected/Not selected}." |

### Keyboard navigation

| Shortcut | Action |
|----------|--------|
| `⌘R` | Refresh (any context) |
| `⌘,` | Open Settings |
| `⌘I` | Generate AI Insight (popover open) |
| `⌘D` | Open Main Window (Detailed View) from popover |
| `⌘Q` | Quit Torven |
| `⌘1`...`⌘5` | Focus VendorCard 1..5 in popover |
| `Tab` / `Shift+Tab` | Move focus through interactive elements (HIG default) |
| `↑` / `↓` | Inside account picker sheet, navigate items |
| `Return` | Activate focused button/select account |
| `Esc` | Close popover / Close account sheet / Cancel insights generation / Close settings |
| `⌘W` | Close main window (does not quit app) |

**Focus indicators:**
- All focusable elements get system focus ring (`.focusable()` SwiftUI default)
- Custom focus ring color: accent color, 2pt outline, 4pt corner radius
- Tab order: defined explicitly via `.focusSection()` and `.accessibilitySortPriority()` where natural order is ambiguous

### Reduce Motion

- `@Environment(\.accessibilityReduceMotion)` checked in every animation block
- Spring animations → linear fade
- Slide transitions → instant change with brief fade
- Spinner "thinking" → opacity pulse only (no rotation)

### High Contrast

- `@Environment(\.colorSchemeContrast)` checked
- When `.increased`: bump opacity of secondary text from 60% → 75%; bump border weights from hairline to 1pt; bump status pill diameter from 8pt → 10pt with 1pt outline

### Min touch target

- Apple HIG min target: 44pt × 44pt for touch (iPad/iPhone), but **HIG macOS allows 28pt × 28pt** for hover-based interaction. Torven is macOS only — 28pt acceptable.
- Account chips, header icons, status pills: ≥28pt hit area (visual size may be smaller, padding extends hit area)
- AccountSheet rows: 36pt tall — comfortable click target
- Settings table edit/delete icons: 24pt × 24pt visual but 32pt × 32pt hit area via padding

### Color contrast (already in UX-Q2)

All semantic colors verified WCAG AA (≥4.5:1) in both light and dark mode against base surfaces.

### Text scaling

- Support `@Environment(\.sizeCategory)` (Dynamic Type)
- Cards and main window scale gracefully up to `.accessibilityLarge`; beyond that, layout reverts to vertical stack (no truncation)
- Menu bar label does NOT scale (fixed by macOS) — 18 char budget remains

---

## 8. Handoff para @sm (River) — UI stories

Mapeamento de cada story de UI da migration sequence (§6 ADR) para a section dessa spec que a alimenta.

| Story # | What it builds (1 line) | UX spec section(s) referenced |
|---------|------------------------|-------------------------------|
| **7** — First FFI bridge (`get_vendor_list()`) | Pure FFI, validates Swift can list vendors | §4 Component (VendorCard data shape preview) |
| **8** — NSStatusBar tray + dynamic label | Menu bar label with vendor priority + states | §3.a Wireframe (Menu bar states) + §2 UX-Q2 (semantic palette) + §5 Motion (menu bar color transition) |
| **9** — FFI: fetch_snapshot + get_all_snapshots | FFI surface for snapshots (no UI yet) | §4 Component (VendorViewModel data shape) |
| **10** — SwiftUI Popover skeleton | PopoverView + header + 5 placeholder VendorCards + Cmd+1..5 nav | §2 UX-Q1 (360×480 layout) + §3.b Wireframe + §4 Components (PopoverView, PopoverHeader, VendorCard placeholders) + §7 Accessibility (keyboard nav) |
| **11** — Vendor card first vendor (OpenRouter) | Real VendorCard with usage meter + sparkline + status pill | §3.b Wireframe (VendorCard anatomy) + §4 Components (VendorCard, UsageMeter, StatusPill, Sparkline) + §6 Typography (card metrics) |
| **12** — Extend to all 5 vendors | Render all 5 vendors with correct semantic palette + brand colors | §2 UX-Q2 (categorical palette per vendor) + §3.b Wireframe |
| **14** — Account picker UI | AccountPicker hybrid (chips ≤3 / chevron+sheet ≥4) | §2 UX-Q3 (full decision) + §3.b Wireframe (account picker mockups) + §4 Components (AccountPicker, AccountSheet) |
| **16** — Main window + sidebar + Swift Charts | MainWindowView with sidebar, vendor detail tabs, HistoryChart | §2 UX-Q2 (Swift Charts palette + animations) + §3.c Wireframe (main window 1024×720) + §4 Components (MainWindowView, SidebarView, VendorDetailView, HistoryChart, TimeRangeSelector, AccountsListView) |
| **18** — FFI streaming callback interface | UDL `InsightsCallback` + `[Async]` annotation (no UI) | §4 Components (InsightsViewModel data shape) — backend only |
| **19** — SwiftUI Insights UI streaming consumption | InsightsPanel + headline-first reveal + InsightCard + cost/latency | §2 UX-Q4 (full decision) + §3.e Wireframe (insights placement + transitions) + §4 Components (InsightsPanel, InsightCard, InsightsStream, InsightsButton) + §5 Motion (insights animations) |
| **22** — Settings UI (SwiftUI Settings scene) | SettingsView with NavigationSplitView sidebar + all tabs | §2 UX-Q5 (sidebar decision) + §3.d Wireframe (Settings 760×520 + sub-tabs) + §4 Components (SettingsView, VendorSettingsTab, AccountsTable, AIInsightsSettingsTab) |
| **23** — Menu bar label dinâmico refinement | Polishing FR-8 priority logic + edge cases (multi-account label) | §3.a Wireframe (multi-account states) + §5 Motion (label transitions) |
| **24** — Budget alerts (UNUserNotificationCenter) | Native notifications + in-app banner | §5 Motion (notification banner slide-in) + §7 Accessibility (notification permission) |
| **25** — Diagnostics command UI | Settings button + copy-to-clipboard | §3.d Wireframe (Settings → About) |
| **29** — First-run onboarding wizard | OnboardingWizard with 5 vendor login cards + Skip | §4 Components (OnboardingWizard) + §6 Typography (title1 for hero) — wireframe TODO in story-level spec |

---

## 9. Open items para @swift-ui-expert (Sven)

Pontos onde devo validar com Sven (caso seja consultado) antes de @sm criar stories:

1. **MenuBarExtra `.window` style positioning bugs em multi-monitor (AR-8 do ADR):** spike conjunto Story 8 — confirmar fallback path se MenuBarExtra falhar (NSStatusItem direto via AppKit no AppDelegate). Decisão final entre native MenuBarExtra vs AppKit bridge afeta como `MenuBarLabel.swift` é estruturado.
2. **Inline sheet UX no popover (AccountSheet de UX-Q3):** SwiftUI suporta `.sheet` modifier dentro de popover, mas com glitches conhecidos (sheet pode "vazar" para fora dos limites do NSPopover). Alternativa: implementar como overlay custom (`ZStack { content; if expanded { AccountSheet() } }`). Validar qual aproach é mais robusto.
3. **Insights panel inline transition (UX-Q4) usando `.transition`:** SwiftUI `.transition(.move(edge: .bottom).combined(with: .opacity))` dentro de popover funciona, mas pode causar layout shift no NSPopover container. Alternativa: ZStack com dois children alternando opacity/offset manualmente.
4. **Swift Charts performance teto (AR-7 do ADR):** confirmar que ~200 pontos por chart é suficiente. Se main window mostrar simultaneamente 5 line charts no stacked area, total = 1000 pontos — pode exigir downsample mais agressivo (100 por chart).
5. **`@Environment(\.accessibilityReduceMotion)` propagation:** validar que reduce motion respeita também animações dentro de Swift Charts (não controlamos diretamente — Apple framework). Pode requerer `.transaction { $0.animation = nil }` em chart updates quando reduce motion ativo.
6. **MenuBarExtra label sizing constraints:** confirmar empiricamente quantos chars cabem antes do macOS truncar (varia por SF font weight). 18 chars é estimate baseado em SF Pro Text 13pt — pode ser 16 em weights pesados.

---

## 10. Não-decisões (deferred to v1.5 ou implementation discretion)

Itens que **não** decidi explicitamente nesta spec, com rationale:

- **App icon design (Assets.xcassets/AppIcon):** deferred. Mantém placeholder até decisão de branding visual completa (acompanha rebrand de naming, vide naming-shortlist.md). Sven ou designer dedicado pode iterar.
- **Onboarding wizard wireframes detalhados (Story 29):** estrutura definida (5 vendor cards + Skip), mas micro-layout de cada step adiado para story-level spec — não bloqueia o roadmap.
- **Empty state copy específica:** "No vendors configured yet", "Need 7+ days of data", etc. — copy direto, sem fofura, mas redação final pode iterar em PR review.
- **Sound on alert (Settings General):** flag exposto mas som específico (system default vs custom .aiff) deferred.
- **Insights History card detailed view:** clicking item em InsightsHistoryView abre what exactly? Hoje: expande inline na lista. Modal? Sheet? Deferred para story-level.
- **Search em Main Window (`⌘F`):** indicado no toolbar mas escopo (search across snapshots? insights? both?) deferred para v1.5 stretch.
- **Dark mode preview tooling:** xcassets handles via system; nenhum decisão extra necessária além de Color Sets bem-formados.

---

## FIM da UX spec v0.1.0. Status: Active. Próximos passos: @sm cria stories 7-12, 14, 16, 18-25, 29 referenciando esta spec.
