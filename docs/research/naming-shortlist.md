# Naming Shortlist — rebrand `ai-usagebar`

> Pesquisa de naming com validação de disponibilidade (domínio, github, crates.io, npm, USPTO).
> Autor: @analyst (Alex) — 2026-05-30
> Sucessor de [`competitive-analysis.md`](./competitive-analysis.md).

---

## Contexto

O projeto está pivotando:
- Stack: Tauri 2 + React + TypeScript + Rust core
- Plataforma: macOS-only (descontinua Waybar/Linux)
- Posicionamento: **"LLM dev tool com eval depth"** (não menu bar tracker)
- Audiência: AI Engineer hiring manager / serious LLM devs
- Features L0: AI Insights + eval pipeline + prompt versioning + structured outputs + cost/latency budgets

O nome atual (`ai-usagebar`) carrega 3 problemas:
1. Hyphenated + "AI" + "bar" → vibe consumer dashboard, não LLM tooling
2. Implica "barra/widget", contradizendo o pivot para app nativo macOS com eval depth
3. Não é registrável como marca (descritivo demais; "AI usage" é genérico)

---

## Critérios

| Critério | Detalhe | Veto? |
|---|---|---|
| **Curto** | ≤8 chars, 1-2 sílabas, pronunciável em inglês | ⚠ severo |
| **Estética AI dev tool** | Vibe Linear/Cursor/Modal/Vercel/Anthropic (não Twitter/SaaS genérico) | ⚠ severo |
| **Sem "AI" no nome** | Saturado, sinaliza follower em vez de líder | 🔴 veto |
| **Sem hyphen/números** | Quebra branding | 🔴 veto |
| **Sem sufixo -ly/-ify/-app** | Vibe 2014, não 2026 | 🔴 veto |
| **Conceito ressonante** | forecasting, eval, tokens, budget, signal, observability | preferência |
| **`.dev` ou `.app` disponível** | `.com` quase sempre taken; aceitamos `.dev` se livre | obrigatório |
| **crates.io disponível** | Hard requirement — é o nome do binário Rust | 🔴 veto |
| **github org/user disponível ou inativo** | Bloqueio só se user ativo | preferência |
| **Sem trademark vivo em class 9/42** | Software/SaaS | 🔴 veto |
| **Sem produto LLM/dev tool homônimo ativo** | Risco de confusão de mercado | 🔴 veto |

---

## Metodologia de validação

| Coluna | Método |
|---|---|
| `.com / .dev / .app` | `dig +short NS {domain}` — NXDOMAIN = AVAILABLE; presença de NS = REGISTERED; depois `WebFetch` para confirmar squatter vs. produto ativo |
| `crates.io` | `GET https://crates.io/api/v1/crates/{name}` — 200 = taken, 404 = available |
| `npm` | `GET https://registry.npmjs.org/{name}` — 200 = taken, 404 = available |
| `GitHub` | `GET https://github.com/{name}` — 200 = exists (status: active vs empty), 404 = available |
| `USPTO` | Busca via Google `site:uspto.report "{name}"` e análise manual de live/dead marks em classes 9 (software) e 42 (SaaS) |

**Notas sobre falsos positivos:**
- `.com` registrado mas com squatter/parking = considerado "taken" (não vale a briga)
- GH user existente com 0 repos público = não-bloqueador (não diluí brand)
- Trademark abandonado/dead = disponível para registro
- Marca em classe não-software (ex: amusement park, restaurant) = risco baixo

---

## Candidatos × Disponibilidade

