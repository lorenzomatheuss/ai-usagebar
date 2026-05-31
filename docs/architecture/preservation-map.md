# Preservation Map — Waybar → macOS native pivot

Inventário de cada módulo do crate atual, classificado em quatro buckets:

- **PRESERVE** — código cross-platform, fica intacto
- **MIGRATE** — fica, mas precisa adaptar interface ou shape
- **DELETE** — Linux/Waybar-coupled, será removido
- **NEW** — não existe ainda, vai ser criado no pivot

Esse mapa alimenta o ADR (fase 4) e as stories (fase 6). É a fonte da verdade para a pergunta "esse arquivo sobrevive ao pivot?".

---

## Resumo executivo

| Bucket | Arquivos | % do código atual (LOC aprox.) |
|---|---:|---:|
| PRESERVE | 32 | ~76% |
| MIGRATE | 4 | ~6% |
| DELETE | 11 | ~18% |
| NEW | — | ~+30% sobre o tamanho atual |

**Confirma a tese inicial:** ~82% do código é reaproveitável (PRESERVE + MIGRATE). Os ~18% que saem são todos da camada de apresentação Waybar/Pango/Omarchy.

---

## PRESERVE — 32 arquivos (fetchers + lógica de negócio)

Toda a lógica de fetch, OAuth, types e cache é cross-platform. Não toca nada.

### Vendor modules (24 arquivos, ~7.5k LOC)

| Path | Razão |
|---|---|
| `src/anthropic/{mod,fetch,oauth,creds,types}.rs` | Fetcher Claude Max OAuth — funciona em qualquer plataforma |
| `src/openai/{mod,fetch,oauth,creds,types,vendor}.rs` | Fetcher Codex/ChatGPT — idem |
| `src/openrouter/{mod,fetch,types,vendor}.rs` | OpenRouter API key-based |
| `src/zai/{mod,fetch,types,vendor}.rs` | Z.AI GLM monitor |

### Core compartilhado (6 arquivos, ~2k LOC)

| Path | Razão |
|---|---|
| `src/cache.rs` | Cache atômico com flock — vale para qualquer arquitetura |
| `src/countdown.rs` | Cálculo de tempo até reset — puro |
| `src/error.rs` | Tipos de erro centralizados |
| `src/pacing.rs` | Cálculo de pace (gasto vs esperado) — puro |
| `src/usage.rs` | `VendorSnapshot` enum + types — IPC interface entre Rust core e React |
| `src/vendor.rs` | `VendorId` enum + `VendorOutcome` — idem |

### TUI (5 arquivos, ~1.5k LOC)

| Path | Razão |
|---|---|
| `src/tui/{mod,app,panels,settings,view}.rs` | Ratatui é cross-platform. **Decisão:** manter o binary `torven-tui` como dev tool / fallback CLI. Custa ~0 manter, ganha "I can run this from the terminal" no pitch. |
| `src/bin/torven-tui.rs` | Entry point do TUI bin |

### Tests (mantém)

| Path | Razão |
|---|---|
| `tests/anthropic_e2e.rs` | E2E com mockito — testa fetcher, não widget. Stays. |
| `tests/fixtures/`, `tests/snapshots/` | Fixtures e snapshots — mantêm validação dos fetchers |
| `tests/live.rs` | Live smoke tests (`make smoke`) — continuam valendo |

---

## MIGRATE — 4 arquivos (precisa adaptar interface)

Esses arquivos têm lógica boa mas estão acoplados ao output Waybar. Vão sobreviver mas com refactor.

| Path | O que muda | LOC |
|---|---|---:|
| `src/config.rs` | Mantém `Config` + TOML loading. **Adicionar:** método `to_json()` para expor ao frontend React via Tauri command. **Adicionar:** suporte opcional a Tauri Store para preferências runtime. | ~300 |
| `src/format.rs` | Atualmente gera strings Pango-markup tipo `<span color="#fff">29%</span>`. **Mudar para:** retornar structs serializáveis (`FormattedSnapshot { color: "low"|"mid"|"high"|"critical", text: "29%", ... }`) que o React renderiza. Pango sai, semantic colors entram. | ~150 |
| `src/lib.rs` | Remove `pub mod pango/theme/tooltip/waybar/widget`. Adiciona `pub mod insights` (NEW), `pub mod history` (NEW). | ~30 |
| `src/anthropic/creds.rs` (e similares nos outros vendors) | OAuth refresh já funciona em macOS. **Verificar:** caminho default de creds em macOS (`~/Library/Application Support/Claude/.credentials.json` vs `~/.claude/.credentials.json`). Possível ajuste de `default_path()`. | ~50 |

---

## DELETE — 11 arquivos (Waybar/Pango/Omarchy)

Camada de apresentação Linux. Some inteira.

