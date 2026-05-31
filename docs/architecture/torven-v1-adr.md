# Torven v1.0 — Architecture Decision Record (ADR) v2 [SwiftUI + Rust FFI]

> **Autor:** @architect (Aria — Visionary)
> **Data:** 2026-05-31
> **Status:** Active
> **Supersedes:** `docs/architecture/discarded/torven-v1-adr-tauri.md` (Tauri path — discarded 2026-05-31)
> **Predecessores:** `docs/prd/torven-v1.md`, `docs/architecture/preservation-map.md`, `docs/research/competitive-analysis.md`
> **Sucessores:** UX spec (@ux-design-expert), epics e stories (@po + @sm)
> **Stack decidido:** **SwiftUI app + Rust core via FFI** (binding tool decidido em ADR-4)
> **Formato:** Michael Nygard ADR style — uma decisão por seção, com Context, Options, Decision, Consequences

---

## Change Log

| Date | Version | Description | Author |
|------|---------|-------------|--------|
| 2026-05-31 | 0.1.0 | ADR v1 (Tauri path) | @architect |
| 2026-05-31 | 0.2.0 | **ADR v2 — pivot Tauri → SwiftUI + Rust FFI.** Reaproveita ADR-1 (workspace), ADR-2 (Keychain), ADR-3 (Sparkle), ADR-5 (SQLite), ADR-7 (tool_use), ADR-8 (eval runner). Refaz ADR-4 (IPC vira FFI), ADR-6 (charts vira Swift Charts). Adiciona ADR-9 (Xcode project structure) e ADR-10 (App entry point). | @architect |

---

## 1. Status & Provenance

- **Status:** Active
- **Supersedes:** `docs/architecture/discarded/torven-v1-adr-tauri.md`
- **Date:** 2026-05-31
- **Stack:** SwiftUI app + Rust core via FFI (binding tool: uniffi — vide ADR-4)
- **Trigger da reversão:** após entrega e revisão do ADR v1 (Tauri), avaliação pessoal concluiu que (a) qualidade Maestri-grade ("R$99 vibe" — equivalente a Bartender, ClaudeBar, Tokens 4 Breakfast — todos Swift nativos) exige UI nativa SwiftUI, não webview; (b) optionality comercial futura (Mac App Store, paid tier) é melhor servida por stack 100% Apple-blessed; (c) competitive analysis (`docs/research/competitive-analysis.md`) mostra que TODOS os concorrentes diretos relevantes do segmento são Swift nativos — Tauri vira "Electron-like smell" no momento da review técnica em portfolio.
- **O que sobrevive do ADR descartado:** ADR-1 (workspace Cargo), ADR-2 (Keychain híbrido), ADR-3 (Sparkle), ADR-5 (SQLite rusqlite), ADR-7 (Anthropic tool_use), ADR-8 (eval runner Rust nativo). Esses 6 ADRs são citados explicitamente nas seções correspondentes.

---

## 2. Context

O **Torven** (rebrandado de `ai-usagebar`) executa pivot de Waybar Linux widget para app nativo macOS. A v1 deste ADR (Tauri 2 + React) foi entregue em 2026-05-31 e descartada no mesmo dia após reflexão sobre qualidade exigida pelo segmento: ClaudeBar, Tokens 4 Breakfast, Claude Usage Tracker, CodexBar — todos os top competidores diretos do `docs/research/competitive-analysis.md` são SwiftUI nativos, e o pitch de portfolio AI Engineer perde força ao colocar webview entre o Rust core (audit-friendly, ~14k LOC battle-tested) e o usuário. SwiftUI é o stack que (a) entrega a fluidez Maestri-grade que o segmento espera (popover open p95 <50ms é factível, vs <100ms factível com Tauri), (b) abre optionality comercial (Mac App Store + sandbox são caminhos legítimos com Swift; Tauri tem fricções), e (c) preserva a tese central de "Rust core + LLM eng depth" porque o core continua sendo Rust — apenas a camada de apresentação muda. Este ADR v2 resolve as 10 decisões necessárias para o stack SwiftUI + Rust FFI, reaproveitando 6 das 8 decisões do ADR descartado e adicionando duas novas (ADR-9 Xcode structure, ADR-10 App entry point).

---

## 3. Architectural Decisions

### ADR-1 — Workspace Cargo: multi-crate split (REUSED de v1)

**Status:** Accepted
**Date:** 2026-05-31
**Reuse:** Decisão preservada do `docs/architecture/discarded/torven-v1-adr-tauri.md#adr-1`. Único ajuste: o crate `torven-app` (que no ADR v1 era o bin Tauri 2) **desaparece como crate Cargo** — vira o Xcode project `apple/Torven.app/` (vide ADR-9). O Rust workspace fica com apenas dois crates Cargo: `torven-core` (lib + bin de evals) e `torven-tui` (bin preserved).

#### Context

O crate atual (`/Users/lorenzorodrigues/Documents/Projetos/ai-usagebar/Cargo.toml`) é flat: lib `torven` + dois bins (`torven` widget Waybar, `torven-tui` ratatui). Pós-pivot:

- O app principal vira Xcode project Swift (não é mais um crate Cargo)
- O Rust core precisa ser compilável como **staticlib** (`.a`) para link no Xcode + **lib** (rlib) para os bins Rust
- TUI ratatui é preservado como dev tool / Linux fallback
- Eval runner roda em CI como bin Rust

#### Options

**A. Workspace com 2 crates Cargo + Xcode project separado** (PROPOSTO)
- `crates/torven-core/` — lib (`crate-type = ["staticlib", "rlib"]`) + `[[bin]] name="torven-evals"`. A staticlib `libtorven_core.a` é consumida pelo Xcode build via build script
- `crates/torven-tui/` — bin ratatui, depends on torven-core (rlib)
- `apple/Torven.xcodeproj/` (ou `project.yml` para XcodeGen — vide ADR-9) — Xcode project Swift puro, fora do Cargo workspace

**B. Workspace com 3 crates Cargo** — mantém `torven-app` como bin Rust dummy que apenas faz `cargo build` da staticlib
- Cons: redundante. Xcode já faz o build da app; o bin Cargo seria placeholder

**C. Flat (status quo) com features flags** — manter src/ flat, compilar staticlib via feature
- Cons: já rejeitado no ADR v1 do mesmo modo — vira ilegível, eval runner puxa Tauri/Swift deps desnecessariamente

#### Decision

**Adotar Opção A — workspace com 2 crates Cargo (`torven-core` + `torven-tui`) + Xcode project separado em `apple/`.**

Racional: o Xcode project é gerenciado por xcodebuild/XcodeGen, não por Cargo. O Cargo workspace continua sendo o lar do Rust core + eval runner + TUI; o Xcode project é cliente do `libtorven_core.a` produzido pelo build do core. Essa separação é **mais limpa** que a versão Tauri (que tentava amarrar Tauri dentro do Cargo workspace) e o pitch de portfolio fica ainda mais nítido: "abre `crates/torven-core/src/insights/` para ver LLM engineering; abre `apple/Torven/` para ver SwiftUI puro nativo".

#### Consequences

- **Positivas:**
  - Build do Rust core e do Xcode project são desacoplados — `cargo build` no core não exige Xcode; `xcodebuild` no app não exige `cargo` se o `.a` está cached
  - Recrutador AI Engineer abre o repo, vê estrutura cristalina: `crates/torven-core/` (Rust LLM core) + `apple/Torven/` (SwiftUI shell)
  - v1.5 (cloud sync) entra como novo crate `torven-sync` no workspace sem mexer no Xcode
  - CI workflow consegue rodar `cargo test -p torven-core` em runner ubuntu (sem macOS) para validação rápida
- **Negativas:**
  - Build orchestration tem dois sistemas (Cargo + xcodebuild). Mitigação: Makefile com targets `make build-core`, `make build-app`, `make build-all`, `make universal` documentados
  - Universal binary precisa `cargo build --target aarch64-apple-darwin && cargo build --target x86_64-apple-darwin && lipo -create` ANTES do xcodebuild — vira step explícito no release pipeline
- **Backward compat:** TUI `crates/torven-tui/` migra de `src/tui/` sem mudança funcional; smoke tests `make smoke` continuam contra `torven-core`

---

### ADR-2 — macOS Keychain integration: híbrido Keychain blob + SQLite metadata (REUSED de v1 com ajuste de owner)

**Status:** Accepted
**Date:** 2026-05-31
**Reuse:** Decisão híbrida (blob JSON por vendor no Keychain + metadata queryable em SQLite) preservada do `docs/architecture/discarded/torven-v1-adr-tauri.md#adr-2`. **Ajuste novo:** decidir ONDE mora a lógica de Keychain — no Rust core via `security-framework` crate (como no ADR v1) ou no Swift via `Security.framework` / `KeychainAccess` lib.

#### Context

