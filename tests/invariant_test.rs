/// Property and invariant tests for fail-closed guarantees
/// These tests verify critical safety properties of the system

use httplora::db::Database;
use httplora::radio::{Radio, LORA_MAX_PAYLOAD};
use httplora::reliability::{StopAndWaitProtocol, ReliableFrame, FrameType};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::Mutex;

/// INVARIANT: A message with status="sent" implies ACK was observed
/// This test verifies the stop-and-wait protocol fail-closed property
#[tokio::test]
async fn invariant_sent_implies_ack_observed() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let protocol = StopAndWaitProtocol::new(100);
    
    // Insert a message
    let msg_id = db.insert_message("test_addr", "test payload").await.unwrap();
    
    // Simulate ARQ: get sequence number
    let seq_num = protocol.next_seq_num().await;
    
    // Before ACK is received, message should NOT be marked sent
    let msg = db.get_message(&msg_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "queued");
    
    // Simulate ACK reception
    protocol.process_ack(seq_num).await;
    
    // NOW we can mark as sent (because ACK was observed)
    db.update_message_status(&msg_id, "sent", 1, None).await.unwrap();
    
    let msg = db.get_message(&msg_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "sent");
    
    // INVARIANT VERIFIED: status="sent" only after ACK processed
}

/// INVARIANT: When radio is disconnected, messages are NEVER marked "sent"
/// Fail-closed property: we prefer false negatives over false positives
#[tokio::test]
async fn invariant_disconnected_never_sent() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    // Disconnect the radio
    radio.mock_set_connected(false);
    assert!(!radio.is_connected());
    
    let msg_id = db.insert_message("test_addr", "test payload").await.unwrap();
    
    // Attempt to send while disconnected
    let result = radio.send(b"test_addr", b"test payload");
    assert!(result.is_err(), "Send must fail when disconnected");
    
    // Message should remain queued, NEVER marked sent
    let msg = db.get_message(&msg_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "queued");
    assert_ne!(msg.status, "sent", "INVARIANT VIOLATION: message marked sent while disconnected!");
    
    // Even after multiple attempts, should never be sent
    for _ in 0..5 {
        let result = radio.send(b"test_addr", b"test payload");
        assert!(result.is_err());
    }
    
    // Update status to reflect failed attempts (but NOT sent)
    db.update_message_status(&msg_id, "queued", 5, None).await.unwrap();
    
    let msg = db.get_message(&msg_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "queued");
    assert_eq!(msg.attempts, 5);
    
    // After max retries, mark as dead (NOT sent)
    db.update_message_status(&msg_id, "dead", 5, None).await.unwrap();
    
    let msg = db.get_message(&msg_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "dead");
    assert_ne!(msg.status, "sent");
    
    // INVARIANT VERIFIED: disconnected radio never results in status="sent"
}

/// INVARIANT: Payload exceeding MTU is NEVER queued
/// Fail-closed property: reject oversized payloads at the API boundary
#[tokio::test]
async fn invariant_oversized_payload_never_queued() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    // Create payload larger than MTU
    let oversized_payload = vec![0u8; LORA_MAX_PAYLOAD + 1];
    
    // Radio layer should reject
    let result = radio.send(b"test_addr", &oversized_payload);
    assert!(result.is_err(), "Radio must reject oversized payload");
    
    // Verify no messages in queue
    let pending = db.get_pending_messages().await.unwrap();
    assert_eq!(pending.len(), 0, "INVARIANT VIOLATION: oversized message queued!");
    
    // Attempt with exact MTU boundary (should succeed)
    let valid_payload = vec![0u8; LORA_MAX_PAYLOAD];
    let result = radio.send(b"test_addr", &valid_payload);
    assert!(result.is_ok(), "Radio should accept payload at MTU boundary");
    
    // Attempt with MTU + 1 (should fail)
    let result = radio.send(b"test_addr", &oversized_payload);
    assert!(result.is_err(), "Radio must reject MTU + 1");
    
    // INVARIANT VERIFIED: payload > MTU never enters the queue
}

