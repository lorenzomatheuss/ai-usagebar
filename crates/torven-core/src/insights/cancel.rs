//! Cancellation handle — FFI-exposed wrapper around a
//! `tokio_util::sync::CancellationToken`.
//!
//! ## Why this exists (AR-2 resolution §Decision 3)
//!
//! UniFFI has no native cross-FFI cancellation mechanism. The Swift side
//! constructs a `CancelHandle`, passes it into the streaming request, and
//! calls `cancel()` on popover dismiss. The Rust streaming loop races the
//! HTTP chunk stream against `token.cancelled()` via `tokio::select!`,
//! returning `InsightsError::Cancelled` if the cancel arm wins.
//!
//! ## Why `tokio_util::sync::CancellationToken` (not `tokio::sync::oneshot`)
//!
//! - Cloneable, hierarchical, `Send + Sync` out of the box.
//! - Designed exactly for this use case.
//! - Single line `tokio::select!` integration vs. plumbing a oneshot Sender
//!   across the FFI boundary as a custom UniFFI type.
//!
//! ## UniFFI binding shape
//!
//! Declared in `torven_core.udl` as `interface CancelHandle { constructor;
//! cancel; is_cancelled; }`. UDL mode requires us to expose `Arc<Self>` and
//! `&self` methods — the UDL `interface` keyword generates Swift bindings
//! that go through reference-counted boxing. See `uniffi_exports.rs` for
//! the bridge functions UniFFI calls.

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

/// Handle that the Swift caller passes into streaming requests and uses to
/// cancel them. Internally wraps a `CancellationToken` that the Rust
/// streaming task `tokio::select!`s against.
///
/// Cloning the `Arc<CancelHandle>` clones the underlying token (a cheap
/// `Arc<Inner>` clone — they share state). All clones observe the same
/// cancellation event.
#[derive(Debug)]
pub struct CancelHandle {
    inner: CancellationToken,
}

impl CancelHandle {
    /// Constructs a fresh handle in the "not cancelled" state. The UniFFI
    /// scaffolding calls this from Swift as `CancelHandle()`.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: CancellationToken::new(),
        })
    }

    /// Signals cancellation. All clones and child tokens observe the event
    /// immediately. Idempotent — calling twice has no extra effect.
    pub fn cancel(&self) {
        self.inner.cancel();
    }

    /// Returns `true` if `cancel()` was called previously on this handle
    /// (or any clone of it). Used by callers that want to bail before
    /// initiating a new request.
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    /// Returns the underlying `CancellationToken` for use inside
    /// `tokio::select!` arms. Crate-private — Swift only sees the
    /// `cancel`/`is_cancelled` surface.
    pub(crate) fn token(&self) -> CancellationToken {
        self.inner.clone()
    }
}

impl Default for CancelHandle {
    fn default() -> Self {
        Self {
            inner: CancellationToken::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancel_handle_starts_not_cancelled() {
        let h = CancelHandle::new();
        assert!(!h.is_cancelled());
    }

    #[test]
    fn cancel_marks_handle_cancelled() {
        let h = CancelHandle::new();
        h.cancel();
        assert!(h.is_cancelled());
    }

    #[test]
    fn cancel_is_idempotent() {
        let h = CancelHandle::new();
        h.cancel();
        h.cancel();
        assert!(h.is_cancelled());
    }

    #[test]
    fn token_observes_cancellation_from_handle() {
        let h = CancelHandle::new();
        let tok = h.token();
        assert!(!tok.is_cancelled());
        h.cancel();
        assert!(tok.is_cancelled());
    }
}