FR-6 do PRD pede multi-account com API keys "criptografadas em rest via macOS Keychain". O ADR v1 propôs híbrido: Keychain blob JSON `[{account_id, api_key}]` por vendor + SQLite `accounts(account_id, name, tag, budget_usd, ...)` para metadata queryable. O **shape** da decisão é mantido. A pergunta nova é: a lógica de leitura/escrita Keychain mora no Rust (via `security-framework`) ou no Swift (via Apple's first-party Security.framework + opcional wrapper `KeychainAccess`)?

#### Options (sobre LOCATION da lógica)

**A. Keychain logic em Rust (`security-framework` crate)** — Swift chama via FFI `keychain_get_blob(vendor) -> Vec<u8>` e `keychain_set_blob(vendor, blob)`.
- Prós:
  - **Single source of truth no core**: vendors fetchers já em Rust precisam da api_key — fica natural no mesmo crate
  - TUI (Linux fallback) pode usar mock/file-based fallback do mesmo trait `SecretStore`
  - Auditabilidade: o cara que abre `crates/torven-core/src/keychain/` vê tudo
- Cons:
  - `security-framework` crate tem coverage limitada para algumas APIs avançadas (kSecAttrAccessGroup, kSecUseAuthenticationContext)
  - "Always allow this app" prompt UX é mediado pelo CFBundleIdentifier — funciona, mas debug edge cases viram debugar Rust→FFI→Apple stack

**B. Keychain logic em Swift (`KeychainAccess` ou `Security.framework` direto)** — Swift expõe os blobs ao Rust core via FFI quando precisa.
- Prós:
  - Coverage 100% das APIs Apple (KeychainAccess wrap nativo)
  - Debug de prompts/ACL fica natural em Xcode
  - Wrapping `KeychainAccess` é maduro e battle-tested (1Password, Mullvad iOS app, etc.)
- Cons:
  - **Direção de fluxo invertida**: vendor fetcher Rust precisa da api_key → tem que pedir pro Swift → callback FFI. Aumenta complexidade no path mais quente (refresh tick)
  - TUI sem Keychain nativo (precisa branch separado de `SecretStore`)

**C. Híbrido — Swift gerencia ACL/prompt, Rust gerencia leitura na hot path** — Swift faz "unlock" no startup, repassa blob para Rust em cache em memória; Rust não toca Keychain após boot.
- Prós: melhor dos dois (UX prompt no idioma Swift, hot path Rust)
- Cons: complexidade de sync; reload de Keychain após Settings update vira coreografia

#### Decision

**Adotar Opção A — Keychain logic em Rust via `security-framework` crate.**

Racional:
1. **Hot path consistency**: vendor fetchers Rust precisam da api_key para cada request HTTP. Manter o secret store no mesmo crate elimina a coreografia "Swift → FFI → Rust → fetcher".
2. **TUI sobrevivência**: o crate `torven-tui` tem `crates/torven-core` como dependência. Se Keychain estiver em Swift, TUI precisa de stub diferente — vira gambiarra. Em Rust, o trait `SecretStore` tem impls `MacKeychainStore` (default no macOS) e `FileFallbackStore` (TUI Linux) trivialmente.
3. **`security-framework` cobre o caso de uso**: blob JSON read/write com `SecItemAdd`/`SecItemCopyMatching` é o caminho mais batido da crate — versão 2.x estável.
4. **CFBundleIdentifier**: o app SwiftUI define `com.torven.app` em `Info.plist`; o Rust core herda automaticamente quando linkado (não precisa setar manualmente).

**Layout:** `crates/torven-core/src/keychain/`:
- `mod.rs` — trait `SecretStore`, type `VendorBlob` (versionado: `{version: 1, accounts: [{account_id, api_key}]}`)
- `mac.rs` — impl `MacKeychainStore` usando `security-framework::passwords::set_generic_password` / `get_generic_password` com service name `com.torven.app.<vendor>` e account name `accounts-blob-v1`
- `fallback.rs` — `FileFallbackStore` (TUI Linux) gravando em `~/.config/torven/secrets/<vendor>.json` com `chmod 600`

Schema migration: se uma versão futura precisar mudar shape do blob, bump `version` field + migration code em `mac.rs::load_with_migration()`. Versionado no JSON, não no Keychain item name.

#### Consequences

- **Positivas:**
  - Hot path "fetch snapshot" continua puro Rust — sem cruzar FFI
  - TUI mantém `SecretStore` consistente cross-platform
  - First-run UX é 5 prompts macOS no MÁXIMO (1 por vendor configurado) — herda 100% do design ADR v1
- **Negativas:**
  - Spike obrigatório de 0.5d antes da implementação (Story 11 — vide migration sequence): validar `security-framework::passwords::set_generic_password` com 5 vendors, verificar comportamento "always allow this app" no first-run real
  - Se um dia precisarmos de `kSecUseAuthenticationContext` (Touch ID prompts), Refactor: migrar para FFI bridge para Swift ou usar `objc2` + Security.framework manual — não é v1.0
- **Security:** API keys nunca tocam SQLite nem `config.toml`. Logs redatam keys. Export CSV/JSON (FR-12) NÃO inclui api_keys. Swift NÃO armazena api_keys em memória — sempre pede pro Rust via FFI quando precisa exibir mascarado (`sk-or-****abc`)

---

### ADR-3 — Auto-update: Sparkle 2 (REUSED de v1 com detalhamento Swift)

**Status:** Accepted
**Date:** 2026-05-31
**Reuse:** Decisão de adotar Sparkle preservada do `docs/architecture/discarded/torven-v1-adr-tauri.md#adr-3`. **Ajuste:** detalhamento da integração agora é nativa Swift (sem binding Rust intermediário).

#### Context

FR-15 + NFR-7 do PRD pedem auto-update. ADR v1 escolheu Sparkle vs Tauri updater. Com stack Swift, Sparkle vira escolha trivial — é literalmente o framework que ClaudeBar, Tokens 4 Breakfast, iStat Menus e milhares de apps macOS usam. A pergunta nova: detalhes de integração SwiftUI App + Sparkle 2 (versão atual em 2026).

#### Options

**A. Sparkle 2 nativo Swift via SPM (Swift Package Manager)** — adicionar `github.com/sparkle-project/Sparkle` como Package Dependency no Xcode, inicializar `SPUStandardUpdaterController` no `AppDelegate` ou no `@main App`.
- Prós:
  - SDK first-party Sparkle, totalmente Swift-compatible em 2026
  - SPM integration via Xcode (clica em "Add Package Dependencies", paste URL, done)
  - Sparkle 2 introduziu sandbox-compatible updates (relevante para Mac App Store opcional futuro)
- Cons: nenhum relevante

**B. Sparkle 2 via Carthage/CocoaPods**
- Cons: SPM é o caminho moderno em 2026; usar SPM

#### Decision

**Adotar Opção A — Sparkle 2 nativo Swift via SPM**.

**Implementação detalhada:**

- **SPM dependency:** `https://github.com/sparkle-project/Sparkle` versão `~> 2.6` (latest stable em 2026-05)
- **Inicialização (em `TorvenApp.swift` — vide ADR-10):**
  ```swift
  import Sparkle

  @main
  struct TorvenApp: App {
      private let updaterController = SPUStandardUpdaterController(
          startingUpdater: true,
          updaterDelegate: nil,
          userDriverDelegate: nil
      )
      // ... rest of App
  }
  ```
- **Info.plist keys:**
  - `SUFeedURL` = `https://github.com/lorenzomatheuss/torven/releases/latest/download/appcast.xml`
  - `SUPublicEDKey` = EdDSA public key (gerada via `generate_keys` tool da Sparkle 2)
  - `SUEnableAutomaticChecks` = `YES`
  - `SUScheduledCheckInterval` = `86400` (24h, conforme FR-15)
  - `SUParameterizedAutomaticUpdateInstallation` = `YES` (sandbox-friendly install)
- **EdDSA key generation flow (one-time setup):**
  ```
  cd /path/to/Sparkle/bin
  ./generate_keys
  # → outputs: ED25519 keypair
  # PUBLIC key → Info.plist `SUPublicEDKey`
  # PRIVATE key → GitHub Actions secret `SPARKLE_PRIVATE_KEY` (NUNCA commitada)
  ```
- **Appcast.xml generation in CI** (vide Story 23 da migration sequence):
  ```
  cd /path/to/Sparkle/bin
  ./generate_appcast \
      --ed-key-file $SPARKLE_PRIVATE_KEY_PATH \
      --download-url-prefix "https://github.com/lorenzomatheuss/torven/releases/download/v${VERSION}/" \
      --output-path appcast.xml \
      Torven-${VERSION}.zip
  ```
- **Integration com SwiftUI lifecycle:** o `updaterController` é mantido como propriedade do `App`; SwiftUI command pode chamar `updaterController.updater.checkForUpdates()` manualmente via menu item "Check for Updates...". Sparkle 2 mostra dialog padrão macOS quando update disponível — UX consistente com toda app macOS premium.

#### Consequences

- **Positivas:**
  - UX canônica macOS — usuário já conhece Sparkle dialogs de Bartender, iStat Menus, etc.
  - Defense-in-depth contra Apple Developer cert lapse (R-3 do PRD): EdDSA valida bin mesmo se Apple revoga
  - Rollback automático se update falha
  - Sparkle 2 sandbox-compatible — Mac App Store futuro fica aberto
- **Negativas:**
  - EdDSA private key é asset crítico — perda = perde controle de updates. Mitigação: backup encrypted da private key em password manager pessoal + GitHub Actions secret. Documentado em `apple/Sparkle/README.md`
  - Setup inicial de keys é one-time (~30 min) mas precisa ser feito ANTES do primeiro release público (não pode adicionar Sparkle keys depois — clients sem public key embed ignoram updates)
- **Future-proof:** se v2.0 trouxer cross-platform, Sparkle não cobre Windows/Linux; trocar para Tauri updater (ou Squirrel.Windows + custom Linux) vira refactor isolado

---

### ADR-4 (NOVO — substitui ADR-4 v1) — Swift↔Rust FFI binding tool: **UniFFI (Mozilla)**

**Status:** Accepted
**Date:** 2026-05-31
**New:** Esta é a decisão de maior impacto arquitetural do ADR v2. Substitui completamente a ADR-4 v1 (que decidia IPC Tauri commands+events) — não há mais Tauri, então não há mais IPC; há FFI direto Swift↔Rust.

#### Context

Toda interação UI ↔ core agora é via FFI binding. Em 2026, há duas opções dominantes no ecossistema Swift↔Rust:

- **UniFFI** (Mozilla, lançado 2021, batle-tested em Firefox iOS, Mullvad VPN, Matrix Rust SDK Element iOS)
- **swift-rs** (Spacedrive team, lançado 2022, ergonômico)

A escolha define: (a) dev velocity ao adicionar uma nova função do Rust ao Swift; (b) type ergonomics (UDLs declarativos vs Rust derive macros); (c) debug experience quando o FFI dá pau; (d) capacidade de expor types complexos (enums, Vec<T>, callbacks); (e) maturity em production iOS/macOS apps.

#### Options

**A. UniFFI (Mozilla)** — UDL (Universal Definition Language) declarativo: você descreve a interface em arquivo `.udl`, UniFFI gera bindings Swift + Rust glue code.

- **Como funciona:**
  ```udl
  // crates/torven-core/torven_core.udl
  namespace torven_core {
      sequence<VendorSnapshot> get_all_snapshots();
  };

  dictionary VendorSnapshot {
      string vendor;
      string? account_id;
      double? cost_usd;
      string label_text;
  };

  interface InsightsClient {
      constructor(string api_key);
      [Throws=InsightsError]
      void request_insights(InsightsContext context, InsightsCallback callback);
  };

  callback interface InsightsCallback {
      void on_token(string token);
      void on_done(InsightsOutput output);
      void on_error(string error);
  };
  ```
- **Build step:** `uniffi-bindgen generate torven_core.udl --language swift --out-dir generated/`. Gera `torven_core.swift` (chamadas idiomáticas Swift) + `torven_coreFFI.h` (C ABI bridge).
- **Prós:**
  - **Production-tested em iOS/macOS de larga escala**: Firefox iOS, Mullvad VPN, Matrix Element iOS, Signal Foundation usa em research, Bitwarden mobile. Estes são apps que pagam folha — não experimentos
  - **Callback interfaces**: suporta callbacks Swift → Rust nativamente (crítico para AI Insights streaming — `callback interface InsightsCallback` em UDL vira protocolo Swift que o Rust pode chamar)
  - **Async support** (UniFFI 0.25+): `[Async]` no UDL permite `async fn` Rust ser exposto como `async` Swift function. CRÍTICO para AI Insights (Rust faz tokio HTTP call; Swift await direto sem bloquear UI thread)
  - **Type system completo**: Enums com payloads (`enum VendorError { Network(string), Auth, RateLimit }`), `Option<T>` (nullable), `Result<T, E>` (throws Swift), Vec<T> (Array Swift), HashMap, custom types via `[Custom]`
  - **Error mapping idiomático**: `enum InsightsError` Rust vira `throws` Swift function — `try await client.requestInsights(...)` puro
  - **Stable ABI versionada**: `uniffi-bindgen` valida que .udl matches Rust impl — refactors Rust quebram build, não runtime
  - **Documentação rica**: book oficial + exemplos de Mozilla
- **Cons:**
  - UDL é um arquivo a mais de manter (sincronizar com Rust impl). Mitigação: feature opcional `proc-macro` (UniFFI 0.27+) permite derive direto `#[uniffi::export]` sem UDL — mas modo proc-macro ainda é menos batido que UDL para uniffi.
  - Type mapping de tipos exóticos (e.g. `chrono::DateTime<Utc>`) exige declaração custom via `[Custom]` interface
  - Build pipeline tem step extra (`uniffi-bindgen generate`); precisa ser explícito no `build.rs`

**B. swift-rs (Spacedrive)** — derive macros Rust expõem funções diretamente; Swift wrapper gerado por macro `#[swift_bridge::bridge]`.

- **Como funciona:**
  ```rust
  // crates/torven-core/src/swift_bridge.rs
  #[swift_bridge::bridge]
  mod ffi {
      extern "Rust" {
          fn get_all_snapshots() -> Vec<VendorSnapshot>;
          type InsightsClient;
          #[swift_bridge(init)]
          fn new(api_key: String) -> InsightsClient;
          async fn request_insights(self: &InsightsClient, context: InsightsContext);
      }
      extern "Swift" {
          type InsightsCallback;
          fn on_token(self: &InsightsCallback, token: String);
      }
  }
  ```
- **Prós:**
  - Derive-only: tudo no Rust, sem .udl side-file
  - Async/await suporte excelente (Spacedrive usa heavily)
  - Type ergonomics melhores que UniFFI em alguns casos (e.g. RefCounted types em Swift mapeados direto pra Rust types)
  - Setup inicial mais rápido (sem arquivo UDL)
  - Build via build.rs nativo Cargo
- **Cons:**
  - **Production maturity ainda em construção**: Spacedrive (apps), Stronghold (research) — não há ainda iOS app pagando folha do tamanho de Firefox iOS ou Mullvad usando swift-rs. Em 2026, ainda é "newer kid on the block"
  - Comunidade menor → menos Stack Overflow / blog posts
  - Documentação mais rasa que UniFFI book
  - Breaking changes em minor versions historicamente mais frequentes que UniFFI (swift-rs ainda pre-1.0 em 2026)
  - Debug experience: panic em Rust durante FFI call vira crash difícil de rastrear (mesma situação UniFFI mas pior por menos docs)

**C. Plain `extern "C"` + Manual `@_cdecl` Swift** — sem framework, FFI manual.

- **Cons:** trabalho braçal manter symbol declarations, type marshalling, drop semantics. Rejeitado para projeto single-dev (R-5 do PRD).

#### Decision

**Adotar Opção A — UniFFI (Mozilla).**

**Racional rigoroso** (esta é a decisão de maior impacto do ADR — extra rigor):

1. **Production maturity é decisiva.** A diferença entre UniFFI e swift-rs em 2026 não é técnica — ambos resolvem o problema — é **track record**. Quando o Torven cruzar 1000 instalações e um usuário relatar "app trava após 2h", o stack trace vai mergulhar em FFI. UniFFI tem: (a) bugs já documentados publicamente em Firefox iOS (1B+ usuários), Mullvad VPN (~10M usuários), Matrix Element iOS (~1M usuários); (b) workarounds canônicos no book oficial; (c) Stack Overflow e GitHub Issues populated. swift-rs tem Spacedrive (~100k usuários) e research projects. Para um app de portfolio que **PRECISA funcionar sem crash em demo de entrevista**, mature wins.

2. **AI Insights streaming via callback interfaces é canônico no UniFFI.** O caso de uso crítico do produto (FR-7) é Rust async tokio fazer HTTP streaming para Anthropic e empurrar tokens para Swift UI. UniFFI 0.25+ tem:
   - `callback interface` no UDL → vira protocolo Swift que o Rust chama
   - `[Async]` annotation → função Rust async vira `async throws` Swift
   - Memory safety automática (ARC ↔ Rust ownership mediados por UniFFI's reference counting)

   No swift-rs, callbacks funcionam mas debug de leaks de retain cycle é mais artesanal.

3. **Type system completude alinhada com o domínio do Torven.** O domínio tem: `enum VendorError { Network(reason), Auth(vendor), RateLimit { retry_after_ms } }`, `Result<T, E>` em todo lugar, `Option<DateTime>` para reset times, `HashMap<VendorId, Vec<Account>>`. UDL suporta tudo isso idiomático. swift-rs também, mas com mais asterisco e workarounds em enum payloads complexos.

4. **Portfolio narrative.** "Decidi entre UniFFI e swift-rs, escolhi UniFFI porque Firefox iOS / Mullvad / Matrix Element usam em prod" é frase forte em entrevista. É evidence-based engineering. Caracterizar o trade-off bem é signal de senioridade.

5. **Single point of weakness mitigado:** UniFFI book + `cargo uniffi` ferramenta CLI + integration guides Apple-specific (`xcframework` build) tudo disponível. swift-rs precisa de "spelunking nos issues do GitHub" para alguns edge cases.

**Implementação detalhada:**

- **Cargo deps em `crates/torven-core/Cargo.toml`:**
  ```toml
  [dependencies]
  uniffi = { version = "0.27", features = ["tokio"] }

  [build-dependencies]
  uniffi = { version = "0.27", features = ["build"] }
  ```
- **UDL location:** `crates/torven-core/src/torven_core.udl` (single file declaring entire FFI surface)
- **build.rs:**
  ```rust
  fn main() {
      uniffi::generate_scaffolding("src/torven_core.udl").unwrap();
  }
  ```
- **Generation step (manual, committed to repo):**
  ```
  cargo run --bin uniffi-bindgen generate src/torven_core.udl --language swift --out-dir ../../apple/Torven/Bridge/Generated/
  ```
  Output: `torven_core.swift` (idiomatic Swift API) + `torven_coreFFI.modulemap` + `torven_coreFFI.h`
- **xcframework packaging:** o `libtorven_core.a` (universal binary) é empacotado como `.xcframework` para Xcode import. Build script `apple/scripts/build-xcframework.sh`:
  ```bash
  cargo build --release --target aarch64-apple-darwin
  cargo build --release --target x86_64-apple-darwin
  lipo -create \
    target/aarch64-apple-darwin/release/libtorven_core.a \
    target/x86_64-apple-darwin/release/libtorven_core.a \
    -output target/universal/libtorven_core.a
  xcodebuild -create-xcframework \
    -library target/universal/libtorven_core.a \
    -headers crates/torven-core/include/ \
    -output apple/Frameworks/TorvenCore.xcframework
  ```
- **Async strategy:** UniFFI 0.27+ suporta `[Async]` no UDL. Rust functions com `tokio` runtime são expostas como `async throws` Swift. Implementação: `tokio::runtime::Runtime` é mantida como `OnceCell` static no Rust core (lazy init na primeira chamada FFI). Swift await chama Rust → Rust roda no tokio runtime → quando completo, Swift resume. **Não bloqueia thread Swift.** Detalhe crítico para AI Insights streaming (Story 15).

#### Consequences

- **Positivas:**
  - Dev velocity após setup inicial: adicionar nova função vira 3 passos (`#[uniffi::export]` ou linha no UDL → regenerar Swift → consumir em Swift). ~5 min por função
  - AI Insights streaming flows naturalmente (callback interface)
  - Type-safety end-to-end: refactor Rust quebra compile Swift (via Xcode build)
  - Memory safety: UniFFI gerencia ownership; menos chance de leaks
- **Negativas:**
  - Setup inicial de Story 2 (Setup UniFFI binding) é spike de ~1d. Mitigação: documentação rica do UniFFI book + exemplo Mozilla `mozilla/uniffi-rs/examples/`
  - Type mapping exótico (e.g. `chrono::DateTime<Utc>`) exige declarar `[Custom]` interface no UDL — convenção: usar Unix timestamps i64 em FFI surface, converter em Rust e Swift sides separadamente
  - Tokio runtime no FFI context exige cuidado: usar `tokio::runtime::Builder::new_current_thread()` no `OnceCell`, NÃO multi-thread runtime (evita thread leak em scenarios specifically iOS/macOS sandbox)
- **Debug strategy:** panics no Rust durante FFI viram `RustError` no Swift via `Result`. Em modo debug, panic message é preservada; em release, mensagem é genérica (security). Logging via `tracing` crate Rust emite via FFI callback `LogObserver` (Swift implementa, encaminha para `os_log`)

---

### ADR-5 — History storage: SQLite via rusqlite (REUSED de v1)

**Status:** Accepted
**Date:** 2026-05-31
**Reuse:** Decisão preservada **integralmente** do `docs/architecture/discarded/torven-v1-adr-tauri.md#adr-5`. SQLite via rusqlite + schema versionado + retention 90d.

#### Context

FR-12 do PRD especifica persistência local de snapshots em SQLite. AI Insights (FR-7) precisa queries temporais. Pivot Tauri → SwiftUI **não muda em nada** essa decisão — SQLite é storage layer no Rust core, completamente desacoplado da camada de apresentação.

#### Decision

**Adotar SQLite via `rusqlite` (Rust core), schema versionado via `rusqlite_migration`.** Schema completo (tabelas `accounts`, `usage_snapshots`, `insights_history`, `schema_migrations`) preservado do ADR v1.

**Exposição ao Swift via FFI (vide ADR-4):** funções FFI helper:

```udl
// torven_core.udl (excerpt)
namespace torven_core {
    sequence<UsageSnapshot> list_snapshots(string vendor, i64 since_ts);
    sequence<UsageSnapshot> list_snapshots_for_account(string account_id, i64 since_ts);
    AggregatedUsage aggregate_by_vendor(i64 since_ts, i64 until_ts);
    AggregatedUsage aggregate_by_account(string account_id, i64 since_ts, i64 until_ts);
    void export_history_csv(string output_path);
    void export_history_json(string output_path);
};

dictionary UsageSnapshot {
    i64 id;
    string vendor;
    string? account_id;
    i64 ts;
    double? cost_usd;
    i64? tokens_used;
    double? pct_used;
    string metric_kind;
};

dictionary AggregatedUsage {
    sequence<VendorAgg> by_vendor;
    double total_cost_usd;
};
```

**Decisão crítica anti-padrão:** **NÃO usar GRDB ou Swift CoreData no app.** Swift NÃO toca SQLite diretamente. Toda query passa pelo Rust core via FFI. Razões:

1. **Single source of truth.** Eval runner (Story 17 — vide migration sequence), TUI, Swift UI — todos consomem o mesmo `crates/torven-core/src/history/` Rust code. Drift entre o que o eval lê e o que o app mostra é eliminado por construção.
2. **Schema migration discipline.** Uma camada de migration (Rust rusqlite_migration), não duas.
3. **Type safety.** UDL define `UsageSnapshot` once; Rust e Swift consomem o mesmo schema.
4. **Performance.** rusqlite com prepared statements é tão rápido quanto GRDB; o overhead FFI é desprezível para queries que retornam <10k rows (típico do retention 90d).

#### Consequences

- **Positivas:**
  - Idênticas ao ADR v1 (AI Insights query last-7-days <10ms, export trivial, doctor reporta schema_version)
  - Swift fica leve — sem GRDB/CoreData/Realm deps
  - Schema migrations em um lugar só
- **Negativas:**
  - Toda mudança de schema exige regenerar UniFFI bindings (se UDL mudou). Mitigação: schema mudanças são raras; pipeline regenerate é 1 comando documentado
- **Recovery:** se SQLite corrompe, Rust core detecta no startup, renomeia para `history.db.corrupt-{ts}`, cria novo arquivo vazio, aplica migrations. Swift recebe via FFI callback `on_storage_recovery(message)` e exibe toast informativo

---

### ADR-6 (REFAZER) — Charts: **Swift Charts framework** (nativo macOS 13+)

**Status:** Accepted
**Date:** 2026-05-31
**Refazer:** Decisão **completamente refeita** vs ADR v1 (que escolheu Recharts React). Com SwiftUI stack, Swift Charts (introduzido WWDC 2022, maturação WWDC 2023+) é a escolha canônica e gratuita.

#### Context

FR-10 (janela detalhada) precisa de charts de histórico 30 dias por vendor, breakdown por modelo, accounts. macOS 13+ (Ventura) é o target NFR-4. Swift Charts requer macOS 13+ — alinhado.

#### Options

**A. Swift Charts framework (Apple, nativo)** — `import Charts` no SwiftUI, declarative API.

- **Como funciona:**
  ```swift
  Chart(snapshots) { snapshot in
      LineMark(
          x: .value("Date", snapshot.timestamp),
          y: .value("Cost", snapshot.costUsd)
      )
      .foregroundStyle(by: .value("Vendor", snapshot.vendor))
  }
  .chartXAxis { AxisMarks(values: .stride(by: .day)) }
  .chartLegend(position: .bottom)
  .frame(height: 240)
  ```
- **Prós:**
  - **Zero deps** — bundled com macOS 13+. Bundle .app size não muda
  - **Apple-native**: dark mode automático, system accent color automático, animação Core Animation built-in, font scaling automático
  - **Acessibilidade WCAG AA out-of-box** (NFR-9 do PRD) — VoiceOver descreve charts narrativamente, sem trabalho extra
  - **API declarativa SwiftUI-idiomática** — não há context switching entre paradigmas
  - **Performance**: Core Animation hardware-accelerated, suporta animação suave de N pontos sem repaint cost (NFR-1)
  - **Maturity 2026**: WWDC 2023 trouxe Donut/Pie + scrolling charts; WWDC 2024 trouxe vectorized rendering. Maduro
  - **Documentação Apple oficial** rica + Hacking with Swift / Paul Hudson tutorials

- **Cons:**
  - macOS 13+ only — mas é exatamente o target do Torven, então NA
  - Customization extrema (e.g. chart types muito exóticos) requer Canvas custom. Não aplicável ao v1.0 (só line/area/bar)

**B. Pop-Charts (third-party SwiftUI charts library)**

- **Cons:**
  - Apple lançou Swift Charts depois de Pop-Charts; comunidade migrou para Swift Charts. Pop-Charts perdeu momentum
  - Não há razão para escolher third-party quando Apple-native cobre o caso

**C. AAChartKit (Highcharts wrapper Swift)**

- **Cons:**
  - WebView-based (carrega Highcharts JS) — exatamente o que queremos evitar ao sair do Tauri
  - Anti-pattern para Swift puro

**D. Charts (Daniel Cohen Gindi — Charts library port from MPAndroidChart)**

- **Cons:**
  - Legacy library, mantida mas Apple lançou Swift Charts como standard
  - Designed para UIKit/AppKit imperative — não SwiftUI-friendly

#### Decision

**Adotar Opção A — Swift Charts framework.**

Racional curto: é a escolha óbvia. Apple-native + zero-deps + dark mode automático + accessibility WCAG AA grátis + SwiftUI-idiomatic + macOS 13+ é o target. Não há cenário em que outra escolha seja melhor para Torven v1.0.

**Charts a implementar no v1.0:**

1. **Line chart 30 dias custo por vendor** (FR-10 history tab):
   - X: date, Y: cost_usd, colored by vendor
2. **Bar chart breakdown por modelo OpenRouter/Z.AI** (FR-10):
   - X: model name, Y: cost_usd, sorted desc
3. **Area chart accounts comparison** (multi-account view):
   - Stacked area, X: date, Y: cost_usd per account
4. **Sparkline mini-chart no VendorCard popover** (FR-9 — opcional):
   - 7d trend inline ao label métrico

**Stretch (v1.5):** Donut chart breakdown total mensal (Swift Charts 5 trouxe `SectorMark` Sept 2023).

#### Consequences

- **Positivas:**
  - Bundle .app NÃO cresce com chart deps (era ~95KB gzip de Recharts no ADR v1)
  - Dark mode + accent color = zero código custom
  - VoiceOver narrates charts → NFR-9 (Accessibility WCAG AA) entregue por grace
  - Performance suficiente para 90 dias × 5 vendors (~450 pontos) sem otimização
- **Negativas:**
  - macOS <13 não suportado — mas é o NFR-4 target, então NA
  - Performance teto se v1.5 trouxer históricos longos (~10k+ pontos). Mitigação preventiva: downsample server-side Rust antes de enviar ao Swift (1 ponto/hora em vez de 1 ponto/min)

---

### ADR-7 — AI Insights JSON Schema: Anthropic tool_use mode (REUSED de v1)

**Status:** Accepted
**Date:** 2026-05-31
**Reuse:** Decisão de adotar Anthropic `tool_use` mode com `tool_choice: {type: "tool", name: "submit_insight"}` preservada **integralmente** do `docs/architecture/discarded/torven-v1-adr-tauri.md#adr-7`.

#### Context

FR-7 + §6.3 do PRD: AI Insights retorna JSON estruturado. Decisão entre tool_use mode (forçado, robusto) e prompt-only (com retry). Stack pivot Tauri → SwiftUI NÃO afeta esta decisão. O cliente Anthropic Messages API roda 100% em Rust (no `crates/torven-core/src/insights/`); Swift consome o output via FFI callback.

#### Decision

**Adotar Anthropic tool_use mode** com schema versionado em `prompts/insights.v{N}.schema.json`. Detalhes técnicos (definição de tool, streaming via `content_block_delta` events do tipo `input_json_delta`, parse partial JSON, fallback) preservados do ADR v1.

**Detalhamento novo — integração com FFI:**

A chamada Anthropic Messages API é feita do lado **Rust** (no `crates/torven-core/src/insights/client.rs`). O payload streaming retorna para Swift via FFI callback (UDL `callback interface InsightsCallback` — vide ADR-4).

**Pergunta crítica:** O Rust faz `tokio` runtime separado, ou Swift gerencia o async via callback?

**Decisão:** **Rust faz tokio runtime separado, owned pelo `InsightsClient` Rust struct.** Justificativa:

1. **Isolamento de concerns**: Swift NÃO precisa saber que existe HTTP streaming. Swift faz `try await client.requestInsights(context: ..., callback: SwiftCallbackImpl())` e recebe tokens via callback. O fato de Rust usar `tokio` runtime separado é invisível.
2. **Performance**: tokio multi-thread runtime gerencia o HTTP streaming em background thread Rust; UI thread Swift não bloqueia. Callbacks são invocados via FFI; UniFFI marshals callbacks back to main thread Swift automaticamente.
3. **Cancellation**: o handle do request (`InsightsRequestId` UUID) permite Swift chamar `client.cancelInsights(requestId)` — Rust aborta o tokio task via `tokio::sync::oneshot` cancellation signal. CRÍTICO para FR-7 AC "usuário pode cancelar mid-stream".
4. **Single tokio runtime per process**: usar `OnceCell<Runtime>` no Rust core. Init na primeira chamada FFI. Build via `tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()`. 2 worker threads é suficiente para o caso de uso (no max 1 insight streaming + N vendor fetches paralelos).

**Streaming flow detalhado:**

```
Swift UI                Rust core                Anthropic API
   │                       │                          │
   ├─ requestInsights ────►│                          │
   │  (UniFFI async)       │                          │
   │                       ├─ tokio::spawn ──────────►│
   │                       │  HTTP POST streaming     │
   │                       │                          │
   │                       │◄─ chunk: content_block_delta
   │                       │   { input_json_delta }   │
   │                       │                          │
   │◄── callback.onToken ──┤ (FFI bridge to main thread)
   │   "{ \"headline\": \"You spent..."                │
   │                       │                          │
   │                       │◄─ chunk: message_stop    │
   │                       ├─ parse complete JSON     │
   │                       │  validate schema         │
   │                       │                          │
   │◄── callback.onDone ───┤ InsightsOutput struct
   │   (final structured)  │                          │
```

#### Consequences

- **Positivas:** Idênticas ao ADR v1 (parse failures <0.5%, token budget rebaixa, portfolio signal forte)
- **Adicional:** isolamento de async complexity dentro do Rust core. Swift UI fica declarativo SwiftUI puro
- **Negativas:** UniFFI `[Async]` annotation precisa estar correto no UDL; spike inicial de 0.5d na Story 15 para validar `async fn` + callback interface end-to-end com mockito test

---

### ADR-8 — Eval runner: Rust nativo (REUSED de v1)

**Status:** Accepted
**Date:** 2026-05-31
**Reuse:** Decisão preservada **integralmente** do `docs/architecture/discarded/torven-v1-adr-tauri.md#adr-8`. Eval runner em Rust nativo dentro de `crates/torven-core` via `[[bin]] name="torven-evals"`.

#### Context

§6.1 do PRD pede eval pipeline com 30+ casos rotulados rodado em CI. Pivot Tauri → SwiftUI **não muda em nada** essa decisão — o eval runner é puro Rust, roda em CI Linux runner (sem necessitar macOS para evals com mock LLM), e compartilha código com o produto via re-uso direto do crate `torven-core`.

#### Decision

**Adotar Rust nativo via `[[bin]] name="torven-evals"` em `crates/torven-core`.** Detalhes técnicos (dataset JSONL, trait `LlmClient` com `RealAnthropicClient` + `MockLlmClient`, judge LLM, markdown report, CI gate >5% regressão) preservados do ADR v1.

**Reaproveitamento 100%.** O eval runner é o mesmo arquivo, mesma estrutura, mesmos prompts.

#### Consequences

Idênticas ao ADR v1.

**Bônus inesperado do pivot:** ao remover Tauri, o eval runner não puxa mais NADA de webview/UI — `cargo run --bin torven-evals` em CI Linux roda em segundos sem cross-compilation, sem webkit2gtk, sem npm. Pure Rust pipeline. Portfolio signal: ainda mais limpo do que era no ADR v1.

---

### ADR-9 (NOVO) — Xcode project structure: **XcodeGen**

**Status:** Accepted
**Date:** 2026-05-31
**New:** Decisão exclusiva do stack SwiftUI. Não existia no ADR v1 (Tauri gerencia projeto via `tauri.conf.json`).

#### Context

Com SwiftUI puro, precisamos de um Xcode project file (`.xcodeproj`) para xcodebuild gerar `.app`. Há 4 caminhos legítimos em 2026:

1. **`.xcodeproj` tradicional** — criado e editado via Xcode GUI; arquivo XML interno gerenciado por Xcode
2. **XcodeGen** — YAML `project.yml` → CLI gera `.xcodeproj`
3. **Tuist** — Swift DSL → CLI gera `.xcodeproj`
4. **Swift Package Manager only** — `Package.swift` produz executable, sem .xcodeproj

Cada opção tem trade-offs distintos em: AIOX-friendly (CLI-editable, pode ser regenerado deterministicamente), version control hygiene, ergonomia de desenvolvimento.

#### Options

**A. `.xcodeproj` tradicional editado via Xcode GUI**
- Prós:
  - Comportamento "default Apple" — toda doc/tutorial assume isso
  - Xcode GUI handle todos workflows nativamente (add file, add target, edit build settings)
- Cons:
  - **`.xcodeproj/project.pbxproj` é horror em git**: merge conflicts frequentes em equipes; mesmo single-dev, mudanças triviais (adicionar 1 arquivo) viram diff de 30 linhas em ordem aleatória
  - **AIOX-hostile**: agentes não conseguem editar via CLI determinístico — qualquer mudança exige Xcode GUI
  - Reconstrução determinística do projeto pós-clean é difícil
  - Hash interno do .pbxproj depende da ordem em que arquivos foram adicionados — não é reproducible

**B. XcodeGen (YAML → .xcodeproj)**
- **Como funciona:** `project.yml` declarativo descreve targets, dependencies, build settings, file groups. Roda `xcodegen generate` → produz `.xcodeproj`. `.xcodeproj` **fica .gitignored** (regenerado a cada checkout).
- Prós:
  - **`project.yml` é git-friendly**: diff legível, merge sem conflicts em pbxproj
  - **AIOX-friendly**: agentes editam YAML diretamente; `xcodegen generate` é determinístico
  - Maturity: YonasKolb/XcodeGen tem 8000+ stars, usado em produção por apps de larga escala (Tuist team usa para alguns projects também)
  - Setup simples: `brew install xcodegen` + `project.yml` + `xcodegen generate`
  - Não conflita com Xcode GUI — pode editar visualmente em sessões interativas (mas sempre commit muda em `project.yml`, regenera depois)
  - Schemes podem ser committed via `<project>.xcodeproj/xcshareddata/xcschemes/` parcial
- Cons:
  - Step extra no setup do dev: precisa `brew install xcodegen` antes de abrir o project
  - Algumas features Xcode bleeding-edge demoram a chegar no XcodeGen (e.g. Swift Macros support inicialmente teve gap)

**C. Tuist (Swift DSL → .xcodeproj)**
- **Como funciona:** `Project.swift` em Swift DSL define projects. CLI `tuist generate`.
- Prós:
  - DSL Swift dá power de programação (loops, conditionals, modules reusáveis)
  - Caching avançado para builds rápidos em mono-repos grandes
  - Adotada por Spotify iOS, Bumble iOS — maturity sólida
- Cons:
  - **Overkill para Torven v1.0** — single app target, sem complexidade de mono-repo iOS
  - Learning curve maior (Tuist DSL ≠ Xcode mental model — precisa "aprender Tuist")
  - Setup pesado (~50MB toolchain)
  - Updates de versão Tuist às vezes quebram projects existentes (issue conhecido)

**D. Swift Package Manager only (sem .xcodeproj)**
- Prós:
  - Zero ferramentas extras — `Package.swift` é nativo Apple
  - Build via `swift build` puro
- Cons:
  - **Limitação fatal**: SwiftPM tem issues conhecidos para shipável .app macOS:
    - Não suporta resource bundling para .app (ícones, .car catalogs) elegantemente
    - Code signing via SwiftPM é manual hell — Apple não automatiza
    - Sparkle 2 integration via SwiftPM funciona, mas Info.plist embed exige workarounds
    - Notarytool integration via SwiftPM é manual scripting
  - Apple **não posiciona SwiftPM como replacement de Xcode para .app shipping** (apenas para libraries e CLI tools)

#### Decision

**Adotar Opção B — XcodeGen.**

**Racional rigoroso** (segunda decisão de maior impacto do ADR — extra rigor):

1. **AIOX compatibility é decisiva.** O projeto roda no contexto AIOX onde agentes (`@dev`, `@architect`, etc.) precisam editar arquivos de config via CLI. `.xcodeproj/project.pbxproj` é hostile a isso por design — não foi feito para edição via texto. XcodeGen converte o problema em "editar YAML", que é território confortável para qualquer agente. SwiftUI sem AIOX-friendly seria contradição com o resto do projeto.

2. **Version control hygiene.** Em single-dev, pbxproj merge conflicts não acontecem (sem equipe), mas **reproducibility** importa: "clonar repo + xcodebuild" funciona em uma máquina nova sem precisar do .pbxproj exato. Com XcodeGen, `git clone && xcodegen generate && xcodebuild` é determinístico. Com `.xcodeproj` traditional, depende do estado interno do arquivo XML que pode drift.

3. **Tuist é overkill.** Torven v1.0 é 1 app target + 1 test target. Não há módulos compartilhados, não há build matrix complexa de iOS+macOS+tvOS. Tuist DSL é poderoso, mas usar canhão para mosquito. XcodeGen YAML resolve com 30 linhas.

4. **SwiftPM-only é fatal** (cons acima — não pode ship .app proper).

5. **Reconstrução determinística do projeto pós-corrupção** é signal de senioridade. Em entrevista: "se meu .xcodeproj corromper, eu rodo `xcodegen generate` e tudo volta". Em apresentação de portfolio, **é evidence de engineering discipline**, não de gambiarra.

**Implementação detalhada:**

- **Setup:**
  - `brew install xcodegen` (one-time per dev machine)
  - `apple/project.yml` (committed) define o project
  - `apple/Torven.xcodeproj/` (gitignored — regenerated)
  - `apple/Torven.xcodeproj/xcshareddata/xcschemes/Torven.xcscheme` (committed — schemes config)
- **`apple/project.yml` skeleton (~30 linhas):**
  ```yaml
  name: Torven
  options:
    bundleIdPrefix: com.torven
    deploymentTarget:
      macOS: "13.0"
  settings:
    base:
      MARKETING_VERSION: "1.0.0"
      CURRENT_PROJECT_VERSION: "1"
      DEVELOPMENT_TEAM: "${DEVELOPMENT_TEAM}"  # from env var
      CODE_SIGN_STYLE: Automatic
  packages:
    Sparkle:
      url: https://github.com/sparkle-project/Sparkle
      from: "2.6.0"
  targets:
    Torven:
      type: application
      platform: macOS
      sources:
        - path: Torven
      resources:
        - path: Torven/Assets.xcassets
      dependencies:
        - package: Sparkle
        - framework: Frameworks/TorvenCore.xcframework
          embed: false
      info:
        path: Torven/Info.plist
        properties:
          LSUIElement: true  # menu bar app, no Dock icon
          SUFeedURL: "https://github.com/lorenzomatheuss/torven/releases/latest/download/appcast.xml"
          SUPublicEDKey: "${SPARKLE_PUBLIC_KEY}"
      entitlements:
        path: Torven/Torven.entitlements
        properties:
          com.apple.security.network.client: true
    TorvenTests:
      type: bundle.unit-test
      platform: macOS
      sources:
        - path: TorvenTests
      dependencies:
        - target: Torven
  ```
- **Build orchestration via Makefile:**
  ```makefile
  build-core:
      cargo build --release --target aarch64-apple-darwin -p torven-core
      cargo build --release --target x86_64-apple-darwin -p torven-core
      ./apple/scripts/build-xcframework.sh

  build-app: build-core
      cd apple && xcodegen generate
      cd apple && xcodebuild -scheme Torven -configuration Release

  build-all: build-app
  ```
- **CI workflow** (Story 23 — release pipeline) roda `make build-all` direto.

#### Consequences

- **Positivas:**
  - `apple/project.yml` é editável por agentes AIOX (`@dev`, `@architect`) sem precisar abrir Xcode
  - Diff em PRs é legível (YAML, não pbxproj)
  - Reprodutibilidade total: `make build-app` funciona em qualquer máquina macOS com toolchain configurada
  - Setup time pós-clone: `brew install xcodegen && make build-all` ~ 10 min
- **Negativas:**
  - Adiciona dependência `xcodegen` no toolchain. Mitigação: documentado em README + CLAUDE.md release checklist + `apple/scripts/setup.sh` instala automaticamente
  - Algumas configurações Xcode bleeding-edge (Swift Macros, novos build settings WWDC 2026) podem demorar a chegar em XcodeGen. Mitigação: hard-coded build settings via `settings.base` no project.yml funciona como escape hatch
- **Schemes hygiene:** `Torven.xcscheme` é committed em `apple/Torven.xcodeproj/xcshareddata/xcschemes/` MESMO com .xcodeproj gitignored — XcodeGen respeita scheme files existentes (não sobrescreve)

---

### ADR-10 (NOVO) — App entry point: **SwiftUI `@main App` protocol** (com `NSApplicationDelegateAdaptor`)

**Status:** Accepted
**Date:** 2026-05-31
**New:** Decisão exclusiva do stack SwiftUI.

#### Context

Em 2026, SwiftUI app entry point tem dois caminhos:

- **SwiftUI `@main App` protocol** (introduzido WWDC 2020, maturado WWDC 2022+) — declarative, modern
- **`NSApplicationDelegate` traditional** (legacy AppKit pattern) — imperative, more control

Mas a realidade é **híbrido**: o pattern moderno é `@main App` com `@NSApplicationDelegateAdaptor` interno para hooks de lifecycle que SwiftUI ainda não expõe (e.g. `applicationDidFinishLaunching` granular, Sparkle delegate, URL scheme handling). MenuBarExtra é a feature SwiftUI-native que **força** uso de `@main App`.

Considerações específicas do Torven:

- **MenuBarExtra requirements:** MenuBarExtra (introduzido macOS 13.0 = nosso target NFR-4) é SwiftUI-native menu bar item. Requer `@main App` — não funciona puro com `NSApplicationDelegate`
- **Sparkle init timing:** `SPUStandardUpdaterController` precisa ser propriedade do App / Delegate para sobreviver lifecycle
- **Deep links / URL schemes futuros:** v1.5 pode ter deep links (e.g. `torven://insights/run`). SwiftUI `onOpenURL` é declarativo no App
- **LSUIElement = true** (`Info.plist`): app sem Dock icon (only menu bar). Vide ADR-9 project.yml

#### Options

**A. Pure `@main App` SwiftUI** — sem NSApplicationDelegate
- Prós:
  - Modern, declarativo
  - MenuBarExtra funciona nativamente
- Cons:
  - **NSStatusItem custom view positioning bugs** — Apple tem histórico documentado de bugs com MenuBarExtra em edge cases (e.g. window appearing in wrong screen with external monitor). Workarounds exigem AppKit access
  - Sem hook claro para Sparkle `userDriverDelegate` setup (Sparkle 2 funciona, mas tem gap em SwiftUI-only)
  - Deep link handling (`onOpenURL`) funciona, mas URL scheme registration via Info.plist ainda exige declaration manual

**B. `NSApplicationDelegate` puro + AppKit `NSStatusItem`**
- Prós:
  - Control total — MenuBarExtra bugs ficam contornáveis
  - Sparkle integration tem todos os hooks
  - Performance teto melhor (sem SwiftUI overhead)
- Cons:
  - **AppKit imperative dissolve productivity SwiftUI** — escrever popover/janela em AppKit em 2026 é regredir 10 anos
  - **Não pode usar MenuBarExtra** (SwiftUI-only feature)
  - Boilerplate massivo

**C. `@main App` + `@NSApplicationDelegateAdaptor`** — híbrido moderno
- **Como funciona:**
  ```swift
  @main
  struct TorvenApp: App {
      @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

      var body: some Scene {
          MenuBarExtra("Torven", systemImage: "chart.bar.fill") {
              MenuBarContent()
          }
          .menuBarExtraStyle(.window)  // popover-style

          Window("Torven", id: "main") {
              MainWindowView()
          }
      }
  }

  class AppDelegate: NSObject, NSApplicationDelegate {
      func applicationDidFinishLaunching(_ notification: Notification) {
          // Sparkle init, Keychain prewarm, logging setup
      }
      func applicationWillTerminate(_ notification: Notification) {
          // graceful shutdown
      }
  }
  ```
- Prós:
  - **Melhor dos dois mundos**: SwiftUI declarativo + AppKit escape hatch quando necessário
  - MenuBarExtra funciona (SwiftUI-native)
  - Sparkle `userDriverDelegate`, deep links, lifecycle hooks tudo no AppDelegate
  - Future-proof: adicionar features (URL schemes, push notifications) sem refactor
- Cons:
  - Dois patterns conviventes — pequena curva de "onde vai cada coisa". Mitigação: convenção clara — UI = SwiftUI, lifecycle/system = AppDelegate

#### Decision

**Adotar Opção C — `@main App` + `@NSApplicationDelegateAdaptor`.**

Racional:

1. **MenuBarExtra é mandatory** (FR-8 PRD). Isso elimina Opção B.
2. **Sparkle 2 integration limpa** exige AppDelegate hooks. Isso elimina Opção A (puro).
3. **Pattern é canônico no segmento 2026**: ClaudeBar, Tokens 4 Breakfast, e essencialmente todo app moderno macOS menu bar usa exatamente esse padrão híbrido.

**Implementação detalhada:**

- **`apple/Torven/TorvenApp.swift`** (entry point):
  ```swift
  import SwiftUI
  import Sparkle

  @main
  struct TorvenApp: App {
      @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate
      @StateObject private var coreBridge = TorvenCoreBridge()

      var body: some Scene {
          MenuBarExtra {
              PopoverView()
                  .environmentObject(coreBridge)
                  .frame(width: 360, height: 480)
          } label: {
              MenuBarLabel(snapshot: coreBridge.menuBarSnapshot)
          }
          .menuBarExtraStyle(.window)

          Window("Torven", id: "main") {
              MainWindowView()
                  .environmentObject(coreBridge)
                  .frame(minWidth: 800, minHeight: 600)
          }
          .windowResizability(.contentMinSize)
          .commands {
              CommandGroup(after: .appInfo) {
                  Button("Check for Updates...") {
                      appDelegate.updaterController.updater.checkForUpdates()
                  }
              }
          }
      }
  }

  final class AppDelegate: NSObject, NSApplicationDelegate {
      let updaterController = SPUStandardUpdaterController(
          startingUpdater: true,
          updaterDelegate: nil,
          userDriverDelegate: nil
      )

      func applicationDidFinishLaunching(_ notification: Notification) {
          // 1. Initialize Rust core (lazy via TorvenCoreBridge)
          // 2. Request notification permission (FR-11 budget alerts)
          // 3. Setup logging
      }

      func applicationWillTerminate(_ notification: Notification) {
          // Graceful shutdown: flush SQLite, drop tokio runtime
          TorvenCoreBridge.shared.shutdown()
      }
  }
  ```
- **`apple/Torven/Info.plist`** (via project.yml `info.properties`):
  - `LSUIElement = YES` (menu bar app sem Dock icon)
  - `LSApplicationCategoryType = public.app-category.developer-tools`
  - `NSHumanReadableCopyright` = `© 2026 Lorenzo Matheuss. MIT licensed.`

#### Consequences

- **Positivas:**
  - Pattern canônico do segmento — qualquer dev macOS pickup do code é familiar
  - Future-proof para deep links (`AppDelegate.application(_:open:)`), push notifications, services
  - Menu commands (`commands { ... }`) declarativos com keyboard shortcuts (Cmd+R, Cmd+,, Cmd+I — FRs 8/9)
- **Negativas:**
  - **NSStatusItem custom view positioning** (AR-7 do ADR v1, agora vira AR-8 deste ADR — vide §7): MenuBarExtra com `.window` style tem histórico de bugs em multi-monitor (window appearing on wrong screen). Mitigação: spike de 0.5d na Story 8 (NSStatusItem + label rendering); fallback documentado é usar `NSStatusItem` direto via AppKit bridge dentro do AppDelegate

---

## 4. Cleanup Plan

Lista concreta de deleções, migrações e renames pós-pivot. Base: `docs/architecture/preservation-map.md` + repo state atual + cleanup decisions deste ADR v2.

### DELETE

| Arquivo / Pasta | Razão | Impacto em tests/builds |
|---|---|---|
| `src/waybar.rs` | WaybarOutput JSON + SIGRTMIN signaling — Waybar não existe em macOS | Remove imports em `src/lib.rs`. Tests: nenhum direto. |
| `src/pango.rs` | Pango markup helpers — substituído por SwiftUI views nativas | Idem. Snapshot tests Pango-specific deletados. |
| `src/tooltip.rs` | Pango bordered-box tooltip — substituído por SwiftUI popover | Snapshot tests de tooltip rendering deletados. |
| `src/theme.rs` | Detecção Omarchy — não existe em macOS; system appearance vem do macOS (light/dark via SwiftUI `@Environment(\.colorScheme)`) | Remove imports. Tests: deletar se existir. |
| `src/active.rs` | Scroll-cycle do Waybar — sem scroll na menu bar macOS | Remove imports. State migra para SwiftUI `@AppStorage` se preciso (improvável). |
| `src/widget/mod.rs`, `src/widget/cli.rs`, `src/widget/pretty.rs`, `src/widget/render.rs`, `src/widget/run.rs` | Shell completo do Waybar widget | Remove `pub mod widget` em `lib.rs`. |
| `src/bin/torven.rs` | Entry point widget Waybar | Substituído por Xcode-built `Torven.app`. |
| `packaging/aur/` (toda) | AUR é Arch Linux only | Stories de release v0.x removem menções. |
| `.github/workflows/release.yml` | Builds Linux x86_64/aarch64 | **REWRITE** (não delete) — vide MIGRATE. |
| `config.example.toml` (formato atual Waybar-oriented) | Schema legacy | Story de migração gera novo `config.example.toml` com `accounts`. |

### MIGRATE

| Arquivo | O que muda | Impacto |
|---|---|---|
| `src/format.rs` | **Decisão crítica:** avaliada se a lógica Pango tem valor sem Waybar — **CONCLUSÃO: NÃO PRESERVAR como está**. O `FormattedSnapshot { color_kind: ColorKind, label_text: String, ... }` proposto no ADR v1 fazia sentido para React consumir. Em SwiftUI, **as cores e formatação devem viver no Swift side** (via `Color.calm`, `Color.amber`, `Color.critical` em SwiftUI assets catalog), não no Rust. Migrar `format.rs` para retornar apenas `RawMetrics { cost_usd, pct_used, tokens_used, label_kind: LabelKind }` onde `LabelKind` é enum semântico (`PercentOfWindow`, `MessagesQuota`, `UsdSpent`). Swift formats final string e cor. **TUI mantém helper de formatação separado** dentro de `crates/torven-tui/src/format_tui.rs`. | Tests do format simplificados. TUI tem helper próprio. |
| `src/lib.rs` | Remove `pub mod {pango,theme,tooltip,waybar,widget,active}`. **Adiciona `pub mod uniffi_exports`** (módulo dedicado para `#[uniffi::export]` annotations ou re-exports consumidos pelo .udl). Adiciona `pub mod insights`, `pub mod history`, `pub mod keychain`. Move para `crates/torven-core/src/lib.rs`. | Imports nos vendor modules continuam. |
| `src/config.rs` | Adicionar `accounts: HashMap<VendorId, Vec<Account>>` (FR-6 PRD). Adicionar `ai_insights: AiInsightsConfig`, `history: HistoryConfig`. **NÃO** precisa `to_json()` (não há React) — UniFFI gera struct mapping automático. Mantém TOML parsing + `toml_edit` para Settings UI. | Schema TOML evolui (migration code mantém backward compat). |
| `src/anthropic/creds.rs` e similares (`openai/creds.rs`, `gemini/creds.rs`) | Default path muda de `~/.config/...` (XDG) para `~/Library/Application Support/Torven/{vendor}.credentials.json` (macOS convention). API keys migram para Keychain blob (vide ADR-2) via story de migration. | Helper `default_creds_path()` retorna macOS path. |
| `src/tui/*` | Move para `crates/torven-tui/src/`. Re-aponta imports para `torven_core::*`. Mantém formatação própria com `format_tui.rs` (vide acima). | TUI continua funcional. |
| `src/{anthropic,openai,gemini,zai,openrouter}/*` | Move para `crates/torven-core/src/vendors/{vendor}/`. Re-aponta imports. | Zero mudança funcional. `make smoke` continua. |
| `src/{cache,countdown,error,pacing,usage,vendor}.rs` | Move para `crates/torven-core/src/`. | Zero mudança funcional. |
| `tests/anthropic_e2e.rs` | Move para `crates/torven-core/tests/`. | Mockito-based, funcional. |
| `tests/live.rs` | Move para workspace-level. | `make smoke` continua. |
| `.github/workflows/release.yml` | **REWRITE COMPLETO**: build matrix `[macos-14]` (Apple Silicon runner) + cross-compile x86_64; build Rust core via `cargo build --target {aarch64,x86_64}-apple-darwin --release -p torven-core`; `lipo -create` para universal `.a`; `xcodebuild -create-xcframework` para `.xcframework`; `xcodegen generate` no `apple/`; `xcodebuild -scheme Torven -configuration Release` para .app; `codesign` + `notarytool submit --wait`; `stapler staple Torven.app`; gera .dmg; `generate_appcast` Sparkle EdDSA sign; publica .dmg + appcast.xml em GH Release. | Story de migração de release pipeline (Story 23). |
| `CLAUDE.md` (release checklist) | Atualizar de v0.x (cargo build + AUR pin) para v1.0 (cargo build workspace core + xcframework + xcodebuild + notarize + Sparkle appcast). | Story 25. |
| `README.md` | Rewrite completo — narrativa AI Insights primária, screenshots SwiftUI, eval metrics table, install via .dmg, "Built with SwiftUI + Rust" badge prominent. | Story 25. |
| `CHANGELOG.md` | Marca v1.0 como pivot completo macOS SwiftUI+Rust. Mantém histórico Waybar como predecessor. | Story 25. |
| `Makefile` | Reescreve targets: `build-core`, `build-app`, `build-all`, `universal`, `clean`, `smoke`, `evals`. Remove targets Linux. | Story 25. |

### NEW (Rust side — crates/torven-core/)

| Path | Função |
|---|---|
| `crates/torven-core/Cargo.toml` | Lib (`crate-type = ["staticlib", "rlib", "cdylib"]`) + `[[bin]] name="torven-evals"`. Deps: `uniffi`, `security-framework`, `rusqlite`, `rusqlite_migration` |
| `crates/torven-core/src/torven_core.udl` | UDL declarativa da FFI surface (vide ADR-4) |
| `crates/torven-core/src/uniffi_exports.rs` | Module dedicado para `#[uniffi::export]` + setup `uniffi::setup_scaffolding!()` |
| `crates/torven-core/build.rs` | `uniffi::generate_scaffolding("src/torven_core.udl").unwrap();` |
| `crates/torven-core/src/insights/` | Cliente Anthropic Messages API com tool_use, streaming, eval-instrumented (vide ADR-7) |
| `crates/torven-core/src/insights/llm_client.rs` | Trait `LlmClient` + `RealAnthropicClient` + `MockLlmClient` |
| `crates/torven-core/src/history/` | SQLite via rusqlite, migrations (vide ADR-5) |
| `crates/torven-core/src/keychain/` | macOS Keychain via `security-framework` (vide ADR-2) |
| `crates/torven-core/src/runtime.rs` | `OnceCell<tokio::runtime::Runtime>` shared para async FFI calls |
| `crates/torven-core/src/bin/torven-evals.rs` | Binário eval runner (vide ADR-8) |
| `crates/torven-core/evals/dataset.jsonl` | Dataset rotulado 30+ casos v1.0 |
| `crates/torven-core/evals/schema.md` | Schema do dataset doc |
| `crates/torven-core/include/torven_core.h` | C header (gerado pelo build script) consumido pelo xcframework |

### NEW (Apple side — apple/)

| Path | Função |
|---|---|
| `apple/project.yml` | XcodeGen config (vide ADR-9) |
| `apple/Torven.xcodeproj/` | **gitignored** — regenerated by `xcodegen generate` |
| `apple/Torven.xcodeproj/xcshareddata/xcschemes/Torven.xcscheme` | **committed** — scheme config |
| `apple/Torven/TorvenApp.swift` | `@main App` entry point (vide ADR-10) |
| `apple/Torven/AppDelegate.swift` | NSApplicationDelegate hooks (Sparkle, lifecycle) |
| `apple/Torven/Info.plist` | Bundle config |
| `apple/Torven/Torven.entitlements` | Sandbox + network entitlements |
| `apple/Torven/Assets.xcassets/` | Icons, accent colors, semantic Color assets (Calm/Amber/Critical) |
| `apple/Torven/MenuBar/MenuBarLabel.swift` | NSStatusItem custom view content (dynamic label) |
| `apple/Torven/MenuBar/MenuBarContent.swift` | MenuBarExtra content wrapper |
| `apple/Torven/Popover/PopoverView.swift` | Main popover view (header + 5 vendor cards) |
| `apple/Torven/Popover/VendorCard.swift` | Card per vendor with metric + sparkline |
| `apple/Torven/Popover/AccountPicker.swift` | Account picker (shape TBD by UX) |
| `apple/Torven/Window/MainWindowView.swift` | Detailed window with sidebar |
| `apple/Torven/Window/HistoryChart.swift` | Swift Charts integration (vide ADR-6) |
| `apple/Torven/Window/AccountsList.swift` | Per-vendor accounts management |
| `apple/Torven/Settings/SettingsView.swift` | Settings UI (config.toml editor via FFI) |
| `apple/Torven/Insights/InsightsPanel.swift` | AI Insights UI + streaming consumption |
| `apple/Torven/Insights/InsightCard.swift` | Single insight display |
| `apple/Torven/Insights/InsightsStream.swift` | Streaming text accumulator |
| `apple/Torven/ViewModels/CoreBridge.swift` | `TorvenCoreBridge: ObservableObject` — holds reference to Rust core, exposes `@Published` state |
| `apple/Torven/ViewModels/VendorViewModel.swift` | Per-vendor reactive state |
| `apple/Torven/ViewModels/InsightsViewModel.swift` | Insights streaming state |
| `apple/Torven/Bridge/Generated/torven_core.swift` | **gitignored** — generated by `uniffi-bindgen` |
| `apple/Torven/Bridge/Generated/torven_coreFFI.h` | **gitignored** — generated |
| `apple/Torven/Bridge/Generated/torven_coreFFI.modulemap` | **gitignored** — generated |
| `apple/Torven/Bridge/CoreBridge+Async.swift` | Hand-written Swift extensions wrapping FFI in idiomatic Swift |
| `apple/TorvenTests/` | XCTest target |
| `apple/Frameworks/TorvenCore.xcframework/` | **gitignored** — built by `apple/scripts/build-xcframework.sh` |
| `apple/Sparkle/` | Sparkle EdDSA public key (committed) — private key in CI secret |
| `apple/Sparkle/README.md` | EdDSA key management runbook |
| `apple/scripts/build-xcframework.sh` | Universal binary + xcframework build |
| `apple/scripts/setup.sh` | One-time dev machine setup (`brew install xcodegen` + deps) |
| `prompts/insights.v1.md` | Primeiro prompt versionado |
| `prompts/insights.v1.schema.json` | Schema JSON do tool_use |
| `prompts/judge.v1.md` | Judge LLM prompt |
| `prompts/CHANGELOG.md` | Histórico de versões prompt |
| `Cargo.toml` (root) | Workspace manifest |
| `.github/workflows/ci.yml` | NEW — cargo test workspace + evals (mocked) em ubuntu-latest |
| `.github/workflows/eval-gate.yml` | NEW — PR gate em mudanças `prompts/` ou `insights/` |

---

## 5. Workspace Skeleton Final

```
torven/                                # repo root
├── Cargo.toml                         # workspace manifest (members = ["crates/*"])
├── Cargo.lock
├── rust-toolchain.toml                # pin rustc version
├── README.md                          # rewrite v1.0 (SwiftUI + Rust narrative)
├── CHANGELOG.md
├── CLAUDE.md                          # updated release checklist
├── Makefile                           # macOS targets only
├── .gitignore                         # adds: apple/Torven.xcodeproj/, apple/Frameworks/, apple/Torven/Bridge/Generated/
├── config.example.toml                # new schema with accounts
│
├── crates/
│   ├── torven-core/                   # ★ Rust LLM core lib + eval bin
│   │   ├── Cargo.toml                 # crate-type = ["staticlib", "rlib"] + [[bin]] torven-evals
│   │   ├── build.rs                   # uniffi::generate_scaffolding
│   │   ├── src/
│   │   │   ├── lib.rs                 # re-exports + uniffi setup
│   │   │   ├── torven_core.udl        # FFI surface declaration
│   │   │   ├── uniffi_exports.rs      # #[uniffi::export] annotations
│   │   │   ├── runtime.rs             # OnceCell<tokio::Runtime>
│   │   │   ├── config.rs              # MIGRATED (TOML + Accounts)
│   │   │   ├── cache.rs               # PRESERVED
│   │   │   ├── countdown.rs           # PRESERVED
│   │   │   ├── error.rs               # PRESERVED
│   │   │   ├── pacing.rs              # PRESERVED
│   │   │   ├── usage.rs               # PRESERVED (VendorSnapshot, raw metrics)
│   │   │   ├── vendor.rs              # PRESERVED (VendorId enum)
│   │   │   ├── format.rs              # SIMPLIFIED (RawMetrics, not Pango)
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
│   │   │   ├── keychain/              # NEW (macOS Keychain via security-framework)
│   │   │   │   ├── mod.rs             # trait SecretStore
│   │   │   │   ├── mac.rs             # MacKeychainStore impl
│   │   │   │   └── fallback.rs        # FileFallbackStore (TUI Linux)
│   │   │   └── bin/
│   │   │       └── torven-evals.rs    # NEW eval runner binary
│   │   ├── include/
│   │   │   └── torven_core.h          # C header (generated, committed)
│   │   ├── evals/
│   │   │   ├── dataset.jsonl
│   │   │   ├── schema.md
│   │   │   └── results/               # gitignored
│   │   └── tests/
│   │       ├── anthropic_e2e.rs       # MOVED from tests/
│   │       ├── insights_e2e.rs        # NEW
│   │       ├── history_e2e.rs        # NEW
│   │       └── keychain_e2e.rs       # NEW
│   │
│   └── torven-tui/                    # PRESERVED: ratatui TUI dev tool / Linux fallback
│       ├── Cargo.toml                 # depends on torven-core (rlib)
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── panels.rs
│           ├── settings.rs
│           ├── view.rs
│           └── format_tui.rs          # TUI-specific formatting (Pango helpers moved here)
│
├── apple/                             # ★ SwiftUI app + Xcode project
│   ├── project.yml                    # XcodeGen config (vide ADR-9)
│   ├── Torven.xcodeproj/              # GITIGNORED (regenerated via xcodegen generate)
│   │   └── xcshareddata/
│   │       └── xcschemes/
│   │           └── Torven.xcscheme    # COMMITTED
│   ├── Torven/                        # App source (Swift)
│   │   ├── TorvenApp.swift            # @main App (vide ADR-10)
│   │   ├── AppDelegate.swift          # NSApplicationDelegate hooks
│   │   ├── Info.plist
│   │   ├── Torven.entitlements
│   │   ├── Assets.xcassets/
│   │   │   ├── AppIcon.appiconset/
│   │   │   ├── AccentColor.colorset/
│   │   │   └── Semantic/              # Calm/Amber/Critical color sets
│   │   ├── MenuBar/
│   │   │   ├── MenuBarLabel.swift
│   │   │   └── MenuBarContent.swift
│   │   ├── Popover/
│   │   │   ├── PopoverView.swift
│   │   │   ├── PopoverHeader.swift    # Insights/Refresh/Settings buttons
│   │   │   ├── VendorCard.swift
│   │   │   ├── AccountPicker.swift
│   │   │   └── InsightsButton.swift
│   │   ├── Window/
│   │   │   ├── MainWindowView.swift
│   │   │   ├── SidebarView.swift
│   │   │   ├── VendorDetailView.swift
│   │   │   ├── HistoryChart.swift     # Swift Charts (vide ADR-6)
│   │   │   ├── AccountsListView.swift
│   │   │   └── InsightsHistoryView.swift
│   │   ├── Settings/
│   │   │   ├── SettingsView.swift     # macOS Settings scene
│   │   │   ├── VendorSettingsTab.swift
│   │   │   ├── AccountsSettingsTab.swift
│   │   │   ├── AIInsightsSettingsTab.swift
│   │   │   └── DataSettingsTab.swift  # retention, export
│   │   ├── Insights/
│   │   │   ├── InsightsPanel.swift
│   │   │   ├── InsightsStream.swift
│   │   │   ├── InsightCard.swift
│   │   │   └── InsightsHistoryList.swift
│   │   ├── ViewModels/
│   │   │   ├── CoreBridge.swift       # TorvenCoreBridge: ObservableObject
│   │   │   ├── VendorViewModel.swift
│   │   │   ├── InsightsViewModel.swift
│   │   │   └── SettingsViewModel.swift
│   │   ├── Bridge/                    # Swift wrappers around FFI
│   │   │   ├── Generated/             # GITIGNORED (uniffi-bindgen output)
│   │   │   │   ├── torven_core.swift
│   │   │   │   ├── torven_coreFFI.h
│   │   │   │   └── torven_coreFFI.modulemap
│   │   │   ├── CoreBridge+Async.swift # idiomatic Swift wrappers
│   │   │   └── Logger.swift           # bridges Rust tracing to os_log
│   │   └── Onboarding/
│   │       └── OnboardingWizard.swift
│   ├── TorvenTests/                   # XCTest target
│   │   ├── PopoverTests.swift
│   │   ├── CoreBridgeTests.swift
│   │   └── InsightsTests.swift
│   ├── Frameworks/                    # GITIGNORED
│   │   └── TorvenCore.xcframework/    # built by apple/scripts/build-xcframework.sh
│   ├── Sparkle/
│   │   ├── README.md                  # EdDSA key management runbook
│   │   └── public_key.txt             # COMMITTED (public only)
│   └── scripts/
│       ├── build-xcframework.sh       # universal binary + xcframework
│       ├── setup.sh                   # one-time dev setup
│       └── notarize.sh                # CI notarytool wrapper
│
├── prompts/                           # AI Insights prompts (versioned)
│   ├── insights.v1.md
│   ├── insights.v1.schema.json
│   ├── judge.v1.md
│   └── CHANGELOG.md
│
├── docs/
│   ├── prd/torven-v1.md
│   ├── architecture/
│   │   ├── preservation-map.md
│   │   ├── torven-v1-adr.md           # ★ this file (Active)
│   │   └── discarded/
│   │       └── torven-v1-adr-tauri.md # historical
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
        ├── ci.yml                     # cargo test workspace + evals (mocked) on ubuntu
        ├── release.yml                # tag-driven: build core + xcframework + xcodebuild + notarize + Sparkle appcast
        └── eval-gate.yml              # PR gate: run evals if prompts/* or insights/* changed
```

### ASCII data flow — popover open path (SwiftUI + Rust FFI)

```
User clicks NSStatusItem (menu bar)
     ↓
MenuBarExtra (.window style)  ── opens popover
     ↓
SwiftUI PopoverView mounts
     ↓
PopoverView accesses @EnvironmentObject coreBridge: TorvenCoreBridge
     ↓
coreBridge.snapshots already populated (initial loaded on app launch)
     ↓ (if stale, triggers refresh)
coreBridge.triggerRefresh()
     ↓ (UniFFI async FFI call)
       try await TorvenCore.triggerRefresh()
                          ↓
                       Rust: tokio runtime spawns vendor fetches (parallel)
                          ↓
                       vendors::{anthropic, openai, ...}::fetch
                          ↓
                       writes UsageSnapshot to SQLite (history.db)
                          ↓
                       returns Vec<VendorSnapshot>
                          ↓ (FFI marshal)
     ← Swift receives [VendorSnapshot] (idiomatic Array)
     ↓
coreBridge.updateSnapshots(received)
     ↓ (@Published change)
SwiftUI re-renders VendorCard × 5 reactively
     ↓
Background: TorvenCore scheduler emits via callback every 30s
     ↓
coreBridge.onSnapshotUpdate(vendor, snapshot) callback
     ↓
@Published mutation → SwiftUI re-renders
```

### ASCII data flow — AI Insights streaming

```
User clicks "Insights" button (Cmd+I) in popover
     ↓
InsightsPanel.requestInsights()
     ↓
InsightsViewModel.start()
     ↓
try await coreBridge.requestInsights(context, callback: self)
     ↓ (UniFFI [Async] + callback interface)
Rust: insights::client::run_streaming(context, callback)
     ↓
HTTP POST Anthropic Messages API (stream: true, tool_choice: submit_insight)
     ↓
Anthropic streams content_block_delta { input_json_delta: "{ \"headline\":" } ...
     ↓ (per chunk)
     callback.onToken("{ \"headline\":") ── via FFI ──► InsightsViewModel.onToken
                                                            ↓
                                                       @Published accumulatedText updated
                                                            ↓
                                                       SwiftUI re-renders InsightsStream view
     ↓
... continues streaming ...
     ↓
Anthropic sends message_stop
     ↓
Rust parses complete JSON, validates schema
     ↓
callback.onDone(InsightsOutput { headline, insights, recommendation, cost_usd })
                          ── via FFI ──► InsightsViewModel.onDone
                                              ↓
                                         @Published structuredOutput updated
                                              ↓
                                         SwiftUI re-renders InsightCard with final
                                              ↓
                                         Persist to insights_history (FFI call)
```

---

## 6. Migration Sequence

Stories ordenadas que o @sm vai transformar em arquivos `docs/stories/`. Cada story em 1 linha + dependências + paralelizabilidade. Mantenho granularidade ≤1d (R-5 do PRD).

**Notação de paralelizabilidade:**
- `[PAR]` — pode rodar em paralelo com sua dependência mais próxima depois que ela completa
- `[SEQ]` — deve esperar dependência completar antes de iniciar
- `[INDEP]` — totalmente independente (pode começar imediatamente, sem dependência)

| # | Story | Depende de | Modo | Output |
|---|---|---|---|---|
| 1 | **Bootstrap Cargo workspace** — root `Cargo.toml` workspace, criar `crates/torven-core/`, `crates/torven-tui/`. Mover `src/` → `crates/torven-core/src/`. Validar `cargo build --workspace` verde. Setup `crate-type = ["staticlib", "rlib", "cdylib"]` no torven-core. | — | [INDEP] | Workspace funcional, TUI buildable, core lib buildable como staticlib. |
| 2 | **Setup UniFFI binding tool** — adicionar `uniffi = "0.27"` em deps, criar `src/torven_core.udl` skeleton (1 função dummy `ping() -> string`), `build.rs` com `uniffi::generate_scaffolding`, scripts/build-xcframework.sh placeholder. Validar `uniffi-bindgen generate` produz Swift output. | 1 | [SEQ] | UniFFI pipeline established. |
| 3 | **Bootstrap Xcode project via XcodeGen** — `brew install xcodegen` doc, criar `apple/project.yml`, `apple/Torven/TorvenApp.swift` skeleton com hello world MenuBarExtra. Validar `xcodegen generate && xcodebuild` produz `.app`. | 2 | [SEQ] | Xcode project funcional, hello world rodando. |
| 4 | **Cleanup Linux-coupled code** — deletar `waybar.rs`, `pango.rs`, `tooltip.rs`, `theme.rs`, `active.rs`, `widget/`, `bin/torven.rs`, `packaging/aur/`. Atualizar `lib.rs`. Confirmar `cargo build --workspace -p torven-core` verde. | 1 | [PAR com 2/3] | Core lib limpo. |
| 5 | **Migrate format.rs to RawMetrics** — substituir Pango strings por `RawMetrics { cost_usd, pct_used, tokens_used, label_kind }`. TUI ganha `format_tui.rs` próprio para Pango/ratatui formatting. | 4 | [SEQ] | format.rs cross-platform, TUI funcional. |
| 6 | **Migrate config.rs schema** — adicionar `accounts: HashMap<VendorId, Vec<Account>>`, `ai_insights`, `history`. Backward-compat migration. | 5 | [SEQ] | Novo schema TOML. |
| 7 | **First FFI bridge — get_vendor_list()** — expor função simples `get_vendor_list() -> Vec<VendorInfo>` via UDL. Consumir em Swift `TorvenCoreBridge.swift`. Validar SwiftUI list renderiza vendor names. | 3, 6 | [SEQ] | FFI end-to-end estabelecido. |
| 8 | **NSStatusBar tray + dynamic label** — MenuBarExtra label dinâmico (lê de Rust core via FFI). MenuBarContent placeholder. Spike de 0.5d para edge cases multi-monitor (AR-8). | 7 | [SEQ] | Menu bar visível com label. |
| 9 | **FFI: fetch_snapshot + get_all_snapshots** — expor `fetch_snapshot(vendor, account?) -> VendorSnapshot` e `get_all_snapshots() -> Vec<VendorSnapshot>` no UDL. Validar Swift recebe dados reais. | 7 | [SEQ] | Core data flow viável. |
| 10 | **SwiftUI Popover skeleton** — `PopoverView` com header (Insights/Refresh/Settings buttons) + 5 VendorCard placeholders. Sem multi-account yet. Cmd+1..5 navigation. | 8, 9 | [SEQ] | Popover funcional single-account. |
| 11 | **Vendor card — first vendor (OpenRouter)** — implementar OpenRouter VendorCard com cost/budget display. API key inicialmente lida de config (Keychain vem em story 13). | 10 | [SEQ] | OpenRouter mostrado no popover. |
| 12 | **Extend cards to all 5 vendors** — Anthropic (Claude Max OAuth), OpenAI (Codex OAuth), Gemini (OAuth), Z.AI (API key), OpenRouter já feito. Validar todos renderizam corretamente. | 11 | [SEQ] | 5 vendors funcionais. |
| 13 | **Keychain integration (per-vendor blob)** — implementar `crates/torven-core/src/keychain/mac.rs` via `security-framework`. **Spike 0.5d primeiro** (AR-3). Migration helper from `~/.config/...` files. FFI: `keychain_get_blob`, `keychain_set_blob`. | 6 | [PAR com 7-12] | API keys secure. |
| 14 | **Account picker UI** — `AccountPicker.swift` dentro de VendorCard. Listagem + select. Shape final depende UX-Q2 (vide §8). | 12, 13 | [SEQ] | OpenRouter/Z.AI multi-account UX. |
| 15 | **SQLite history layer** — `crates/torven-core/src/history/` com rusqlite + migration 001. Scheduler grava snapshot a cada refresh. FFI: `list_snapshots`, `aggregate_by_vendor`, etc. Retention job 90d. | 6 | [PAR com 7-12] | history.db populated, queryable via FFI. |
| 16 | **Main window + sidebar + Swift Charts** — `MainWindowView` com `SidebarView` (vendors + settings + insights history). Tabs Overview/History/Accounts/Settings por vendor. Swift Charts `HistoryChart.swift`. | 14, 15 | [SEQ] | Janela detalhada funcional. |
| 17 | **AI Insights core (Rust)** — `crates/torven-core/src/insights/` com Anthropic tool_use streaming. Trait LlmClient + Real + Mock. `prompts/insights.v1.md` + schema `v1.schema.json`. Budget guards (cost $0.05, input 8K, output 1K). Privacy redaction (account_id hash). | 15 | [SEQ] | Backend insights pronto. |
| 18 | **FFI: streaming callback interface** — UDL `callback interface InsightsCallback { onToken, onDone, onError }`. `[Async]` annotation em `request_insights`. **Spike 0.5d** para validar end-to-end (AR-6). | 17 | [SEQ] | Streaming FFI funcional. |
| 19 | **SwiftUI Insights UI streaming consumption** — `InsightsPanel.swift` no popover (Cmd+I trigger). Streaming render token-by-token via callback. Cost/latency display. Failure modes do PRD §6.6. | 18 | [SEQ] | UX completo de insights. |
| 20 | **Eval runner + dataset** — `crates/torven-core/src/bin/torven-evals.rs`. Dataset 30 casos rotulados em `evals/dataset.jsonl`. Judge LLM em `prompts/judge.v1.md`. Markdown report. | 17 | [PAR com 18-19] | Eval pipeline executável. |
| 21 | **CI eval gate** — `.github/workflows/eval-gate.yml` roda evals em PR que toca `prompts/` ou `insights/`. Bloqueia merge se regressão >5%. PR comment com métricas. | 20 | [SEQ] | Quality gate ativo. |
| 22 | **Settings UI (SwiftUI Settings scene)** — `SettingsView` com tabs (General/Vendors/Accounts/AI Insights/Data). Edita config.toml via FFI (Rust toml_edit preserva comentários). Hot-reload via callback `on_config_reloaded`. | 16 | [SEQ] | Settings funcional sem restart. |
| 23 | **Menu bar label dinâmico (NSStatusItem)** — label rotaciona vendor com maior pressão (FR-8 prioridade). Cor calm/amber/red. Label truncado 18 chars. **Spike refinement** se Story 8 deixou edge cases. | 12 | [PAR com 16-22] | Label dinâmico funcionando. |
| 24 | **Budget alerts (UNUserNotificationCenter)** — Swift side via `UserNotifications` framework. Trigger via FFI callback `on_budget_alert`. Throttle 1x/dia por threshold. Click abre janela detalhada filtrada. | 15, 22 | [SEQ] | Alertas funcionais. |
| 25 | **Diagnostics command (Torven Doctor)** — FFI `run_doctor() -> DoctorReport`. UI button em Settings + copy-to-clipboard. | 15, 17 | [PAR com 24] | FR-16 funcional. |
| 26 | **Sparkle 2 integration** — SPM dependency Sparkle, `SPUStandardUpdaterController` no AppDelegate. EdDSA key generation flow doc em `apple/Sparkle/README.md`. Public key em Info.plist via XcodeGen env. Appcast.xml URL configurado. | 3 | [PAR com 7-25] | Sparkle integrated, awaiting CI to publish appcast. |
| 27 | **Code signing + notarization pipeline** — `apple/scripts/notarize.sh`. CI: Apple ID + app-specific password em GitHub Actions secrets. Entitlements correctos em `Torven.entitlements`. Hardened runtime habilitado. Smoke test `spctl -a -v` post-notarize. | 26 | [SEQ] | Notarization funcional. |
| 28 | **CI release workflow rewrite** — `.github/workflows/release.yml`: build matrix macos-14 runner, `cargo build` core (aarch64 + x86_64), `lipo` universal, `xcodebuild -create-xcframework`, `xcodegen generate`, `xcodebuild` .app, `codesign` + `notarytool` + `stapler`, .dmg gen, Sparkle EdDSA sign appcast.xml, publish GH Release. | 27 | [SEQ] | tag push → release publicado completo. |
| 29 | **First-run onboarding wizard (FR-14)** — `OnboardingWizard.swift` first launch detection (`@AppStorage("hasCompletedOnboarding")`), 5 vendor cards com Login/Add key CTAs, Skip for now. | 14, 22 | [PAR com 23-28] | Onboarding funcional. |
| 30 | **README + screenshots + CHANGELOG + CLAUDE.md rewrite** — README hero AI Insights, screenshots SwiftUI (popover + main window + insight), eval metrics table, install via .dmg, comparison table from @analyst. CHANGELOG v1.0 pivot. CLAUDE.md release checklist updated. | 28 | [SEQ] | Portfolio-ready artifacts. |
| 31 | **v1.0 release tagging** — final checklist run, `git tag v1.0.0`, push tag, validate CI release succeeds, validate Sparkle appcast lives, validate first install flow on clean machine. | 30 | [SEQ] | Torven v1.0 released. |

**Total estimado:** 31 stories. R-5 do PRD pede ≤1d por story; algumas (13, 17, 18, 28) podem partir em 2 substories. @sm decide granularidade final.

**Paralelização permitida:**
- Após Story 1 (Bootstrap workspace) completar, Stories 2, 3, 4 podem rodar em paralelo (2 setup UniFFI, 3 setup Xcode, 4 cleanup Rust)
- Após Story 6 (config schema): Stories 13 (Keychain) e 15 (SQLite history) podem rodar em paralelo com a thread principal de UI (Stories 7-12)
- Após Story 12 (5 vendors): Story 23 (menu bar label dinâmico) é independente da thread 14-22 (popover/window/settings)
- Story 26 (Sparkle integration) pode começar logo após Story 3 (Xcode skeleton) — não bloqueia UI work

---

## 7. Architectural Risks

5-8 riscos específicos ao stack SwiftUI + Rust FFI. **Não inclui** riscos de produto (esses estão no PRD §9).

### AR-1 — UniFFI maturity gaps em Swift macOS (MEDIUM)

- **Risco:** UniFFI é primariamente focada em **iOS** (Firefox iOS, Mullvad iOS, Matrix Element iOS). macOS specifically tem menos production users. Debug experience em edge cases macOS (e.g. NSException crossing FFI boundary, code signing affecting library load) pode ter gaps documentation-wise.
- **Mitigação:**
  - Spike obrigatório de 0.5d na Story 2 (Setup UniFFI binding) — validar `xcframework` build + import + simple call before commit
  - Fallback documented: se UniFFI quebrar em macOS edge case, alternativa é swift-bridge (refactor isolado da FFI surface) ou manual `extern "C"` (~2 dias de retrabalho)
  - Monitor: UniFFI issues no GitHub filter por `macos` label semanal durante v1.0 dev

### AR-2 — Tokio runtime em FFI context (chamadas Rust async de Swift sync → thread blocking) (MEDIUM-HIGH)

- **Risco:** UniFFI `[Async]` annotation marshals Swift `async/await` para Rust `async fn`. Mas o tokio runtime precisa ser owned por algo que sobreviva — `OnceCell<Runtime>` static. Edge cases:
  - Se Swift cancela `async let task` mas UniFFI não propaga `CancellationToken` para tokio, request fica órfã
  - Multi-thread tokio runtime em context macOS sandboxed pode triggerar issues de thread spawning permitions
  - Sparkle update may require quiescing the runtime (`Runtime::shutdown_timeout`) — coreografia entre Swift lifecycle e Rust runtime
- **Mitigação:**
  - Usar **`new_current_thread` tokio runtime** (single-thread) em vez de multi-thread em primeiro experimento; promote para multi-thread só se profiling mostrar contenção
  - Spike de 0.5d na Story 18 — testar cancellation end-to-end com mockito (simular Anthropic streaming lento, cancelar mid-stream, validar tokio task cleanup)
  - AppDelegate `applicationWillTerminate` chama `TorvenCore.shutdown()` (FFI) que faz `runtime.shutdown_timeout(Duration::from_secs(2))` graceful

### AR-3 — Memory ownership ao cruzar FFI boundary (drop semantics, ARC vs Rust ownership) (HIGH)

- **Risco:** UniFFI gerencia ownership via reference counting bridge entre Swift ARC e Rust drop. Vazamentos podem ocorrer em:
  - Callback interfaces que mantêm reference cycle (`InsightsCallback` Swift retém `InsightsViewModel` que retém Rust `InsightsClient` que retém callback)
  - `Option<T>` retornados de FFI passando como nullable Swift — drop ordering inconsistente
  - Vec<T> com T sendo struct complex — large allocations crossing FFI cada chamada
- **Mitigação:**
  - **Spike de 0.5d na Story 9 (FFI fetch_snapshot)** — usar `leaks` Apple tool para profile uma sessão de 1h com refresh contínuo; baseline de zero leaks antes de prosseguir
  - Padrão para callbacks: Swift callback impl usa `weak self` no closure equivalent → quebra cycle
  - Vec<T> grandes: prefer FFI helpers que retornam batch sizes pequenos (e.g. `list_snapshots(limit: 100)`) — NÃO `get_all_history()` que pode retornar 10k+ rows
  - Test: `crates/torven-core/tests/ffi_memory.rs` runs 10k iterações de cada FFI call em loop, valida heap stable

### AR-4 — Sparkle EdDSA key management (perda da chave = perde controle de updates) (MEDIUM)

- **Risco:** Sparkle 2 verifica updates via EdDSA signature. Public key embedded em Info.plist no first release. **Se private key for perdida**, NÃO há recurso — usuários instalados nunca conseguirão receber updates assinados (eles confiam na key embedded no .app deles). Re-keying exige todos usuários reinstalar manualmente.
- **Mitigação:**
  - Private key armazenada em GitHub Actions secret `SPARKLE_PRIVATE_KEY` + backup encrypted em password manager pessoal (1Password ou similar)
  - Documentado em `apple/Sparkle/README.md` runbook explícito
  - Public key committed em `apple/Sparkle/public_key.txt` para sanity check em CI (compare Info.plist value vs known value)
  - Discipline: never rotate the key without coordinated re-keying plan (and accepting the cost)

### AR-5 — xcodebuild signing failures em CI (provisioning profile expiration, certificate revocation) (MEDIUM)

- **Risco:** Apple Developer Program certificate expira anualmente. Provisioning profile pode expirar. CI release falha silenciosamente em PR ou na noite anterior a entrevista importante.
- **Mitigação:**
  - GitHub Actions workflow valida cert expiration date em pre-flight check (`security find-certificate -p ...` + parse expiration); fails fast 30 dias antes
  - Notarytool API uses app-specific password (rotatable, low blast radius) instead of Apple ID password (full account access)
  - Documentado em CLAUDE.md release checklist: "if release fails with signing error, check Apple Developer portal cert expiration"
  - Backup: `make release-local` target permite build + sign + notarize localmente quando CI falha (release atrasado mas viável)

### AR-6 — Universal binary size (lipo dobra o tamanho do binário Rust) (LOW-MEDIUM)

- **Risco:** Rust core compilado para aarch64-apple-darwin é ~5-8MB; compilado para x86_64-apple-darwin é igual; `lipo -create` produz binary ~10-16MB (dobra). NFR-5 do PRD pede `.app` <25MB total.
- **Estimate atualizada para v2 (SwiftUI):**
  - Rust core universal: ~10-12MB (após `strip = "symbols"`, `lto = "fat"`, `codegen-units = 1` em release profile)
  - Swift app bundle: ~3-5MB (SwiftUI runtime já no OS, só app code conta)
  - Sparkle.framework: ~3-4MB
  - Assets (icons, colors): <1MB
  - **Total estimado: ~17-22MB**. Dentro de NFR-5.
- **Mitigação:**
  - CI build step reporta `.app` size em PR comment; gate em 24MB
  - Se ultrapassar: usar `cargo bloat` para identificar deps Rust pesadas (e.g. `rusqlite` com features default vs com `bundled` minimal); strip mais agressivo
  - Stretch: Rust binary stripping mais agressivo via `strip = "debuginfo"` + `panic = "abort"` (perde panic info em release, mas ganha tamanho)

### AR-7 — Swift Charts performance com 90 dias de snapshots (LOW)

- **Risco:** 90 dias × 5 vendors × 1 snapshot/30s = ~1.3M rows. Renderizar tudo em Swift Charts vira lag perceptível (NFR-1 violado).
- **Mitigação:**
  - **Server-side downsample no Rust core**: `aggregate_for_chart(vendor, since_ts, max_points)` retorna no max 200 pontos (1 ponto/hour para 30 dias é ~720 pontos; downsample para 200)
  - Swift Charts performance é boa até ~1k pontos sem otimização — 200 é folga
  - Caching: tabela `chart_aggregates(vendor, granularity, ts_bucket, agg_cost_usd, ...)` populated lazy on first chart open per session

### AR-8 — NSStatusItem custom view positioning bugs (multi-monitor, MenuBarExtra .window style) (MEDIUM)

- **Risco:** Apple tem histórico documentado de bugs com MenuBarExtra `.window` style em edge cases multi-monitor (popover aparece em screen errado), notch (M-series MacBook Pros), e dynamic display arrangement changes. Fórum apple.com tem dezenas de threads.
- **Mitigação:**
  - Spike de 0.5d na Story 8 — testar com 1 monitor, 2 monitor (external + built-in), MacBook notch
  - Fallback documentado: substituir MenuBarExtra por `NSStatusItem` direto via AppKit bridge no AppDelegate (mais code, mas full control) — refactor isolado de Story 8
  - Snapshot tests: `apple/TorvenTests/MenuBarPositioningTests.swift` valida frame coordinates do popover relative to status item

---

## 8. Handoff para @ux-design-expert (Uma)

Estas 5 questões precisam ser fechadas pelo @ux-design-expert antes de @sm criar stories de UI específicas (Stories 10, 14, 16, 19, 22 da migration sequence).

### UX-Q1 — Layout SwiftUI específico: MenuBarExtra constraints e NSPopover sizing

O PRD pede 5 cards no popover ~360×420px. Mas SwiftUI MenuBarExtra com `.window` style tem constraints específicas:
- Tamanho **mínimo recomendado** Apple: 250×280
- Tamanho **máximo razoável** (sem feel "janela secundária"): ~480×640
- Cada VendorCard típico (header + status pill + main metric + delta + sparkline 7d) tem altura natural ~90-110px no SwiftUI. 5 × 90 = 450px + header 50px = 500px → spill em 420px height.

**Opções:**
- **A.** Aumentar popover para 380×540 (acomoda 5 cards full + header sem scroll)
- **B.** Cards colapsáveis (1 expanded ~120px + 4 collapsed ~50px = 320px + header = 370px)
- **C.** Scroll vertical com 360×420 fixo (UX feio em popover macOS)
- **D.** "Compact mode" / "Expanded mode" toggle (Cmd+E expandable)

Decisão @ux + considerar trade-off com NFR-1 (popover open <50ms warm — mais conteúdo = mais work no mount).

### UX-Q2 — Estados visuais com Swift Charts (cores semânticas, animação, dark mode)

Swift Charts respeita system appearance e accent color automaticamente. Mas precisamos definir:

- **Paleta semântica Calm/Amber/Critical:** valores HEX para light mode e dark mode (assets catalog Semantic/)
- **Mapping em line chart:** Anthropic = cor X, OpenAI = cor Y, Gemini = cor Z, Z.AI = cor W, OpenRouter = cor V — sugiro paleta categórica accessibility-compliant (8-color palette OK para colorblind WCAG AA)
- **Animação:** Swift Charts anima updates automaticamente; queremos animação subtle (300ms) ou explicit timing?
- **Eixo Y formatting:** `$X.XX` para cost, `XX%` para window, `X / Y` para messages quota — quem decide format por chart?
- **Empty state:** quando vendor sem histórico ainda, chart mostra placeholder ou hide?

### UX-Q3 — Account picker UX no popover compacto

Quando OpenRouter tem 4 accounts em popover de ~360px width, como mostrar? Esta decisão impacta directly Story 14.

**Opções:**
- **A.** Combobox dropdown no topo do card (Apple-native NSPopUpButton equivalent — `Picker` SwiftUI)
- **B.** Segmented control horizontal (4 chips clicáveis, max 4-5 antes de wrap)
- **C.** Expandable list embaixo do card (acordeon — clica "4 accounts ▾" expande lista vertical)
- **D.** "All accounts" agregado por default + link "View per account →" abre janela detalhada (postpone choice)

Trade-off: density (B/C usa mais vertical space) × clarity (A é familiar) × clicks-to-task (D adiciona um clique para detail view).

### UX-Q4 — AI Insights streaming UI (progress, partial reveal, completion)

PRD §6 e ADR-7 indicam streaming. Mas a experiência visual exata:

- **Loading state inicial** (entre clicar Insights e primeiro token): spinner? Skeleton estructurado mostrando "headline coming..." "insights[] coming..."?
- **Partial reveal:** mostrar `{"headline": "You spent...` literalmente parsed (raw) ou apenas exibir o `accumulated_text` em fonte monoespaçada como Claude-app-style streaming?
- **Transition para final structured view:** quando `onDone` chega, transição visual de "streaming text view" → "structured InsightCard view" deve ser smooth (crossfade, slide, etc.)?
- **Cost/latency display:** sempre visible enquanto streaming? Aparece só no fim? Tooltip?
- **Cancel button:** UX de cancelamento mid-stream (CTA prominent? small X? confirmação?)

### UX-Q5 — Settings UI organização (sidebar style vs tabs?)

SwiftUI `Settings` scene em macOS 13+ permite 2 styles:

- **A. Tabs no topo (TabView style)** — paradigma macOS tradicional (System Preferences pre-Ventura). Apple usa em Xcode Preferences, Terminal Settings
- **B. Sidebar (NavigationSplitView style)** — paradigma macOS Ventura+ (System Settings nova). Apple usa em System Settings nativo

Considerar:
- Settings do Torven tem: General, Vendors, Accounts (per vendor expansion), AI Insights, Data (retention/export), Updates, About — ~7 sections
- Tabs ficam congestionadas com 7 items; Sidebar acomoda melhor
- Mas Sidebar é "mais peso visual" — Settings de menu bar app é tradicionalmente leve

Decisão @ux: A tabs ou B sidebar (com sub-items expansíveis para accounts).

### UX-Q6 — Dark mode handling e system appearance API

SwiftUI tem `@Environment(\.colorScheme)` automatic. Mas:

- **Asset catalog colors** (Semantic/Calm, Semantic/Amber, Semantic/Critical) precisam de variantes light e dark — quem define os pairs?
- **Menu bar label color:** macOS automatically inverts para dark/light menu bar — mas se quisermos color encoding (calm/amber/critical), precisamos override via `.foregroundColor` — funciona em ambos modes?
- **Custom icons** (chart.bar.fill SF Symbol vs custom): SF Symbols auto-adapt; custom requires both variants

---

## 9. Handoff para @sm (River)

**Confirmação:** as 31 stories da migration sequence (§6) estão prontas para detalhamento pelo @sm. Cada uma tem:

- **Título** claro
- **Dependências explícitas** (story numbers)
- **Mode** ([INDEP] / [SEQ] / [PAR])
- **Output esperado** ao completar

**Granularidade:** R-5 do PRD pede ≤1d por story. Stories ≤1d na minha estimativa: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 19, 22, 23, 24, 25, 29, 30, 31 (~20 stories). Stories que provavelmente partir em 2 substories pelo @sm: 13 (Keychain spike + impl), 14 (Account picker depends on UX-Q3), 16 (Main window + charts é grande), 17 (AI Insights core é o cérebro), 18 (FFI streaming callback spike + impl), 26 (Sparkle key gen + integration), 27 (notarize pipeline), 28 (release workflow rewrite — sub-substories: matrix setup, xcframework, signing, dmg, appcast).

**Paralelização — quais stories podem começar IMEDIATAMENTE (sem depender de UX):**

✅ **Story 1 (Bootstrap workspace)** — pode começar agora, não depende de UX
✅ **Story 4 (Cleanup Linux-coupled code)** — pode começar imediatamente após Story 1 completar, sem precisar de UX. É pure Rust deletion.
✅ **Story 2 (Setup UniFFI binding)** — depende só de Story 1; sem necessidade UX
✅ **Story 3 (Bootstrap Xcode project via XcodeGen)** — depende só de Story 2; sem necessidade UX (UI viewer é hello world)
✅ **Story 5 (Migrate format.rs)** — pure Rust refactor; sem UX
✅ **Story 6 (Migrate config.rs schema)** — pure Rust; sem UX
✅ **Story 13 (Keychain integration)** — pure Rust + FFI; sem UX (mas precisa Story 2 done)
✅ **Story 15 (SQLite history)** — pure Rust + FFI; sem UX
✅ **Story 17 (AI Insights Rust core)** — pure Rust; sem UX (depende Story 15)
✅ **Story 20 (Eval runner + dataset)** — pure Rust; sem UX (paralelo com 18/19)
✅ **Story 26 (Sparkle integration)** — pure Swift integration; sem UX

**Stories que PRECISAM de UX fechado primeiro:**
- **Story 10 (Popover skeleton)** — precisa UX-Q1 (layout/sizing)
- **Story 14 (Account picker)** — precisa UX-Q3 (account picker shape)
- **Story 16 (Main window + charts)** — precisa UX-Q5 (sidebar vs tabs), UX-Q2 (chart styling)
- **Story 19 (Insights UI streaming)** — precisa UX-Q4 (streaming experience)
- **Story 22 (Settings UI)** — precisa UX-Q5 (sidebar vs tabs)

**Recomendação:** @sm pode criar Stories 1-6 e 13, 15, 17, 20 imediatamente. @ux-design-expert tem ~2 semanas (durante Stories 1-9 execution) para fechar UX-Q1-Q6 antes que UI stories cheguem.

---

## Resumo Executivo (para o user) — 300 palavras max

**Decisões finais 10 ADRs (1 linha cada):**

1. **ADR-1 [REUSED]:** Workspace Cargo `crates/torven-core` (lib + bin evals) + `crates/torven-tui` (TUI); Xcode project em `apple/` separado.
2. **ADR-2 [REUSED]:** Keychain híbrido (blob JSON por vendor + SQLite metadata) com lógica no Rust core via `security-framework`.
3. **ADR-3 [REUSED]:** Sparkle 2 nativo Swift via SPM com EdDSA defense-in-depth contra Apple cert lapse.
4. **ADR-4 [NEW — decisão de maior impacto]:** UniFFI (Mozilla) como Swift↔Rust FFI binding tool — production-tested em Firefox iOS, Mullvad VPN, Matrix Element iOS.
5. **ADR-5 [REUSED]:** SQLite via rusqlite no Rust core; Swift consulta exclusivamente via FFI (NUNCA GRDB/CoreData direto).
6. **ADR-6 [REFAZER]:** Swift Charts framework (nativo macOS 13+, zero deps, dark mode automático, WCAG AA grátis).
7. **ADR-7 [REUSED]:** Anthropic tool_use mode com chamada feita do Rust core, streaming retornado via UniFFI callback interface.
8. **ADR-8 [REUSED]:** Eval runner Rust nativo (`torven-evals` bin); reaproveitamento 100% do ADR descartado.
9. **ADR-9 [NEW]:** XcodeGen (YAML → .xcodeproj) — AIOX-friendly, git-friendly, reproducible.
10. **ADR-10 [NEW]:** SwiftUI `@main App` + `@NSApplicationDelegateAdaptor` híbrido — MenuBarExtra-compatible + AppKit escape hatch.

**Top-3 riscos arquiteturais (SPECIFIC ao stack Swift+Rust):**

1. **AR-3 (Memory ownership FFI boundary — HIGH):** ARC ↔ Rust drop semantics. Callback interfaces propensas a retain cycles. Spike 0.5d Story 9 com `leaks` Apple tool baseline obrigatório.
2. **AR-2 (Tokio runtime em FFI context — MEDIUM-HIGH):** async Rust de Swift sync. Cancellation propagation. Spike 0.5d Story 18 com mockito streaming cancel test.
3. **AR-8 (NSStatusItem multi-monitor MenuBarExtra positioning — MEDIUM):** histórico documentado de bugs Apple. Spike 0.5d Story 8 + fallback `NSStatusItem` direto via AppKit.

**Paralelização confirmada — stories que podem começar IMEDIATAMENTE (sem UX):** Stories 1-6, 13, 15, 17, 20, 26. UX precisa fechar UX-Q1-Q6 antes das Stories 10, 14, 16, 19, 22 chegarem (~2 semanas folga).

**Próximos passos para @ux-design-expert (5 perguntas a fechar):** UX-Q1 (popover layout 360×420 vs 380×540), UX-Q2 (paleta semântica light/dark Swift Charts), UX-Q3 (account picker shape A/B/C/D), UX-Q4 (insights streaming UX), UX-Q5 (Settings sidebar vs tabs).

**FIM do ADR v2.0. Status: Active. Supersedes Tauri path discarded 2026-05-31.**
