use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use crate::state::WindowId;

/// Default fail-safe timeout after which pending transactions are force-committed.
pub const TRANSACTION_TIMEOUT: Duration = Duration::from_millis(300);

/// A synchronized multi-window resize transaction.
/// Prevents layout tearing by waiting for all resized windows to commit new framebuffers.
#[derive(Debug, Clone)]
pub struct Transaction {
    pub id: u64,
    pub deadline: Instant,
    pub pending_windows: HashSet<WindowId>,
    pub committed_windows: HashSet<WindowId>,
}

impl Transaction {
    pub fn new(id: u64, windows: impl IntoIterator<Item = WindowId>) -> Self {
        let pending_windows: HashSet<WindowId> = windows.into_iter().collect();
        Self {
            id,
            deadline: Instant::now() + TRANSACTION_TIMEOUT,
            pending_windows,
            committed_windows: HashSet::new(),
        }
    }

    /// Mark a window as committed for this transaction. Returns true if all windows are now committed.
    pub fn on_commit(&mut self, window_id: WindowId) -> bool {
        if self.pending_windows.remove(&window_id) {
            self.committed_windows.insert(window_id);
        }
        self.is_complete()
    }

    /// Checks if all pending windows have committed their new sizes.
    pub fn is_complete(&self) -> bool {
        self.pending_windows.is_empty()
    }

    /// Checks if the fail-safe timeout has elapsed.
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
}

/// Manager tracking active multi-window sync transactions.
#[derive(Debug, Default)]
pub struct TransactionManager {
    next_id: u64,
    pub active_transactions: HashMap<u64, Transaction>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            active_transactions: HashMap::new(),
        }
    }

    /// Create and register a new transaction for the given window set.
    pub fn create_transaction(&mut self, windows: impl IntoIterator<Item = WindowId>) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let tx = Transaction::new(id, windows);
        if !tx.is_complete() {
            self.active_transactions.insert(id, tx);
        }
        id
    }

    /// Process a window surface commit. Cleans up completed/expired transactions.
    pub fn on_surface_commit(&mut self, window_id: WindowId) {
        let mut completed = Vec::new();

        for (id, tx) in self.active_transactions.iter_mut() {
            if tx.on_commit(window_id) || tx.is_expired() {
                completed.push(*id);
            }
        }

        for id in completed {
            self.active_transactions.remove(&id);
        }
    }

    /// Check and prune any expired transactions (call on each render/event tick).
    pub fn prune_expired(&mut self) {
        self.active_transactions
            .retain(|_, tx| !tx.is_expired() && !tx.is_complete());
    }

    /// Returns true if a window is currently blocked waiting for other siblings in a transaction.
    pub fn is_blocked(&self, window_id: WindowId) -> bool {
        self.active_transactions
            .values()
            .any(|tx| tx.committed_windows.contains(&window_id) && !tx.is_complete())
    }

    /// True while any synchronized-resize transaction is in flight.
    ///
    /// Render loops consult this to withhold presentation until every
    /// resized client has committed its new framebuffer (or the fail-safe
    /// timeout expired) — the atomic-swap behavior transactions exist for.
    pub fn has_active_transactions(&self) -> bool {
        !self.active_transactions.is_empty()
    }

    /// Force-complete every active transaction.
    ///
    /// Used when a resize drag ends: whatever the clients committed by now
    /// is presented immediately instead of freezing output for up to
    /// TRANSACTION_TIMEOUT waiting for stragglers.
    pub fn force_complete_all(&mut self) {
        self.active_transactions.clear();
    }
}