| # | Nome | .com | .dev | .app | crates.io | npm | GitHub | USPTO software | Verdict |
|---|------|------|------|------|-----------|-----|--------|----------------|---------|
| 1 | **Torven** | ❌ Mercedes-Benz repair (Espanha, since 1982) [whois] | ✅ NXDOMAIN | ✅ NXDOMAIN | ✅ 404 | ✅ 404 | ⚠ user existe (EverQuest hobby, 2 forks) | ✅ nenhuma marca direta; só TORVO/THORAVEN similares | 🟢 **Ideal** |
| 2 | **Loomex** | ⚠ no DNS resolution na .com (provavelmente parked/dropped) | ✅ NXDOMAIN | ✅ NXDOMAIN | ✅ 404 | ✅ 404 | ⚠ user existe (LOOMEX) com 0 repos públicos | ✅ nenhuma marca; só LUMEX (pavers) similar | 🟢 **Ideal** |
| 3 | **Scopebar** | ✅ "No match" no whois | ✅ NXDOMAIN | ✅ NXDOMAIN | ✅ 404 | ✅ 404 | ✅ 404 (user não existe) | ✅ nenhuma marca encontrada | 🟢 **Ideal** |
| 4 | **Evalbar** | ❌ "Evalguesser" game site na .com | ✅ NXDOMAIN | ✅ NXDOMAIN | ✅ 404 | ✅ 404 | ❌ 404 (user não existe) | ✅ nenhuma marca encontrada | 🟢 **Ideal** |
| 5 | **Cresci** | ❌ cresci.com (GoDaddy, since 2000, TLS broken — squatter) | ✅ NXDOMAIN | ✅ NXDOMAIN | ✅ 404 | ✅ 404 | ⚠ Rafael Cresci, dev real (0 repos públicos) | ✅ nenhuma marca encontrada | 🟢 **Viable** (sobrenome italiano comum) |
| 6 | **Promptbench** | ❌ Microsoft owns (Feb 2025, Dynadot) | ✅ NXDOMAIN | ❌ Cloudflare hosted | ✅ 404 | ✅ 404 | 🔴 **github.com/microsoft/promptbench** projeto ativo (JMLR 2024 paper) | 🔴 Microsoft Research artifact | 🔴 **Blocked** |
| 7 | **PromptDeck** | ❌ Squarespace 2020 | ❌ inwx.eu registered | ✅ NXDOMAIN | ✅ 404 | ❌ taken | ⚠ user existe, 1 repo JS | ✅ nenhuma marca encontrada | 🟡 **Viable** (mas óbvio/genérico, conota "prompt library" não eval) |
| 8 | **Foreseer** | ❌ foreseer.com (GoDaddy 2003) | ✅ NXDOMAIN | ❌ registrar-servers (NameCheap parking) | ✅ 404 | ❌ React boilerplate by itsankitverma | ⚠ user existe, 2 forks (ESP8266/Graphify) | ✅ nenhuma marca | 🟡 **Viable** (mas npm + .com taken; longo) |
| 9 | **TokenWatch** | [unverified .com] | ❌ Cloudflare | ❌ Cloudflare | ✅ 404 | [unverified] | ⚠ org existe, 0 repos | [unverified] | 🟡 (provavelmente um produto crypto) |
| 10 | **Vellix** | ❌ vellix.com → domainhero (squatter) | ✅ NXDOMAIN | ❌ Porkbun (provavelmente o owner do vellix.io) | ✅ 404 | ✅ 404 | ⚠ user existe, 0 repos | ❌ "VELLIX ADVENTURE PARK" class 41 (amusement park, não-software) **MAS** 🔴 **vellix.io = AI test automation platform com Amazon/Walmart como clientes** | 🔴 **Blocked** (LLM dev tool conflict direto) |
| 11 | **Halyard** | ❌ halyard.com (Ballast PE) | ❌ Cloudflare | ❌ Cloudflare | [skipped] | ❌ taken (2016) | ❌ Puppet config user | 🔴 multi-conflict: O&M Halyard (medical), Halyard Labs AUS (AI consultancy), **halyard.io = active dev project management tool em waitlist** | 🔴 **Blocked** |
| 12 | **Pace** | ❌ Pace empresa global | ❌ Cloudflare (taken) | ❌ **Microsoft Online owns pace.app!** | ❌ pace crate exists (Erin Bohrer) | ❌ taken | ⚠ org existe | 🔴 PACE serial 97032673 LIVE + múltiplas registrações ativas | 🔴 **Blocked** (era top original mas crates.io taken + multi-TM + MS owns .app) |
| 13 | **Wattage** | ❌ wattage.com (MarkMonitor, since 1996 — premium domain) | [unverified] | ❌ Afternic (parking-for-sale) | ❌ wattage crate exists | ❌ taken (random data lib) | ❌ **github.com/wattage** = Wattage Inc. Toronto org (wattage.io) | ✅ nenhuma marca software direta; "TURNING WASTE INTO WATTAGE" (Scosche) é electrical | 🔴 **Blocked** (crates + npm + gh + wattage.io = Toronto startup) |
| 14 | **Tally** | ❌ tally.so (forms — Belgium) | ❌ Cloudflare | ❌ CSC DNS (premium) | ❌ tally crate exists | ❌ taken | ❌ owner active | 🔴 Tally Technologies LIVE + Tally Labs (24 marks) + DappHero Tally (governance class 9 PaaS) | 🔴 **Blocked** |

