use crate::db::Database;
use crate::radio::{Radio, RadioError, SharedRadio, LORA_MAX_PAYLOAD};
use rand::Rng;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

const MAX_RETRY_ATTEMPTS: i32 = 5;
const BASE_RETRY_DELAY_MS: u64 = 1000;
const MAX_RETRY_DELAY_MS: u64 = 60000;

pub struct Worker {
    db: Arc<Database>,
    radio: SharedRadio,
}

impl Worker {
    pub fn new(db: Arc<Database>, radio: SharedRadio) -> Self {
        Worker { db, radio }
    }
    
    pub async fn run(self) {
        info!("Worker started");
        
        loop {
            if let Err(e) = self.process_pending_messages().await {
                error!("Worker error: {}", e);
            }
            
            if let Err(e) = self.process_incoming_messages().await {
                error!("Worker receive error: {}", e);
            }
            
            sleep(Duration::from_millis(500)).await;
        }
    }
    
    async fn process_pending_messages(&self) -> Result<(), Box<dyn std::error::Error>> {
        let messages = self.db.get_pending_messages().await?;
        
        for msg in messages {
            match self.send_message(&msg.id, &msg.to_addr, &msg.payload, msg.attempts).await {
                Ok(true) => {
                    info!("Message {} sent successfully", msg.id);
                    self.db.update_message_status(&msg.id, "sent", msg.attempts + 1, None).await?;
                }
                Ok(false) => {
                    let attempts = msg.attempts + 1;
                    
                    if attempts >= MAX_RETRY_ATTEMPTS {
                        warn!("Message {} exceeded max retries, marking as dead", msg.id);
                        self.db.update_message_status(&msg.id, "dead", attempts, None).await?;
                    } else {
                        let retry_delay = calculate_retry_delay(attempts);
                        let next_retry_at = chrono::Utc::now().timestamp() + retry_delay as i64;
                        
                        warn!("Message {} failed, will retry in {}ms (attempt {})", 
                              msg.id, retry_delay, attempts);
                        
                        self.db.update_message_status(
                            &msg.id, 
                            "queued", 
                            attempts, 
                            Some(next_retry_at)
                        ).await?;
                    }
                }
                Err(e) => {
                    error!("Error processing message {}: {}", msg.id, e);
                }
            }
        }
        
        Ok(())
    }
    
    async fn send_message(
        &self,
        msg_id: &str,
        to_addr: &str,
        payload: &str,
        attempt: i32,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let to_bytes = to_addr.as_bytes();
        let payload_bytes = payload.as_bytes();
        
        if payload_bytes.len() > LORA_MAX_PAYLOAD {
            error!("Message {} payload too large: {} bytes", msg_id, payload_bytes.len());
            return Ok(false);
        }
        
        let mut radio = self.radio.lock().await;
        
        match radio.send(to_bytes, payload_bytes) {
            Ok(()) => {
                info!("Radio send succeeded for message {} (attempt {})", msg_id, attempt + 1);
                Ok(true)
            }
            Err(RadioError::Disconnected) => {
                warn!("Radio disconnected, cannot send message {}", msg_id);
                Ok(false)
            }
            Err(RadioError::InvalidPayload) => {
                error!("Invalid payload for message {}", msg_id);
                Ok(false)
            }
            Err(e) => {
                error!("Radio error for message {}: {}", msg_id, e);
                Ok(false)
            }
        }
    }
    
    async fn process_incoming_messages(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut radio = self.radio.lock().await;
        
        match radio.receive() {
            Ok(Some((from_addr, payload))) => {
                let from_str = String::from_utf8_lossy(&from_addr).to_string();
                let payload_str = String::from_utf8_lossy(&payload).to_string();
                
                info!("Received message from {}", from_str);
                self.db.insert_inbox_message(&from_str, &payload_str).await?;
            }
            Ok(None) => {}
            Err(e) => {
                warn!("Radio receive error: {}", e);
            }
        }
        
        Ok(())
    }
}

fn calculate_retry_delay(attempt: i32) -> u64 {
    let base = BASE_RETRY_DELAY_MS * 2_u64.pow((attempt - 1).max(0) as u32);
    let delay = base.min(MAX_RETRY_DELAY_MS);
    
    let jitter = rand::thread_rng().gen_range(0..delay / 4);
    
    delay + jitter
}
