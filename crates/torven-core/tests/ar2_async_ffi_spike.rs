//! AR-2 empirical spike — Story 1.15 T9.
//!
//! See `docs/architecture/ar-2-spike-resolution.md` §Decision 4 for the
//! design. The five assertions below are the gating criteria for the AR-2
//! risk:
//!
//! 1. **Cancel latency p99 < 100ms** over 1000 cycles.
//! 2. **Zero orphan tasks after cancellation.** Operationalised as
//!    "no callback fires after `cancel()` is observed by the streaming
//!    loop", which is the user-visible symptom of an orphan task. (We do
//!    NOT use `tokio::runtime::Handle::metrics().num_alive_tasks()` —
//!    that API requires the `tokio_unstable` cfg, which would force us
//!    to add a rustc flag and ripple through CI. The semantic outcome
//!    we care about is identical.)
//! 3. **No `on_token` after `cancel()`.** Same instrumentation as #2,
//!    asserted directly.
//! 4. **Memory stable across 1000 cycles** — RSS delta < 5 MiB
//!    (`mach_task_basic_info` on macOS via `libc`).
//! 5. **Happy-path control:** 100 cycles without cancellation all return
//!    `Ok(InsightsOutput)` and emit ≥ 1 `on_token`.
//!
//! Long-running; run with:
//!   `cargo test -p torven-core --test ar2_async_ffi_spike -- --ignored --nocapture`
//!
//! Captures p50/p99 cancel latency and RSS delta to stdout; the Change Log
//! on Story 1.15 quotes those numbers.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use torven_core::insights::{
    CancelHandle, InsightsCallback, InsightsContext, InsightsError, LlmClient, RealAnthropicClient,
};
use torven_core::runtime::get_runtime;

mod support;
use support::mock_anthropic::start_streaming;

/// Counting callback used across all spike assertions. Tracks total
/// `on_token` invocations and how many of them landed after the cancel
/// flag was raised.
struct SpyCallback {
    total_tokens: AtomicUsize,
    after_cancel: AtomicUsize,
    cancel_observed: Arc<AtomicBool>,
}

impl InsightsCallback for SpyCallback {
    fn on_token(&self, _token: String) {
        self.total_tokens.fetch_add(1, Ordering::SeqCst);
        if self.cancel_observed.load(Ordering::SeqCst) {
            self.after_cancel.fetch_add(1, Ordering::SeqCst);
        }
    }
    fn on_error(&self, _error: String) {}
}

#[cfg(target_os = "macos")]
fn current_rss_bytes() -> u64 {
    // Use `proc_pid_rusage` instead of `task_info` to avoid pulling the
    // mach types crate in. `getrusage` is portable and the
    // `ru_maxrss` field on macOS is in BYTES (BSD-style), not KB.
    use std::mem::MaybeUninit;
    let mut usage = MaybeUninit::<libc::rusage>::uninit();
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return 0;
    }
    let usage = unsafe { usage.assume_init() };
    usage.ru_maxrss as u64
}

#[cfg(not(target_os = "macos"))]
fn current_rss_bytes() -> u64 {
    // On Linux `ru_maxrss` is KB; on macOS bytes. Keep the helper portable
    // so the spike can also be run in CI Linux runners during pre-merge.
    use std::mem::MaybeUninit;
    let mut usage = MaybeUninit::<libc::rusage>::uninit();
    let rc = unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) };
    if rc != 0 {
        return 0;
    }
    let usage = unsafe { usage.assume_init() };
    (usage.ru_maxrss as u64) * 1024
}

const CYCLES: usize = 1000;
const HAPPY_CYCLES: usize = 100;

/// AR-2 assertions 1, 2, 3: cancellation latency, no orphan-task symptoms,
/// no post-cancel callbacks. Runs `CYCLES` cancel cycles.
#[test]
#[ignore = "AR-2 spike — run with --ignored"]
fn ar2_cancellation_under_100ms_and_no_post_cancel_callbacks() {
    let rt = get_runtime();

    // One mock for the whole spike — 50ms chunk delay × 60 chunks = ~3s
    // total stream length, plenty for the 200ms post-start cancel.
    let mock = rt.block_on(start_streaming(
        60,
        Duration::from_millis(50),
        CYCLES as u64 * 2,
    ));
    let base_url = mock.base_url().to_string();

    let mut latencies: Vec<Duration> = Vec::with_capacity(CYCLES);
    let mut total_emitted = 0usize;

    for i in 0..CYCLES {
        let client =
            RealAnthropicClient::new_with_base_url("test-key".to_string(), base_url.clone());
        let handle = CancelHandle::new_arc();
        let cancel_observed = Arc::new(AtomicBool::new(false));
        let cb = Arc::new(SpyCallback {
            total_tokens: AtomicUsize::new(0),
            after_cancel: AtomicUsize::new(0),
            cancel_observed: cancel_observed.clone(),
        });
        let cb_dyn: Arc<dyn InsightsCallback> = cb.clone();

        let handle_for_cancel = handle.clone();
        let cancel_at = Arc::new(std::sync::Mutex::new(None::<Instant>));
        let cancel_at_w = cancel_at.clone();
        let cancel_task = rt.spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            *cancel_at_w.lock().unwrap() = Some(Instant::now());
            cancel_observed.store(true, Ordering::SeqCst);
            handle_for_cancel.cancel();
        });

        let result = rt.block_on(client.request_insight_streaming(
            InsightsContext::synthetic(),
            cb_dyn,
            handle,
        ));
        rt.block_on(cancel_task).unwrap();
        let cancel_instant = cancel_at.lock().unwrap().expect("cancel task ran");
        let latency = cancel_instant.elapsed();
        latencies.push(latency);

        assert!(
            matches!(result, Err(InsightsError::Cancelled)),
            "cycle {i}: expected Cancelled, got {result:?}"
        );

        // Assertion 3: no callback after cancel observed. Sleep a bit so
        // any pending tokio task that *would* fire `on_token` gets a
        // chance to do so before we check.
        std::thread::sleep(Duration::from_millis(50));
        let after = cb.after_cancel.load(Ordering::SeqCst);
        assert_eq!(
            after, 0,
            "cycle {i}: {after} on_token callbacks fired after cancel"
        );
        total_emitted += cb.total_tokens.load(Ordering::SeqCst);
    }

    latencies.sort();
    let p50 = latencies[latencies.len() / 2];
    let p99 = latencies[(latencies.len() * 99) / 100];
    let max = *latencies.last().unwrap();
    println!(
        "AR-2 cancel-latency: cycles={CYCLES} p50={:?} p99={:?} max={:?} avg_tokens/cycle={:.1}",
        p50,
        p99,
        max,
        total_emitted as f64 / CYCLES as f64,
    );

    // Assertion 1: p99 < 100ms.
    assert!(
        p99 < Duration::from_millis(100),
        "p99 cancel latency = {p99:?} (limit 100ms)"
    );
}