---

## Recomendação top-3 com rationale completo

### 🥇 **Torven** — score 8.5/10

**Pitch (1 linha):** Neologismo curto, vibe nórdico-engineering, .dev/.app/crates/npm todos livres, zero conflito com produtos LLM/dev tools.

**Análise:**
- **Fonologia:** TOR-ven, 2 sílabas, 6 chars, fricativa + nasal — fácil pronúncia EN/PT
- **Estética:** Mesma família visual de **Modal**, **Replit**, **Anvil**, **Foundry** — soa "infra forjada", combina com Rust core
- **Etimologia ad-hoc:** Vagamente sugere "Tor" (tower/throne em nórdico antigo) + sufixo `-ven` (Modus operandi: Cresta, Cerebras). Permite branding "the tower that watches your LLM stack"
- **Conflitos verificados:**
  - 🟢 **crates.io**: 404, livre
  - 🟢 **npm**: 404, livre
  - 🟢 **.dev / .app**: ambos NXDOMAIN (livres)
  - 🟡 **.com**: torven.com = Torven S.A., oficina Mercedes-Benz na Espanha desde 1982 (zero overlap com software dev tool; classe de negócio totalmente diferente, idioma diferente)
  - 🟡 **GitHub**: `github.com/torven` é hobby account com 2 forks de EverQuest emulator — não-bloqueador (sem brand colision)
  - 🟢 **.io**: registrado via Porkbun mas sem produto detectado (provavelmente squatter/individual — mais fácil de adquirir do que .com)
  - 🟢 **USPTO**: sem TM "Torven" registrada; apenas similares (TORVO vacuum cleaners, THORAVEN ventures, TORVINESQ) — todos em classes diferentes, sem risco de likelihood-of-confusion para software
- **Trade-offs:**
  - ⚠ Etimologia não óbvia → exige investir em explicação de brand story
  - ⚠ Pode ser confundido com "Korven", "Norven", "Sorbet" se mal-pronunciado em call
  - ⚠ Tier 2 risk: futura oficina Torven na Espanha pode reclamar se brand crescer muito (mas classes diferentes = improvável)
- **Best fit para:** Posicionamento "LLM dev tool com eval depth" — soa engineering-first, não consumer

**Por que ganha:** Único candidato com TODAS as 4 propriedades críticas verdes (crates, npm, .dev, .app) e zero conflito com produto LLM/dev/AI ativo. .com taken mas em domínio (auto-mecânica espanhol) que NUNCA gerará confusão.

---

### 🥈 **Loomex** — score 7.5/10

**Pitch (1 linha):** Portmanteau de "loom" (tear, tecer) + "ex" (execução); evoca threading/orquestração de LLM chains; tudo crítico livre.

