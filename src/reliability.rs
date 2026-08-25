use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

/// Reliability mode for message transmission
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReliabilityMode {
    /// Fire-and-forget: no ACKs, baseline mode
    FireAndForget,
    /// Stop-and-wait ARQ: wait for ACK before sending next message
    StopAndWait,
}

impl Default for ReliabilityMode {
    fn default() -> Self {
        ReliabilityMode::FireAndForget
    }
}

/// ACK frame structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AckFrame {
    pub seq_num: u32,
    pub ack: bool,
}

/// Pending ACK tracker for stop-and-wait ARQ
pub struct AckTracker {
    pending: Arc<Mutex<HashMap<u32, tokio::sync::oneshot::Sender<bool>>>>,
}

impl AckTracker {
    pub fn new() -> Self {
        AckTracker {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// Register a pending ACK for a sequence number
    pub async fn register(&self, seq_num: u32) -> tokio::sync::oneshot::Receiver<bool> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut pending = self.pending.lock().await;
        pending.insert(seq_num, tx);
        rx
    }
    
    /// Mark an ACK as received
    pub async fn receive_ack(&self, seq_num: u32) -> bool {
        let mut pending = self.pending.lock().await;
        if let Some(tx) = pending.remove(&seq_num) {
            let _ = tx.send(true);
            true
        } else {
            false
        }
    }
    
    /// Cancel a pending ACK (on timeout)
    pub async fn cancel(&self, seq_num: u32) {
        let mut pending = self.pending.lock().await;
        pending.remove(&seq_num);
    }
}

/// Stop-and-wait ARQ protocol state
pub struct StopAndWaitProtocol {
    seq_num: Arc<Mutex<u32>>,
    ack_tracker: AckTracker,
    timeout_ms: u64,
}

impl StopAndWaitProtocol {
    pub fn new(timeout_ms: u64) -> Self {
        StopAndWaitProtocol {
            seq_num: Arc::new(Mutex::new(0)),
            ack_tracker: AckTracker::new(),
            timeout_ms,
        }
    }
    
    /// Get next sequence number
    pub async fn next_seq_num(&self) -> u32 {
        let mut seq = self.seq_num.lock().await;
        let num = *seq;
        *seq = seq.wrapping_add(1);
        num
    }
    
    /// Wait for ACK with timeout
    pub async fn wait_for_ack(&self, seq_num: u32) -> Result<bool, String> {
        let rx = self.ack_tracker.register(seq_num).await;
        
        match timeout(Duration::from_millis(self.timeout_ms), rx).await {
            Ok(Ok(ack)) => Ok(ack),
            Ok(Err(_)) => Err("ACK channel closed".to_string()),
            Err(_) => {
                self.ack_tracker.cancel(seq_num).await;
                Err("ACK timeout".to_string())
            }
        }
    }
    
    /// Process received ACK
    pub async fn process_ack(&self, seq_num: u32) {
        self.ack_tracker.receive_ack(seq_num).await;
    }
}

/// Frame type for protocol discrimination
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Data = 0x01,
    Ack = 0x02,
}

impl FrameType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0x01 => Some(FrameType::Data),
            0x02 => Some(FrameType::Ack),
            _ => None,
        }
    }
}

/// Enhanced frame with sequence number and type
#[derive(Debug, Clone)]
pub struct ReliableFrame {
    pub frame_type: FrameType,
    pub seq_num: u32,
    pub to_addr: Vec<u8>,
    pub payload: Vec<u8>,
}

impl ReliableFrame {
    pub fn new_data(seq_num: u32, to_addr: Vec<u8>, payload: Vec<u8>) -> Self {
        ReliableFrame {
            frame_type: FrameType::Data,
            seq_num,
            to_addr,
            payload,
        }
    }
    
    pub fn new_ack(seq_num: u32, to_addr: Vec<u8>) -> Self {
        ReliableFrame {
            frame_type: FrameType::Ack,
            seq_num,
            to_addr,
            payload: Vec::new(),
        }
    }
    
    /// Serialize frame for transmission
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(self.frame_type as u8);
        buf.extend_from_slice(&self.seq_num.to_le_bytes());
        buf.extend_from_slice(&self.to_addr);
        if self.frame_type == FrameType::Data {
            buf.extend_from_slice(&self.payload);
        }
        buf
    }
    
    /// Deserialize frame from bytes
    pub fn deserialize(data: &[u8]) -> Option<Self> {
        if data.len() < 13 {
            return None;
        }
        
        let frame_type = FrameType::from_u8(data[0])?;
        let seq_num = u32::from_le_bytes([data[1], data[2], data[3], data[4]]);
        let to_addr = data[5..13].to_vec();
        let payload = if frame_type == FrameType::Data {
            data[13..].to_vec()
        } else {
            Vec::new()
        };
        
        Some(ReliableFrame {
            frame_type,
            seq_num,
            to_addr,
            payload,
        })
    }
}
