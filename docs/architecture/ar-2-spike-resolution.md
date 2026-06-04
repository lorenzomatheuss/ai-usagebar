# AR-2 Resolution — Tokio Runtime in UniFFI FFI Context

**Status:** RESOLVED (design); awaits empirical confirmation via spike in Story 1.15 T9
**Decided by:** @architect (Aria)
**Date:** 2026-06-04
**Supersedes:** AR-2 risk in `docs/architecture/torven-v1-adr.md` §7 (lines 1395-1404)
**Scope:** Defines the inegotiable shape of the async FFI surface for AI Insights streaming + cancellation. Binds Story 1.15 implementation.

---

## Context

Story 1.15 (AI Insights core) needs to expose a Rust async function — `request_insight_streaming(context, callback) -> InsightsOutput` — to Swift, while:
(a) streaming tokens token-by-token from Anthropic Messages API to SwiftUI;
(b) letting Swift cancel mid-stream (user closes popover);
(c) running inside a single, sandbox-safe tokio runtime owned by the Rust core.

ADR-4 (UniFFI, line 236) and ADR-7 (Anthropic tool_use, line 558) each prescribe pieces of the answer, but **leave a tension unresolved**: ADR-7 mandates `new_current_thread()` for sandbox safety; ADR-4 promotes `[Async]` annotation; Story 1.15 AC-6 declares a `callback interface InsightsCallback`. The current Story 1.15 T8 names an undefined-in-UDL signature `ffi_request_insight([Async] InsightsContext context, InsightsCallback callback)`. This document resolves all four design choices empirically and binds @dev to a single shape.

Authoritative external source consulted: UniFFI 0.29 futures docs (`mozilla.github.io/uniffi-rs/0.29/futures.html`), which establishes two load-bearing facts: (1) `[Async]` and `callback interface` **can** coexist when the trait method uses `async-trait`; (2) **UniFFI has no native cancellation** — caller must build a separate cancellation channel ("for example, exposing a `cancel()` method that sets a flag the library checks periodically").

---

## Decision 1 — FFI Async Surface: `[Async]` annotation + callback interface (both, not either-or)

**Decision:** Use `[Async]` annotation on the FFI entry point **AND** a `callback interface` for token streaming. They are complementary, not alternatives.

**Why not callback-only (sync FFI call + fire-and-forget async inside Rust):**
Swift call site would have to be:
```swift
client.requestInsights(context: ctx, callback: viewModel)  // returns void immediately
// ... wait for callback.onDone or .onError to be invoked, no idiomatic way to await
```
This forces the SwiftUI side to model the request as imperative state-machine on `viewModel`, instead of a single `async let` — losing structured concurrency, cancellation propagation, and the ability to use `try await` for the terminal result. Anti-pattern in modern Swift.

**Why not `[Async]`-only (await the whole future, no callback):**
The final `InsightsOutput` only arrives after `message_stop`. There is no way to surface intermediate tokens. SwiftUI cannot render the typewriter effect that is a UX-mandatory cue ("LLM is working"). Defeats the streaming requirement.

**Decision:** Both. The `[Async]` future resolves on `message_stop` with the final structured `InsightsOutput`; the callback fires on every `content_block_delta`. UniFFI 0.29 supports this combo (confirmed via futures.html docs and `async-api-client` example referenced therein).

**Final UDL (binds T8 of Story 1.15):**

```udl
namespace torven_core {
    // ... existing ...
};

[Error]
enum InsightsError {
    "Network",
    "Auth",
    "RateLimit",
    "InputBudgetExceeded",
    "Timeout",
    "Cancelled",      // NEW — required for Decision 3
    "ParseFailure",
    "SchemaInvalid",
};

callback interface InsightsCallback {
    void on_token(string token);          // partial JSON chunk from content_block_delta
    void on_error(string error);          // terminal error (also resolves future as Err)
    // NOTE: no on_done — final InsightsOutput is the Future's return value (Decision 1)
};

dictionary InsightsOutput { ... };        // unchanged from AC-7

interface InsightsClient {
    constructor(string api_key);

    [Async, Throws=InsightsError]
    InsightsOutput request_insight_streaming(
        InsightsContext context,
        InsightsCallback callback,
        CancelHandle cancel_handle        // see Decision 3
    );
};

interface CancelHandle {
    constructor();
    void cancel();
    boolean is_cancelled();
};
```

