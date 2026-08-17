use std::time::Duration;
use truss::sync::{Transaction, TransactionManager};
use truss::WindowId;

#[test]
fn test_transaction_creation_and_commit_lifecycle() {
    let mut tx_mgr = TransactionManager::new();

    // Create transaction for windows 1, 2, 3
    let tx_id = tx_mgr.create_transaction(vec![WindowId(1), WindowId(2), WindowId(3)]);
    assert!(tx_mgr.active_transactions.contains_key(&tx_id));

    // Window 1 commits
    tx_mgr.on_surface_commit(WindowId(1));
    assert!(tx_mgr.active_transactions.contains_key(&tx_id));
    assert!(tx_mgr.is_blocked(WindowId(1)));

    // Window 2 commits
    tx_mgr.on_surface_commit(WindowId(2));
    assert!(tx_mgr.active_transactions.contains_key(&tx_id));

    // Window 3 commits -> transaction completes and gets removed
    tx_mgr.on_surface_commit(WindowId(3));
    assert!(!tx_mgr.active_transactions.contains_key(&tx_id));
    assert!(!tx_mgr.is_blocked(WindowId(1)));
}

#[test]
fn test_transaction_timeout_expiration() {
    let mut tx = Transaction::new(1, vec![WindowId(1), WindowId(2)]);
    assert!(!tx.is_expired());

    // Artificial deadline in the past
    tx.deadline = std::time::Instant::now() - Duration::from_millis(10);
    assert!(tx.is_expired());
}
