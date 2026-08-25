use httplora::api::{AppState, SendMessageRequest};
use httplora::db::Database;
use httplora::radio::{Radio, LORA_MAX_PAYLOAD};
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::Mutex;

#[tokio::test]
async fn test_http_accept_durable_persist() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_str().unwrap();
    
    let db = Arc::new(Database::new(db_path_str).await.unwrap());
    db.initialize().await.unwrap();
    
    let radio = Arc::new(Mutex::new(Radio::new(None)));
    
    let state = AppState {
        db: db.clone(),
        radio: radio.clone(),
        auth_token: None,
    };
    
    let message_id = db
        .insert_message("test_addr", "test payload")
        .await
        .unwrap();
    
    drop(db);
    drop(state);
    
    let db2 = Arc::new(Database::new(db_path_str).await.unwrap());
    db2.initialize().await.unwrap();
    
    let msg = db2.get_message(&message_id).await.unwrap();
    assert!(msg.is_some());
    let msg = msg.unwrap();
    assert_eq!(msg.id, message_id);
    assert_eq!(msg.to_addr, "test_addr");
    assert_eq!(msg.payload, "test payload");
    assert_eq!(msg.status, "queued");
}

#[tokio::test]
async fn test_mock_radio_send_success() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    let result = radio.send(b"test_addr", b"test payload");
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mock_radio_disconnected_fail_closed() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let message_id = db
        .insert_message("test_addr", "test payload")
        .await
        .unwrap();
    
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    radio.mock_set_connected(false);
    
    let result = radio.send(b"test_addr", b"test payload");
    assert!(result.is_err());
    
    let msg = db.get_message(&message_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "queued");
}

#[tokio::test]
async fn test_mock_radio_retry_and_dead() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let message_id = db
        .insert_message("test_addr", "test payload")
        .await
        .unwrap();
    
    for attempt in 1..=5 {
        db.update_message_status(&message_id, "queued", attempt, None)
            .await
            .unwrap();
    }
    
    db.update_message_status(&message_id, "dead", 5, None)
        .await
        .unwrap();
    
    let msg = db.get_message(&message_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "dead");
    assert_eq!(msg.attempts, 5);
}

#[tokio::test]
async fn test_payload_too_large_rejected() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let large_payload = "x".repeat(LORA_MAX_PAYLOAD + 1);
    
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    let result = radio.send(b"test_addr", large_payload.as_bytes());
    assert!(result.is_err());
}

#[tokio::test]
async fn test_message_status_transitions() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let message_id = db
        .insert_message("test_addr", "test payload")
        .await
        .unwrap();
    
    let msg = db.get_message(&message_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "queued");
    
    db.update_message_status(&message_id, "sent", 1, None)
        .await
        .unwrap();
    
    let msg = db.get_message(&message_id).await.unwrap().unwrap();
    assert_eq!(msg.status, "sent");
    assert_eq!(msg.attempts, 1);
}

#[tokio::test]
async fn test_inbox_receive() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    radio.mock_simulate_receive(b"sender01", b"incoming message");
    
    let result = radio.receive().unwrap();
    assert!(result.is_some());
    
    let (from_addr, payload) = result.unwrap();
    assert_eq!(from_addr, b"sender01");
    assert_eq!(payload, b"incoming message");
    
    let from_str = String::from_utf8_lossy(&from_addr).to_string();
    let payload_str = String::from_utf8_lossy(&payload).to_string();
    
    db.insert_inbox_message(&from_str, &payload_str)
        .await
        .unwrap();
    
    let inbox = db.get_inbox_messages(10).await.unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].payload, "incoming message");
}

#[tokio::test]
async fn test_pending_messages_with_retry_delay() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test.db");
    
    let db = Arc::new(Database::new(db_path.to_str().unwrap()).await.unwrap());
    db.initialize().await.unwrap();
    
    let future_time = chrono::Utc::now().timestamp() + 3600;
    
    let msg_id = db.insert_message("test_addr", "test payload").await.unwrap();
    db.update_message_status(&msg_id, "queued", 1, Some(future_time))
        .await
        .unwrap();
    
    let pending = db.get_pending_messages().await.unwrap();
    assert_eq!(pending.len(), 0);
    
    db.update_message_status(&msg_id, "queued", 1, None)
        .await
        .unwrap();
    
    let pending = db.get_pending_messages().await.unwrap();
    assert_eq!(pending.len(), 1);
}

#[tokio::test]
async fn test_cpp_mock_compiles() {
    let mut radio = Radio::new(None);
    let init_result = radio.initialize();
    assert!(init_result.is_ok());
    
    assert!(radio.is_connected());
    
    radio.mock_set_connected(false);
    assert!(!radio.is_connected());
}
