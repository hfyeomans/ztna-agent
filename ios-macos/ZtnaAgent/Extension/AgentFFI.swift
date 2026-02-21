import os

/// Thread-safe wrapper for the Rust QUIC Agent OpaquePointer.
///
/// Protects the pointer lifecycle with `OSAllocatedUnfairLock` to prevent
/// use-after-free during teardown. Thread safety for ongoing FFI calls is
/// ensured by the `networkQueue` serialization and `isRunning` guard protocol:
///
/// 1. `stopTunnel` sets `isRunning = false` (atomic, visible to all queues)
/// 2. All entry points check `isRunning` before touching agent
/// 3. `stopTunnel` dispatches `destroy()` on networkQueue with `.barrier` flag,
///    ensuring all in-flight work completes before the pointer is freed.
///
/// Lock-based (not actor) to avoid async overhead on the hot packet path.
/// `OSAllocatedUnfairLock.withLock` is nanosecond-level with zero allocations.
final class AgentFFI: @unchecked Sendable {

    /// Wrapper to make OpaquePointer compatible with OSAllocatedUnfairLock.
    /// OpaquePointer lost Sendable conformance in Swift 6; the lock provides safety.
    private struct PointerState: @unchecked Sendable {
        var pointer: OpaquePointer?
    }

    private let lock = OSAllocatedUnfairLock(initialState: PointerState())
    private let logger = Logger(subsystem: "com.hankyeomans.ztna-agent", category: "AgentFFI")

    /// Quick check if agent pointer is alive (non-nil).
    var isAlive: Bool {
        lock.withLockUnchecked { $0.pointer != nil }
    }

    /// Read the current agent pointer. Returns nil after destroy.
    ///
    /// Callers must ensure agent lifetime via the teardown protocol:
    /// check `isRunning` before starting work, and `stopTunnel` must use
    /// barrier dispatch for orderly shutdown.
    var agent: OpaquePointer? {
        lock.withLockUnchecked { $0.pointer }
    }

    /// Create a new Rust QUIC agent. Destroys any existing agent first.
    @discardableResult
    func create() -> OpaquePointer? {
        lock.withLockUnchecked { state in
            if let existing = state.pointer {
                logger.warning("Creating agent while previous exists — destroying old agent")
                agent_destroy(existing)
            }
            state.pointer = agent_create()
            if state.pointer != nil {
                logger.info("Agent created")
            } else {
                logger.error("agent_create() returned NULL")
            }
            return state.pointer
        }
    }

    /// Idempotent teardown: destroy agent and nullify pointer under lock.
    /// Safe to call multiple times — subsequent calls are no-ops.
    func destroy() {
        lock.withLockUnchecked { state in
            guard let existing = state.pointer else { return }
            agent_destroy(existing)
            state.pointer = nil
            logger.info("Agent destroyed")
        }
    }
}
