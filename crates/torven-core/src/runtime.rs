//! FFI-owned tokio runtime — single instance, single-thread, `'static` lifetime.
//!
//! ## Why this exists (Story 1.15 + AR-2 resolution)
//!
//! Swift calls Rust through synchronous-looking UniFFI entry points; the
//! Rust side that needs to `await` (HTTP streaming, file I/O batched, etc.)
//! must drive its own tokio runtime — there is no Swift event loop reachable
//! from the Rust side. ADR-7 line 583 and the AR-2 spike resolution
//! (`docs/architecture/ar-2-spike-resolution.md` §Decision 2) mandate:
//!
//! - **`new_current_thread()`** (single-thread). The workload is I/O-bound
//!   (≤6 concurrent network requests at peak: 5 vendor fetches + 1 Insight
//!   stream); a worker-thread runtime adds App Sandbox surface area, makes
//!   `applicationWillTerminate` shutdown non-deterministic, and exposes a
//!   callback-ordering race against `@Published` SwiftUI state. None of
//!   those tradeoffs buy us anything for an I/O-bound workload of this
//!   size.
//! - **`OnceCell<Runtime>`** (single instance, leaked-for-`'static`).
//!   UniFFI scaffolding hands us thread-local `&self` callbacks from many
//!   call sites; the runtime must outlive every individual FFI call. The
//!   `&'static Runtime` returned by [`get_runtime`] is safe to capture in
//!   `tokio::spawn` futures.
//! - **No `Runtime::shutdown_timeout`** from FFI in v1.0. `OnceCell` does
//!   not surrender ownership of the runtime, so we cannot `move` it into
//!   `shutdown_timeout`. macOS reclaims the OS thread on process exit. If
//!   future telemetry shows quit-time leaks or CPU-after-quit, we will
//!   revisit (the design hook is documented but unused).
//!
//! See also: ADR-7 lines 558-614 (Anthropic tool_use streaming), AR-2 lines
//! 1395-1404 (original risk statement, now superseded by the spike doc).

use once_cell::sync::OnceCell;
use tokio::runtime::Runtime;

/// Process-global single-instance tokio runtime. Initialized lazily on the
/// first call to [`get_runtime`]; the resulting `Runtime` lives for the
/// remainder of the process.
static RUNTIME: OnceCell<Runtime> = OnceCell::new();

/// Returns a `&'static Runtime` suitable for `Runtime::block_on` and
/// `Runtime::spawn`. The first caller pays the cost of construction; every
/// subsequent caller gets the same instance.
///
/// ## Panics
///
/// Panics if `tokio::runtime::Builder::new_current_thread().build()` fails.
/// That should only happen if the process is out of file descriptors or has
/// hit a `RLIMIT_NOFILE`-class limit. We treat that as unrecoverable for the
/// FFI surface — without a runtime, no async FFI function can make progress.
pub fn get_runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .thread_name("torven-tokio")
            .build()
            .expect("failed to build tokio runtime")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_runtime_returns_same_instance() {
        let a = get_runtime();
        let b = get_runtime();
        // Same `&'static` reference — `OnceCell` guarantees init-once.
        assert!(std::ptr::eq(a, b));
    }

    #[test]
    fn get_runtime_can_run_a_task() {
        let rt = get_runtime();
        let n = rt.block_on(async { 1 + 2 });
        assert_eq!(n, 3);
    }
}
