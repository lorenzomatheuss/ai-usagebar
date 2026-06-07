# Spike — Claude Code Keychain OAuth schema

**Story:** 5.5.2 anthropic-oauth-claude-code-keychain — AC-0 BLOCKING spike
**Date:** 2026-06-07
**Operator:** Lorenzo (bench session)
**Verdict:** **GO** — schema 100% compatível com struct `ClaudeAiOauth` existente, parse direto sem adapter

---

## Contexto

Story 5.5.2 precisa ler credenciais OAuth do **Claude Code** (não Claude CLI standalone). Claude Code grava em macOS Keychain entry `"Claude Code-credentials"` em vez de `~/.claude/.credentials.json`. Antes de codar o parser, schema precisa ser confirmado.

Sem schema cravado → @dev codaria contra shape chutado → quebra runtime → re-trabalho.

## Discovery commands (executados por Lorenzo)

### Query 1 — top-level keys

```bash
security find-generic-password -s "Claude Code-credentials" -w | jq 'keys'
```

**Output:**
```json
[
  "claudeAiOauth",
  "trustedDeviceToken"
]
```

### Query 2 — claudeAiOauth sub-keys

```bash
security find-generic-password -s "Claude Code-credentials" -w | jq '.claudeAiOauth | keys'
```

**Output:**
```json
[
  "accessToken",
  "expiresAt",
  "rateLimitTier",
  "refreshToken",
  "scopes",
  "subscriptionType"
]
```

## Schema documentado

Top-level wrapper:

```json
{
  "claudeAiOauth": {
    "accessToken": "<JWT>",
    "expiresAt": <epoch_ms>,
    "rateLimitTier": "<string>",
    "refreshToken": "<refresh>",
    "scopes": [<array>],
    "subscriptionType": "<string>"
  },
  "trustedDeviceToken": "<opaque>"
}
```

- `expiresAt` é epoch **milliseconds** (mesmo unit que file format `~/.claude/.credentials.json`)
- `trustedDeviceToken` é device-trust token não-OAuth — **out-of-scope Story 5.5.2** (ignorar; serde drops automaticamente)

## Mapping pra struct existente

Struct `ClaudeAiOauth` em `crates/torven-core/src/vendors/anthropic/creds.rs:14-37` **já tem todos os 6 sub-fields** com `#[serde(rename)]` apontando pros camelCase keys:

| Keychain JSON key | Rust struct field | serde rename evidence |
|---|---|---|
| `accessToken` | `access_token: String` | line 20 `rename = "accessToken"` |
| `refreshToken` | `refresh_token: String` | line 22 `rename = "refreshToken"` |
| `expiresAt` | `expires_at_ms: i64` | line 27 `rename = "expiresAt", deserialize_with = "de_ms_epoch"` |
| `subscriptionType` | `subscription_type: String` | line 29 `rename = "subscriptionType", default` |
| `rateLimitTier` | `rate_limit_tier: String` | line 31 `rename = "rateLimitTier", default` |
| `scopes` | `scopes: Option<serde_json::Value>` | line 35 `default, skip_serializing_if = "Option::is_none"` |

Top-level wrapper (line 14):
```rust
#[serde(rename = "claudeAiOauth")]
pub claude_ai_oauth: ClaudeAiOauth,
```

→ extra top-level key `trustedDeviceToken` é silently dropped por serde (default deny_unknown_fields OFF). Zero risco.

## Verdict: GO

Schema **100% compatível**. Story 5.5.2 AC-0 spike result:

- ✅ No adapter layer needed
- ✅ Existing `ClaudeAiOauth` struct `serde_json::from_str::<ClaudeCreds>(blob)` parsa direto
- ✅ `expires_at_ms` epoch ms calibrado (já matches `oauth::needs_refresh` pattern)
- ✅ `trustedDeviceToken` ignorável (não-OAuth, fora de escopo)

## Implicações pra Story 5.5.2 AC-1 → AC-14

Story scope **inalterado** (não cresce nem encolhe). Schema GO significa:

- AC-1 `CredsSource::Keychain(String)` variant: trivial
- AC-2 `read_from_keychain(service)` impl: `keyring::Entry::new("Claude Code-credentials", "")` (or platform-specific service flavor) → `get_password()` → `serde_json::from_str::<ClaudeCreds>(blob)` — done
- AC-3 `write_back_to_keychain` impl: `serde_json::to_string(&creds)` → `keyring::Entry::set_password(blob)`
- AC-6 unit tests: fixture inline JSON com shape acima, assert round-trip via `serde_json::from_str` / `to_string`

**Carry-forward sugerido pra Dev Agent Record:** validar `keyring` crate macOS backend usa **service-only lookup** (sem `account` filter), match Claude Code's `security find-generic-password -s ...` (sem `-a`).

## Próximo passo (pipeline retoma)

@dev `*develop 5.5.2` autônomo YOLO. Sem novos blockers.
