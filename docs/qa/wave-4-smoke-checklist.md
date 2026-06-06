# Wave 4 — Smoke Checklist (SMK-001 → SMK-006)

**Objetivo:** validar manualmente no app rodando todas as features da Wave 4 (Stories 4.1 a 4.6) num único launch. Pattern audit-trail estabelecido em Wave 2/3.

**Como rodar:** `cd apple && DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer xcodebuild -project Torven.xcodeproj -scheme Torven -configuration Debug build && open ~/Library/Developer/Xcode/DerivedData/Torven-*/Build/Products/Debug/Torven.app` — ou Build → Run no Xcode (⌘R).

**Depois de executar:** anexar um bloco textual com os 6 resultados ao Change Log da Story 4.6 (`docs/stories/epics/epic-torven-v1/4.6.budget-burn-indicator.story.md`) na seção `## Change Log`.

---

## SMK-001 — Story 4.1: Main Window shell + ⌘1 hotkey

> **Status atual:** Já feito 5/5 ✅ pré-orquestração (registrado em session_checkpoint). Pular se já confirmado.

| # | Teste | Resultado |
|---|---|---|
| 1.1 | App limpo → primeira ⌘1 → Accessibility permission prompt | ✅ |
| 1.2 | ⌘W → janela some sem dealloc, app fica no menu bar | ✅ |
| 1.3 | ⌘1 após ⌘W → reabre com mesmo state | ✅ |
| 1.4 | ⌘1 com Torven em background → app foreground + janela visível | ✅ |
| 1.5 | ⌘1 múltiplos cliques rápidos → 1 instância (idempotente) | ✅ |

---

## SMK-002 — Story 4.2: DateRangePicker (7d/30d/Custom)

| # | Teste | Resultado |
|---|---|---|
| 2.1 | Abrir Main Window via ⌘1 ou "Show History…" → 3 segmentos visíveis no top bar | ✅ |
| 2.2 | Segmento "7d" ativo por default ao abrir | ✅ |
| 2.3 | Clicar "30d" → segmento switcha, sem DatePicker visível | ✅ |
| 2.4 | Clicar "Custom" → 2 DatePickers `.compact` + Apply button aparecem | ✅ |
| 2.5 | Setar end < start → Apply fica desabilitado + caption red "End date must be after start date" | ☐ |
| 2.6 | Setar range válido → Apply enabled → tap aplica (verificável quando charts wireados — qualquer chart re-renderiza) | ☐ |

---

## SMK-003 — Story 4.3: StackedAreaChart cost-by-vendor

| # | Teste | Resultado |
|---|---|---|
| 3.1 | Chart renderiza (placeholder "History — Wave 4" sumiu) | ☐ |
| 3.2 | Com `history.db` vazio (ou range sem dados) → empty-state "No Data" + ícone chart.xyaxis.line | ☐ |
| 3.3 | Com dados → stacked area com até 5 cores de vendor + legend no rodapé | ☐ |
| 3.4 | Y-axis formatado como USD (símbolo `$`) | ☐ |
| 3.5 | Trocar DateRangePicker 7d → 30d → Custom → chart re-renderiza, sem flicker de stale data | ☐ |
| 3.6 | Hover sobre o chart → dashed vertical RuleMark aparece + annotation card no top-trailing com data + breakdown por vendor (cor + nome + custo) | ☐ |

---

## SMK-004 — Story 4.4: PerVendorGrid + selection filter

| # | Teste | Resultado |
|---|---|---|
| 4.1 | ViewMode picker visível ao lado do DateRangePicker (segmentos "Aggregate" / "Per-vendor") | ☐ |
| 4.2 | Default mode = Aggregate (StackedAreaChart) | ☐ |
| 4.3 | Click "Per-vendor" → LazyVGrid com 5 MiniVendorChart cards (layout 3+2 em janela ≥820pt, 2+2+1 em ≤560pt) | ☐ |
| 4.4 | Cada card mostra: vendor display name + total $ + color swatch + LineMark chart | ☐ |
| 4.5 | Clicar em qualquer card → border colorida aparece (selection highlight, lineWidth 2) | ☐ |
| 4.6 | Voltar para "Aggregate" mantendo seleção → stacked area mostra SÓ o vendor selecionado | ☐ |
| 4.7 | Voltar para "Per-vendor" + clicar no MESMO card → border some (toggle) | ☐ |
| 4.8 | Reduce Motion enabled (System Settings → Accessibility) → toggle Aggregate↔Per-vendor é instantâneo (sem fade) | ☐ |