**Análise:**
- **Fonologia:** LOOM-ex, 2 sílabas, 6 chars, lateral + bilabial — pronúncia clara
- **Estética:** Família de **Codex**, **Vortex**, **Cortex** — soa AI/infra moderna. "Loom" também é a primitiva async do Tokio (`tokio-loom`), o que dá guinada nerd-credible para o público Rust
- **Etimologia narrável:** "Loom" = tecer (associado a fios de execução, costurar prompts em pipelines); "ex" = execute. O loom que executa seu eval.
- **Conflitos verificados:**
  - 🟢 **crates.io**: 404
  - 🟢 **npm**: 404
  - 🟢 **.dev / .app**: ambos NXDOMAIN
  - 🟡 **.com / .io**: loomex.io = Cloudflare (registrado, mas sem produto visível); .com sem NS resolvido
  - 🟡 **GitHub**: `github.com/LOOMEX` user existe, 0 repos, achievement "Quickdraw" — provavelmente squatter individual
  - 🟢 **USPTO**: sem TM "Loomex"; apenas LUMEX (pavers), LOMVEXR (electronic cases), LOONYX (toilet paper) — classes totalmente alheias
- **Trade-offs:**
  - ⚠ "Loom" pode evocar Loom.com (video messaging, Atlassian) — risco de associação cognitiva (não legal, mas perceptual)
  - ⚠ Sufixo "-ex" tem leve sabor 2018 (Codex foi 2021 mas...); menos timeless que Torven
- **Best fit para:** Quando você quer comunicar **orquestração** (eval pipelines, prompt chains) como pilar central

---

### 🥉 **Scopebar** — score 7.0/10

**Pitch (1 linha):** Compound word literal — "scope" (telescópio/observability) + "bar" (homenageia raiz ai-usagebar); .com, .dev, .app, github, todos livres.

**Análise:**
- **Fonologia:** SCOPE-bar, 2 sílabas, 8 chars (no limite), pronúncia inequívoca
- **Estética:** Compound concreto à la **Sidebar**, **Toolbar**, **Statusbar** — soa **descritivo** mas industrial. Família de **Stripe**, **Linear**, **Plane**: objetos do mundo físico aplicados ao digital
- **Conceito:** "Telescópio sobre sua LLM stack". Comunica observability/eval com clareza imediata; carrega a continuidade do "bar" original (zero confusão para early adopters)
- **Conflitos verificados:**
  - 🟢 **.com**: whois "No match" — disponível em todos os 3 TLDs principais
  - 🟢 **.dev / .app**: ambos NXDOMAIN
  - 🟢 **crates.io**: 404
  - 🟢 **npm**: 404
  - 🟢 **GitHub**: 404 (user/org disponível)
  - 🟢 **USPTO**: sem matches diretos em busca pública
- **Trade-offs:**
  - 🔴 **Crítico:** Soa **literal/descritivo** demais — "scope" + "bar" carrega a vibe Waybar antiga que o pivot quer descontinuar. Mantém a frame mental "menu bar widget" que estamos abandonando
  - ⚠ 8 chars no limite (Linear é 6, Cursor é 6, Modal é 5)
  - ⚠ Descritivos têm dificuldade de TM por causa de "merely descriptive" rejection em class 9
- **Best fit para:** Se quiser **mínimo risco operacional** (zero conflito hoje) e **continuidade narrativa** com o nome antigo. Pior fit para storytelling "pivot agressivo para LLM eval tool"

**Por que entra:** É o **safe pick**. Se Torven/Loomex parecerem arriscados demais por etimologia opaca, Scopebar é zero-friction registrável. Mas paga preço em diferenciação de marca.

---

## Honorable mentions (não recomendados, mas válidos)

| Nome | Por quê não top-3 | Por quê listo |
|------|-------------------|----------------|
| **Evalbar** | github 404, mas evalbar.com já tem "Evalguesser" (jogo, não AI dev tool — risco baixo). Carrega o sufixo "-bar" como Scopebar, pior diferenciação | Domínios .dev/.app livres; crates/npm livres; barato pivotar |
| **Cresci** | .com (squatter de 2000) e .io taken; carrega conotação italiana (significa "cresci/eu cresci" em italiano — "I grew") | Foneticamente bonito; .dev/.app/crates/npm livres |
| **Foreseer** | .com taken; npm taken (React boilerplate inativo); 8 chars no limite | Conceito de forecasting é perfeito para Insights L0 |