/// AR-2 assertion 4: memory stable across `CYCLES` cycles.
#[test]
#[ignore = "AR-2 spike — run with --ignored"]
fn ar2_memory_stable_1000_cycles() {
    let rt = get_runtime();
    let mock = rt.block_on(start_streaming(
        60,
        Duration::from_millis(20),
        CYCLES as u64 * 2,
    ));
    let base_url = mock.base_url().to_string();

    let cb = Arc::new(SilentCallback);
    let mut samples: Vec<(usize, u64)> = Vec::new();

    for i in 0..CYCLES {
        let client =
            RealAnthropicClient::new_with_base_url("test-key".to_string(), base_url.clone());
        let handle = CancelHandle::new_arc();
        let cb_dyn: Arc<dyn InsightsCallback> = cb.clone();
        let handle_for_cancel = handle.clone();
        let cancel_task = rt.spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            handle_for_cancel.cancel();
        });
        let _ = rt.block_on(client.request_insight_streaming(
            InsightsContext::synthetic(),
            cb_dyn,
            handle,
        ));
        rt.block_on(cancel_task).unwrap();

        if i == 10 || i == 500 || i == CYCLES - 10 {
            samples.push((i, current_rss_bytes()));
        }
    }

    println!("AR-2 RSS samples: {samples:?}");
    let baseline = samples.first().map(|(_, b)| *b).unwrap_or(0);
    let final_rss = samples.last().map(|(_, r)| *r).unwrap_or(0);
    let delta = final_rss.saturating_sub(baseline);
    println!(
        "AR-2 RSS delta: baseline={} bytes, final={} bytes, delta={} bytes ({:.2} MiB)",
        baseline,
        final_rss,
        delta,
        delta as f64 / (1024.0 * 1024.0),
    );
    assert!(
        delta < 5 * 1024 * 1024,
        "RSS delta {} bytes exceeds 5 MiB ceiling",
        delta
    );
}

/// AR-2 assertion 5: happy path — `HAPPY_CYCLES` cycles without cancel
/// all return `Ok` and emit ≥ 1 token.
#[test]
#[ignore = "AR-2 spike — run with --ignored"]
fn ar2_happy_path_100_cycles() {
    let rt = get_runtime();
    // Tight chunks (5ms × 30) so the test finishes quickly — happy path
    // doesn't care about cancel timing.
    let mock = rt.block_on(start_streaming(
        30,
        Duration::from_millis(5),
        HAPPY_CYCLES as u64 * 2,
    ));
    let base_url = mock.base_url().to_string();

    for i in 0..HAPPY_CYCLES {
        let client =
            RealAnthropicClient::new_with_base_url("test-key".to_string(), base_url.clone());
        let handle = CancelHandle::new_arc();
        let cancel_observed = Arc::new(AtomicBool::new(false));
        let cb = Arc::new(SpyCallback {
            total_tokens: AtomicUsize::new(0),
            after_cancel: AtomicUsize::new(0),
            cancel_observed,
        });
        let cb_dyn: Arc<dyn InsightsCallback> = cb.clone();

        let result = rt.block_on(client.request_insight_streaming(
            InsightsContext::synthetic(),
            cb_dyn,
            handle,
        ));
        match result {
            Ok(output) => {
                assert_eq!(output.prompt_version, "v1", "cycle {i}");
                assert!(!output.headline.is_empty(), "cycle {i}");
            }
            Err(e) => panic!("cycle {i}: expected Ok, got Err({e:?})"),
        }
        let tokens = cb.total_tokens.load(Ordering::SeqCst);
        assert!(
            tokens >= 1,
            "cycle {i}: emitted {tokens} tokens, expected >= 1"
        );
    }
    println!("AR-2 happy-path: {HAPPY_CYCLES} cycles OK");
}

struct SilentCallback;
impl InsightsCallback for SilentCallback {
    fn on_token(&self, _t: String) {}
    fn on_error(&self, _e: String) {}
}