**Swift call site (binds Story 1.19 design):**
```swift
let cancel = CancelHandle()
do {
    let output = try await client.requestInsightStreaming(
        context: ctx,
        callback: self,         // ViewModel implements InsightsCallback
        cancelHandle: cancel
    )
    viewModel.finalOutput = output
} catch InsightsError.Cancelled {
    // user closed popover — silent
} catch {
    viewModel.errorBanner = "\(error)"
}

// elsewhere, on popover dismiss:
cancel.cancel()
```

This is the **only** acceptable shape. AC-6 (`on_done` in callback) is superseded: see "Updates Required to Story 1.15" below.

---

## Decision 2 — Tokio Runtime Builder: `new_current_thread()` (confirms ADR-7)

**Decision:** `tokio::runtime::Builder::new_current_thread().enable_all().build()` — single-thread. **Confirms ADR-7 line 583's directive; overrides any drift toward multi-thread.**

**Why not `new_multi_thread()`:**

| Failure mode (multi-thread) | Concrete scenario |
|---|---|
| App Sandbox entitlement bloat | `new_multi_thread()` spawns worker threads via `pthread_create`. macOS App Sandbox (`com.apple.security.app-sandbox = true`, required for notarization and future Mac App Store) does not block thread spawning per se, but each spawned thread inherits sandbox restrictions and increases the surface for `EAGAIN` failures under load. Single-thread eliminates a class of edge cases. |
| Sparkle shutdown coreography | On `applicationWillTerminate`, Rust must `runtime.shutdown_timeout(2s)`. With multi-thread, worker threads may be mid-HTTP-response when shutdown fires → ungraceful aborts logged to Console.app, visible to the user. Single-thread = one shutdown point, deterministic. |
| Callback ordering surprise | UniFFI marshals callback invocations back to Swift. With multi-thread tokio, two callbacks from two parallel tasks could race onto the FFI bridge. UniFFI 0.29 docs note callback delivery is thread-safe but ordering across tasks is not guaranteed. SwiftUI's `@Published` updates require main-actor; reordering tokens visibly corrupts the typewriter effect. |

**Why current_thread is sufficient:**

Workload sizing (from PRD §6 + AC-5 budget guards):
- Max 1 in-flight Insight request per InsightsClient (AC-5 rate limit: 1/min)
- Concurrent vendor fetches (Story 1.7/1.9) run on **the same** runtime but at most 5 in parallel (5 vendors), each a single HTTP request ~1-5s
- Total concurrent tasks at peak: ~6 (5 vendor fetches + 1 Insight stream)
- `current_thread` with `enable_all()` polls all 6 cooperatively on one OS thread — no contention because the workload is I/O-bound (network), not CPU-bound

**Failure scenario that would force multi-thread (and how we detect it):**
If profiling (Story 1.7/1.9 perf tests or Story 1.17 eval runner) shows >50ms of `Poll::Pending` starvation on the insights task while a vendor fetch is mid-decode, we'd be CPU-bound, not I/O-bound, and `new_multi_thread().worker_threads(2)` would be justified. The AR-2 spike below includes this measurement explicitly so we don't promote on a hunch.

**Final code (binds Story 1.15 T2):**
```rust
// crates/torven-core/src/runtime.rs
use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;

static RUNTIME: OnceCell<Runtime> = OnceCell::new();

pub fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .thread_name("torven-tokio")
            .build()
            .expect("failed to build tokio runtime")
    })
}

pub fn shutdown() {
    // Called from FFI on applicationWillTerminate
    if let Some(rt) = RUNTIME.get() {
        // Cannot move out of static; use shutdown_background instead.
        // (Runtime drops cleanly when process exits if shutdown_timeout not called.)
        // For graceful: expose `Runtime::shutdown_timeout` only if AppDelegate path proves needed.
    }
}
```

Note: `Runtime::shutdown_timeout` requires owning the Runtime, which `OnceCell` does not surrender. ADR-7 line 1404's `TorvenCore.shutdown()` directive is preserved as a *future* enhancement gated on observing actual leak/CPU-on-quit telemetry; for v1.0, process exit suffices (macOS reclaims threads on process death).

---

## Decision 3 — Cancellation Strategy: external `CancelHandle` interface (UniFFI-exposed flag + tokio `select!`)

**Decision:** Expose a `CancelHandle` UDL interface (Decision 1 UDL above) that wraps a `tokio_util::sync::CancellationToken`. Rust streaming loop uses `tokio::select!` to race the HTTP stream against the cancellation token.