---

## Riscos transversais

1. **Trademark search é raso.** As verificações via Google + uspto.report são best-effort. Antes de qualquer registro formal, **rodar busca completa via attorney** (gerbenlaw, jpglegal, ou TM360 Pro) cobrindo USPTO classes 9 + 42, common-law marks, e international (EUIPO/INPI) — orçamento ~$300-$800.

2. **Domínios .com sempre taken para nomes de 5-7 letras.** Aceite isso. Modern AI dev tools (Modal, Vercel, Anthropic, Cursor) compraram .com por 5-7 dígitos. Use .dev/.app/.io até atingir Series A.

3. **GitHub orgs com username squatting são adquiríveis.** Politica do GitHub permite reivindicar usernames inativos via [Trademark Policy](https://docs.github.com/en/site-policy/content-removal-policies/github-trademark-policy) (60-90 dias).

4. **crates.io é o único veto real para projeto Rust.** É hard-blocking porque o binário precisa publicar com esse nome. Os top-3 (Torven/Loomex/Scopebar) estão todos 404 — green light.

5. **Branding linguístico:** Torven soa bem em EN/PT/ES. Loomex pode ser confundido com "Lumix" (Panasonic câmeras) em PT. Scopebar é internacionalmente neutro.

---

## Próximos passos sugeridos

**O usuário (Lorenzo) precisa decidir:**

1. **Hierarquia de critério:** valoriza mais "diferenciação de marca" (Torven), "narrativa de pivot" (Loomex), ou "zero risco operacional" (Scopebar)?
2. **Budget para trademark search formal:** $300-$800 com attorney antes de registrar.
3. **Plano de domínio:** comprar `.dev` + `.app` da escolha (~$25/ano cada na Cloudflare/Porkbun); plano para adquirir `.com` quando $$$ permitir.
4. **GitHub org:** criar imediatamente após decidir (12-24h para confirmação), reservar o user também.
5. **Crates.io reservation:** publicar `0.0.0` placeholder em crates.io para evitar squatter durante o rebrand.

**Antes do rebrand executivo:**
- [ ] Confirmar com Lorenzo qual dos 3 vence
- [ ] Rodar trademark search formal (orçamento ~$400)
- [ ] Comprar .dev + .app + .io (se livre)
- [ ] Criar github org + reservar crates.io placeholder
- [ ] Atualizar Cargo.toml, README, packaging/aur/, .github/workflows/

**Sequência sugerida:** decisão final → trademark clearance → domain purchase → GH org → crates reservation → branch `rebrand/{name}` → PRD começa **com novo nome no PRD desde o título**.

---

## Apêndice — Fontes & métodos

- **crates.io API:** `https://crates.io/api/v1/crates/{name}` (200 vs 404)
- **npm registry:** `https://registry.npmjs.org/{name}`
- **GitHub:** `https://github.com/{name}` (200 vs 404)
- **DNS NS lookup:** `dig +short NS {domain}` — NXDOMAIN = livre
- **USPTO via Google:** `site:uspto.report "{name}"` para identificar trademarks listados
- **WebFetch direto** em produtos suspeitos (vellix.io, halyard.io, elide.dev, vellum.ai, promptra.dev, evallo.com, sondra.com, macula.dev, cresta.com, wattage.io, halyard.com) para confirmar conflitos ativos
- **Whois local** para .com (NS resolução, registrar identity, creation date)

Total candidatos avaliados: ~50. Top 14 incluídos na tabela. Top 3 com análise completa.

**Confidence:** ALTO para os 3 finalistas em crates/npm/.dev/.app (verificação direta). MÉDIO para USPTO (limitação de bots em TESS). Recomendação fechar trademark search com attorney antes de commit final.