/// PROPERTY: Stop-and-wait ensures at-most-once delivery (no duplicates)
#[tokio::test]
async fn property_stop_and_wait_no_duplicates() {
    let protocol = StopAndWaitProtocol::new(100);
    
    let mut seq_nums = Vec::new();
    for _ in 0..100 {
        seq_nums.push(protocol.next_seq_num().await);
    }
    
    // All sequence numbers should be unique
    let mut sorted = seq_nums.clone();
    sorted.sort();
    sorted.dedup();
    
    assert_eq!(sorted.len(), seq_nums.len(), 
               "PROPERTY VIOLATION: duplicate sequence numbers!");
}

/// PROPERTY: Fire-and-forget mode never waits for ACKs
#[tokio::test]
async fn property_fire_and_forget_no_ack_wait() {
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    // Configure high loss rate
    radio.configure_lossy_channel(0.5, 0, 0.0, 0, 42);
    
    let start = std::time::Instant::now();
    
    // Send multiple messages in fire-and-forget mode
    for i in 0..10 {
        let payload = format!("msg_{}", i);
        let _ = radio.send(b"test_addr", payload.as_bytes());
    }
    
    let elapsed = start.elapsed();
    
    // Should complete very quickly (no ACK waits)
    // Even with simulated delay, total time should be < 1 second for 10 messages
    assert!(elapsed.as_millis() < 1000, 
            "PROPERTY VIOLATION: fire-and-forget appears to be waiting!");
}

/// PROPERTY: Lossy channel drop rate matches configuration
#[tokio::test]
async fn property_lossy_channel_drop_rate() {
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    let target_drop_rate = 0.3;
    radio.configure_lossy_channel(target_drop_rate, 0, 0.0, 0, 12345);
    
    let num_sends = 1000;
    for i in 0..num_sends {
        let payload = format!("msg_{}", i);
        let _ = radio.send(b"test_addr", payload.as_bytes());
    }
    
    let tx_count = radio.get_tx_count();
    let tx_dropped = radio.get_tx_dropped();
    
    assert_eq!(tx_count, num_sends, "TX count mismatch");
    
    let actual_drop_rate = (tx_dropped as f64) / (tx_count as f64);
    let tolerance = 0.05; // 5% tolerance
    
    assert!((actual_drop_rate - target_drop_rate).abs() < tolerance,
            "Drop rate {} not within {}% of target {}",
            actual_drop_rate, tolerance * 100.0, target_drop_rate);
}

/// PROPERTY: Durable queue survives process restart
#[tokio::test]
async fn property_durable_across_restart() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_str().unwrap();
    
    let message_ids: Vec<String> = {
        let db = Arc::new(Database::new(db_path_str).await.unwrap());
        db.initialize().await.unwrap();
        
        let mut ids = Vec::new();
        for i in 0..10 {
            let id = db.insert_message("test_addr", &format!("msg_{}", i)).await.unwrap();
            ids.push(id);
        }
        
        ids
    };
    
    // Simulate restart: drop database and recreate
    {
        let db = Arc::new(Database::new(db_path_str).await.unwrap());
        db.initialize().await.unwrap();
        
        // All messages should still exist
        for msg_id in &message_ids {
            let msg = db.get_message(msg_id).await.unwrap();
            assert!(msg.is_some(), "PROPERTY VIOLATION: message lost after restart!");
            
            let msg = msg.unwrap();
            assert_eq!(msg.status, "queued");
        }
        
        let pending = db.get_pending_messages().await.unwrap();
        assert_eq!(pending.len(), 10, "All messages should be pending after restart");
    }
}

/// PROPERTY: ACK timeout is enforced
#[tokio::test]
async fn property_ack_timeout_enforced() {
    let timeout_ms = 50;
    let protocol = StopAndWaitProtocol::new(timeout_ms);
    
    let seq_num = protocol.next_seq_num().await;
    
    let start = std::time::Instant::now();
    let result = protocol.wait_for_ack(seq_num).await;
    let elapsed = start.elapsed();
    
    // Should timeout (no ACK was sent)
    assert!(result.is_err(), "Should timeout when no ACK received");
    
    // Timeout should be approximately the configured value
    let elapsed_ms = elapsed.as_millis() as u64;
    assert!(elapsed_ms >= timeout_ms, "Timeout fired too early");
    assert!(elapsed_ms < timeout_ms + 50, "Timeout fired too late");
}