**Why not `tokio::sync::oneshot` (ADR-7's original suggestion):**
A oneshot channel can only fire once and the *receiver* must be the streaming task. Passing the sender across FFI is awkward: UniFFI does not natively export `tokio::sync::oneshot::Sender<()>` as a Swift type. We'd wrap it in a custom `[Custom]` UDL type, duplicating work `CancellationToken` already does idiomatically.

**Why not drop-as-signal (drop the callback handle to cancel):**
UniFFI's callback drop semantics are deterministic in steady state but the *timing* of when Swift's ARC releases the callback after a request is not observable from Rust. We'd be waiting on Swift's runtime to decide. Not a control surface — a leak detector at best.

**Why `CancellationToken` from `tokio-util`:**
- Designed for exactly this use case; clonable, hierarchical, `Send + Sync`.
- Wrapping it in a UniFFI `interface CancelHandle` is canonical (UniFFI 0.29 supports `interface` types holding `Arc<Mutex<...>>` internals).
- `tokio::select!` integration is one line: `select! { _ = token.cancelled() => return Err(Cancelled), chunk = stream.next() => ... }`.

**Flow diagram:**

```
Swift                          Rust (tokio task)               Anthropic
  │                                  │                              │
  ├─ let h = CancelHandle()          │                              │
  ├─ try await client                │                              │
  │   .requestInsightStreaming(      │                              │
  │      ctx, self, cancelHandle: h) │                              │
  │                                  ├─ tokio::spawn task           │
  │                                  ├─ token = h.inner.clone()     │
  │                                  ├─ HTTP POST stream ──────────►│
  │                                  │                              │
  │                                  │◄─── content_block_delta ─────┤
  │◄── callback.on_token("...")──────┤                              │
  │                                  │                              │
  │  [user closes popover]           │                              │
  ├─ h.cancel()                      │                              │
  │  → token.cancel()                │                              │
  │                                  │  select! fires cancelled arm │
  │                                  ├─ drop stream (HTTP abort)    │
  │                                  │  ────────── RST ────────────►│
  │                                  ├─ return Err(Cancelled)       │
  │◄── future throws Cancelled ──────┤                              │
```

**Implementation sketch (binds Story 1.15 T5):**

```rust
// crates/torven-core/src/insights/cancel.rs
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(uniffi::Object)]
pub struct CancelHandle {
    inner: CancellationToken,
}

#[uniffi::export]
impl CancelHandle {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self { inner: CancellationToken::new() })
    }
    pub fn cancel(&self) { self.inner.cancel(); }
    pub fn is_cancelled(&self) -> bool { self.inner.is_cancelled() }
}

impl CancelHandle {
    pub(crate) fn token(&self) -> &CancellationToken { &self.inner }
}
```

```rust
// crates/torven-core/src/insights/client.rs (excerpt)
async fn run_streaming(
    &self,
    ctx: InsightsContext,
    cb: Arc<dyn InsightsCallback>,
    handle: Arc<CancelHandle>,
) -> Result<InsightsOutput, InsightsError> {
    let mut stream = self.http.post(URL).json(&body).send().await?.bytes_stream();
    let token = handle.token();
    let mut acc = String::new();
    loop {
        tokio::select! {
            biased;
            _ = token.cancelled() => return Err(InsightsError::Cancelled),
            chunk = stream.next() => match chunk {
                Some(Ok(bytes)) => { /* parse delta, call cb.on_token, append to acc */ }
                Some(Err(e))    => return Err(InsightsError::Network(e.to_string())),
                None            => break,  // message_stop
            }
        }
    }
    let output = validate_output(&acc)?;
    Ok(output)
}
```

New cargo dep: `tokio-util = { version = "0.7", features = ["rt"] }`.

---

## Decision 4 — Empirical Spike (replaces Story 1.15 T9)

**Files to create:**
- `crates/torven-core/tests/ar2_async_ffi_spike.rs` — the spike test (modeled on `ar3_leaks_spike.rs` from Story 1.5)
- `crates/torven-core/tests/support/mock_anthropic.rs` — mockito streaming server emitting `content_block_delta` SSE chunks with configurable delay

**Command:**
```bash
cargo test -p torven-core --test ar2_async_ffi_spike -- --ignored --nocapture
```

(The `--ignored` mirrors `ar3_leaks_spike.rs` — long-running, opt-in, run pre-merge.)

**Assertions (each MUST pass for spike to be GREEN):**

1. **Cancellation latency < 100ms.**
   Spawn 1000 cycles: start a streaming request against a mockito server that emits one token every 50ms for 60 tokens (3s total); after 200ms call `handle.cancel()`; measure time from `cancel()` to future returning `Err(Cancelled)`. Assert: `p99 < 100ms`, `max < 200ms`.

2. **Zero orphaned tokio tasks after cancellation.**
   After each cancellation, the Runtime's task count (via `tokio::runtime::Handle::metrics().num_alive_tasks()` — stable on tokio 1.42+) must return to the pre-request baseline within 50ms. Assert: `runtime_metrics_alive_tasks == baseline` for all 1000 cycles. (This catches the AR-2 "orphan task" risk literally.)

3. **No callback invocation after cancellation.**
   `InsightsCallback` impl counts `on_token` calls. After `cancel()` returns, no further `on_token` MUST fire. Assert: a 500ms wait after cancellation observes zero additional callback invocations.

4. **Memory stable across 1000 cycles** (AR-3 cross-check).
   Use `jemalloc-ctl` or RSS via `/proc/self/statm` equivalent (`mach_task_basic_info` on macOS) — sample RSS at iteration 10, 500, 990. Assert: `rss_990 - rss_10 < 5 MiB` (loose bound; we're catching leaks, not micro-allocations).

5. **Happy-path streaming completes when not cancelled.**
   Control group: 100 cycles with no cancellation. Assert: all return `Ok(InsightsOutput)`, `on_token` called ≥30 times each, no panics.

**Skeleton (≤60 lines, @dev fills in `mock_anthropic` helper and exact metric reads):**

```rust
// crates/torven-core/tests/ar2_async_ffi_spike.rs
//! AR-2 empirical spike — see docs/architecture/ar-2-spike-resolution.md §Decision 4
//! Long-running; run with `cargo test --test ar2_async_ffi_spike -- --ignored --nocapture`.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use torven_core::insights::{InsightsClient, InsightsContext, CancelHandle, InsightsCallback};
use torven_core::runtime::get_runtime;

mod support;
use support::mock_anthropic::MockAnthropic;

struct CountingCallback {
    tokens: AtomicUsize,
    after_cancel: AtomicUsize,
    cancelled_flag: Arc<std::sync::atomic::AtomicBool>,
}
impl InsightsCallback for CountingCallback {
    fn on_token(&self, _tok: String) {
        self.tokens.fetch_add(1, Ordering::SeqCst);
        if self.cancelled_flag.load(Ordering::SeqCst) {
            self.after_cancel.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn on_error(&self, _e: String) {}
}

#[test]
#[ignore]
fn ar2_cancellation_under_100ms_and_no_orphan_tasks() {
    let mock = MockAnthropic::start_streaming(60, Duration::from_millis(50));
    let client = InsightsClient::new_for_test(mock.url(), "test-key");
    let rt = get_runtime();
    let metrics = rt.metrics();
    let baseline = metrics.num_alive_tasks();

    let mut latencies = Vec::with_capacity(1000);

    for i in 0..1000 {
        let handle = CancelHandle::new();
        let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cb = Arc::new(CountingCallback {
            tokens: AtomicUsize::new(0),
            after_cancel: AtomicUsize::new(0),
            cancelled_flag: flag.clone(),
        });
        let cb_dyn: Arc<dyn InsightsCallback> = cb.clone();

        let fut = client.request_insight_streaming(InsightsContext::synthetic(), cb_dyn, handle.clone());
        let h = handle.clone();
        let start_cancel = rt.spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let t0 = Instant::now();
            flag.store(true, Ordering::SeqCst);
            h.cancel();
            t0
        });

        let result = rt.block_on(fut);
        let t0 = rt.block_on(start_cancel).unwrap();
        let latency = t0.elapsed();
        latencies.push(latency);

        assert!(matches!(result, Err(torven_core::insights::InsightsError::Cancelled)), "cycle {i}");
        // Assertion 3: no callback after cancellation flag set
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(cb.after_cancel.load(Ordering::SeqCst), 0, "cycle {i}: callback after cancel");
        // Assertion 2: no orphaned tasks
        assert_eq!(metrics.num_alive_tasks(), baseline, "cycle {i}: orphan task");
    }

    // Assertion 1: p99 < 100ms
    latencies.sort();
    let p99 = latencies[(latencies.len() * 99) / 100];
    assert!(p99 < Duration::from_millis(100), "p99 cancel latency = {p99:?}");
}

#[test]
#[ignore]
fn ar2_happy_path_100_cycles() { /* assertion 5 */ }

#[test]
#[ignore]
fn ar2_memory_stable_1000_cycles() { /* assertion 4 — uses libc::mach_task_basic_info on macOS */ }
```

**Definition of spike DONE:** all five assertions green on macOS (aarch64 + x86_64 via CI matrix). Spike output (latency p50/p99, max RSS delta, alive_tasks baseline) captured in Story 1.15 Change Log entry, mirroring the format used for AR-3 in Story 1.5.

---

## Updates Required to Story 1.15

These are **recommendations to @sm/@po** — Story 1.15 is currently `Ready`, and per `.claude/rules/story-lifecycle.md` only @po can edit AC/Title/Scope. @architect cannot modify the story directly.

- **AC-6:** Remove `on_done(InsightsOutput output)` from the UDL `callback interface InsightsCallback`. The final structured output is delivered as the return value of the `[Async]` function (Decision 1). Keep `on_token` and `on_error`. The Change Log on the story must record the AC drift with reference to this document.

- **AC-7:** Unchanged — `InsightsOutput` dictionary still required (now as the Future's return type, not a callback parameter).

- **AC-8:** Unchanged in intent (`new_current_thread()`) — Decision 2 confirms. Add a sentence: "Runtime is `&'static Runtime` via `OnceCell::get_or_init`; never call `Runtime::shutdown_timeout` from FFI in v1.0 (deferred per AR-2 resolution §Decision 2)."

- **AC-3 (`LlmClient` trait):** Add a third parameter `cancel: Arc<CancelHandle>` to `request_insight_streaming`. Required for Decision 3 to plumb cancellation into the trait, so the mock and real client behave identically.

- **NEW AC-10 (recommended):** "FFI surface exposes `CancelHandle` interface (constructor, `cancel()`, `is_cancelled()`) backed by `tokio_util::sync::CancellationToken`. Streaming task uses `tokio::select!` to race HTTP chunks against cancellation."

- **T2:** Add `tokio-util = { version = "0.7", features = ["rt"] }` to deps list.

- **T5 (RealAnthropicClient):** Add the `tokio::select!` cancellation arm shown in Decision 3.

- **T8 (UDL):** Replace `ffi_request_insight([Async] InsightsContext context, InsightsCallback callback)` with the exact UDL block in Decision 1 above (3 elements: `InsightsError`, `InsightsCallback` without `on_done`, `InsightsClient.request_insight_streaming` with `CancelHandle`, plus `CancelHandle` interface).

- **T9:** **Replaced by this document.** New T9 text: "Implement and run `tests/ar2_async_ffi_spike.rs` per `docs/architecture/ar-2-spike-resolution.md §Decision 4`. All 5 assertions must pass before T5 (real Anthropic client) is merged. Log p99 cancel latency + max RSS delta + alive_tasks_baseline in Change Log."

- **"Risk: AR-2" section at story bottom:** Replace narrative with `Resolved by docs/architecture/ar-2-spike-resolution.md (status: RESOLVED design; spike pending @dev).`

---

## References

- ADR-4, lines 236-388 (`docs/architecture/torven-v1-adr.md`) — UniFFI binding tool decision, async strategy, xcframework packaging
- ADR-7, lines 558-614 (`docs/architecture/torven-v1-adr.md`) — Anthropic tool_use streaming, tokio runtime ownership, cancellation rationale
- AR-2, lines 1395-1404 (`docs/architecture/torven-v1-adr.md`) — original risk statement, superseded by this document
- Story 1.5 precedent: `crates/torven-core/tests/ar3_leaks_spike.rs` — empirical-spike-as-test pattern (AR-3 RESOLVED, 0 leaks across 10k iterations)
- UniFFI 0.29 futures docs: https://mozilla.github.io/uniffi-rs/0.29/futures.html — confirms `[Async]` + callback interface coexistence and **no native cancellation** (must build separate channel)
- UniFFI 0.29 `async-api-client` example (referenced by futures.html) — canonical pattern for the combo used here
- `tokio-util` `CancellationToken`: https://docs.rs/tokio-util/0.7/tokio_util/sync/struct.CancellationToken.html
- `crates/torven-core/Cargo.toml` lines 67, 77 — UniFFI pinned at 0.29 (confirmed Story 1.2)