| Path | Razão de deletar |
|---|---|
| `src/waybar.rs` | `WaybarOutput` JSON + SIGRTMIN signaling — Waybar não existe em macOS |
| `src/pango.rs` | Pango markup helpers — substituído por React |
| `src/tooltip.rs` | Pango bordered-box tooltip — substituído por popover Tauri |
| `src/theme.rs` | Detecção do tema Omarchy — não existe em macOS; tema vem do macOS appearance (light/dark) |
| `src/active.rs` | Estado da scroll-cycle do Waybar — sem scroll na status bar do mac |
| `src/widget/{mod,cli,pretty,render,run}.rs` | Shell inteiro do widget Waybar |
| `src/bin/torven.rs` | Entry point do widget — substituído pelo binary Tauri |

### Packaging Linux

| Path | Razão |
|---|---|
| `packaging/aur/` | AUR é Arch Linux apenas. Substituído por `tauri.conf.json` + bundling .app + notarization |
| `.github/workflows/release.yml` (parcial) | Builds Linux x86_64/aarch64 saem. Build macOS .app entra (universal binary x86_64 + aarch64) |

---

## NEW — módulos a criar

### Rust (no crate ou no `src-tauri/`)

| Path proposto | Função | Origem |
|---|---|---|
| `src/insights/` | **Feature diferenciadora.** Cliente da Anthropic Messages API que recebe snapshots históricos + contexto e gera análise tipo "seu uso de Claude pulou 3x na última semana, principalmente em sessões de 8-10pm; considere mover trabalho pesado para a janela de 5h pré-reset". Streaming de resposta. | Novo, depende de `~/.config/torven/api_keys.toml` ou Tauri secure storage |
| `src/history/` | Persistência local de snapshots ao longo do tempo (SQLite via `rusqlite` ou JSONL diário). Alimenta o `insights/` e a janela detalhada com gráficos. | Novo |
| `src/menubar/` (opcional) | Bridge para texto dinâmico na NSStatusBar — pode viver dentro do Tauri ou expor via comando | Novo, possivelmente delegado ao plugin `tauri-plugin-positioner` ou `tray-icon` crate |

### Tauri + Frontend (top-level)

| Path proposto | Função |
|---|---|
| `src-tauri/` | App Tauri 2: `tauri.conf.json`, `Cargo.toml` do binary do app, `main.rs` registrando commands e tray |
| `web/` (ou `src-ui/`) | React + TypeScript + Vite. Components: `MenuBarLabel`, `Popover`, `VendorCard`, `InsightsPanel`, `MainWindow`, `SettingsView` |
| `web/src/lib/tauri.ts` | Type-safe wrapper dos Tauri commands (`get_vendor_snapshot`, `request_insights`, `update_config`, etc.) |

### Tests novos

| Path | Função |
|---|---|
| `tests/insights_e2e.rs` | Mock da Anthropic Messages API; valida que `insights` produz output útil dado um histórico |
| `web/tests/` | Vitest + React Testing Library para componentes UI |

---

## Resumo visual da transformação

```
ANTES (Waybar-coupled)            DEPOIS (macOS native)
─────────────────────             ──────────────────────
bin/torven (Waybar)   ─▶     src-tauri/ (Tauri 2 app)
bin/torven-tui        ───    bin/torven-tui (mantém)
src/widget/                 X     web/ (React + TS)
src/waybar.rs               X     web/src/components/MenuBarLabel.tsx
src/pango.rs / tooltip.rs   X     web/src/components/Popover.tsx
src/theme.rs                X     (system appearance via Tauri API)
src/active.rs               X     (sem scroll-cycle)
src/{anthropic,openai,...}  ─▶    src/{anthropic,openai,...} (intacto)
src/cache.rs/usage.rs       ─▶    src/cache.rs/usage.rs (intacto)
src/format.rs (Pango)       ──    src/format.rs (structured)
src/config.rs               ──    src/config.rs (+to_json)
                                  src/insights/ (NEW — AI Insights)
                                  src/history/ (NEW — SQLite)
packaging/aur/              X     src-tauri/tauri.conf.json (bundle .app)
```

`─▶` PRESERVE · `──` MIGRATE · `X` DELETE · `(NEW)` criado no pivot

---

## Implicações para o ADR (fase 4)

Decisões abertas que o @architect precisa fechar:

1. **Workspace Cargo:** monolítico (um crate só) ou multi-crate (`torven-core` lib + `torven-tauri` bin + `torven-tui` bin)?
   - Recomendação: workspace com `core` lib + 2 bins. Limpa a separação entre o que é fetcher e o que é frontend.

2. **History storage:** SQLite via `rusqlite` (mais robusto, query-friendly, +1 dep nativa) ou JSONL diário (zero-dep, append-only, harder to query)?
   - Recomendação: SQLite. AI Insights precisa de queries temporais.

3. **AI Insights — API key strategy:** usar a mesma OAuth do usuário (reutiliza creds Claude Max — gratuito até hit do plano) ou exigir API key separada (mais previsível, mas paga)?
   - Recomendação: OAuth primeiro (zero friction), fallback para API key configurada.

4. **TUI binary fate:** continuar empacotando no .app ou separar?
   - Recomendação: separar. .app fica leve, TUI fica como `cargo install torven-tui` para devs.

5. **Tauri commands vs events:** snapshots via command (request/response) ou event broadcast (push do core para UI quando refresh)?
   - Recomendação: híbrido. Initial load via command, atualizações via event (`vendor-snapshot-updated`).