---

## SMK-005 — Story 4.5: Cost/Requests metric toggle

| # | Teste | Resultado |
|---|---|---|
| 5.1 | Terceiro Picker "Cost/Requests" visível ao lado do ViewMode no top bar | ☐ |
| 5.2 | Default = "Cost" → Y-axis em USD, hover card mostra "$X.XX" | ☐ |
| 5.3 | Clicar "Requests" → Y-axis vira inteiros (sem `$`), hover card mostra "N requests" | ☐ |
| 5.4 | Per-vendor mode + Requests → cada MiniVendorChart header mostra "N requests" no total (em vez de "$X.XX total") | ☐ |
| 5.5 | Toggle rápido Cost↔Requests → range selecionado e modo (Aggregate/Per-vendor) **NÃO** resetam (state independence) | ☐ |
| 5.6 | Resize janela narrow → top bar (DateRange + ViewMode + Cost/Requests + Spacer) permanece em uma linha até ~656pt; abaixo disso pode quebrar — observar comportamento | ☐ |

---

## SMK-006 — Story 4.6: BudgetBurn gauge (closer)

> **Pré-requisito:** editar `~/Library/Application Support/torven/config.toml` (macOS) ou `~/.config/torven/config.toml` (Linux dev box). Adicionar/remover a seção `[budgets]` para exercitar os paths.

### Sem `[budgets]` configurado

| # | Teste | Resultado |
|---|---|---|
| 6.1 | Abrir Main Window com config.toml SEM `[budgets]` → footer NÃO visível (gauge ausente + Divider ausente — zero pixels footprint) | ☐ |

### Com `[budgets]` configurado

Adicione ao config.toml (use valores baixos para conseguir simular as 3 thresholds rapidamente):

```toml
[budgets]
monthly_usd_total = 1.0

[budgets.per_vendor]
openrouter = 0.50
anthropic = 0.50
```

Quit e reabra o Torven (ou apenas ⌘W + ⌘1 para forçar reload — verificar se basta).

| # | Teste | Resultado |
|---|---|---|
| 6.2 | Footer aparece com Divider + barra de progresso + label "Budget" no rodapé do ChartContent | ☐ |
| 6.3 | Label de spend formato `$X.XX / $1.00` | ☐ |
| 6.4 | Se gasto atual < 80% do total → cor **verde** | ☐ |
| 6.5 | Forçar 80-99% do cap (ou editar config para reduzir o cap) → cor **amber/orange** | ☐ |
| 6.6 | Forçar ≥100% do cap → cor **vermelha** | ☐ |
| 6.7 | Trocar DateRangePicker (range) → BudgetBurn atualiza (cadência de refresh aligned com chart reload) | ☐ |
| 6.8 | Reduce Motion enabled → mudança de cor é instantânea (sem easeInOut fade) | ☐ |
| 6.9 | (opcional, hardware-dependent) Em macOS 13.0 vs 14+ → ambos paths funcionam (Gauge vs ProgressView fallback) | ☐ |

---

## Após executar — bloco para anexar ao Change Log de Story 4.6

Edite `docs/stories/epics/epic-torven-v1/4.6.budget-burn-indicator.story.md` e adicione na tabela `## Change Log`:

```markdown
| 2026-06-06 | Lorenzo (runtime smoke) | Wave 4 smoke batch confirmation: SMK-001 5/5 ✅ (pre-orchestration), SMK-002 X/6 [seu resultado], SMK-003 X/6, SMK-004 X/8, SMK-005 X/6, SMK-006 X/9. Total: X/40 ✅. [Observações livres: o que quebrou ou surpreendeu]. |
```

---

## Carry-forwards conhecidos (não-bloqueantes, esperados)

- **TZN-001:** se você está em GMT-3 e abriu o app perto da meia-noite do dia 1, o BudgetBurn pode resetar com até 24h de diferença do wall clock — UTC month-boundary é a escolha cravada em AC-7, Wave 5 pode trazer timezone preference.
- **HOV-001:** annotation card do StackedAreaChart fica top-trailing fixo (não segue o cursor) — Wave 5 polish.
- **DES-002:** MiniVendorChart não tem hover overlay (omitido propositalmente em 4.4 — UX call em card de 260pt) — Wave 5 polish.
- **DES-001:** se você usar uma futura "chart click-to-zoom" que mude o `dateRange`, o picker "7d/30d/Custom" não vai auto-virar pra `.custom` — limitação one-way conhecida, Wave 4.4 não introduziu zoom então não tripa.
