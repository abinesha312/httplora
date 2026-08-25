/// Evaluation harness for lossy channel experiments
/// Compares fire-and-forget vs stop-and-wait ARQ under simulated packet loss
/// 
/// IMPORTANT: This uses SIMULATED channel loss, NOT real RF measurements.
/// Results are for protocol comparison only, not field performance.

use httplora::db::Database;
use httplora::radio::Radio;
use httplora::reliability::{ReliabilityMode, StopAndWaitProtocol, ReliableFrame, FrameType};
use std::sync::Arc;
use std::time::Instant;
use tempfile::tempdir;
use tokio::sync::Mutex;

const ACK_TIMEOUT_MS: u64 = 100;
const MAX_RETRIES: u32 = 5;

struct EvalResult {
    mode: ReliabilityMode,
    loss_rate: f64,
    messages_sent: u32,
    messages_delivered: u32,
    messages_dead: u32,
    total_retries: u32,
    mean_latency_ms: f64,
}

impl EvalResult {
    fn delivery_rate(&self) -> f64 {
        if self.messages_sent == 0 {
            0.0
        } else {
            (self.messages_delivered as f64) / (self.messages_sent as f64)
        }
    }
}

/// Fire-and-forget transmission (baseline)
async fn send_fire_and_forget(
    radio: &Arc<Mutex<Radio>>,
    to_addr: &[u8],
    payload: &[u8],
) -> Result<u64, String> {
    let start = Instant::now();
    let mut radio_guard = radio.lock().await;
    
    match radio_guard.send(to_addr, payload) {
        Ok(_) => Ok(start.elapsed().as_millis() as u64),
        Err(e) => Err(format!("{:?}", e)),
    }
}

/// Stop-and-wait ARQ transmission with retries
async fn send_stop_and_wait(
    radio: &Arc<Mutex<Radio>>,
    protocol: &StopAndWaitProtocol,
    to_addr: &[u8],
    payload: &[u8],
) -> Result<(u64, u32), String> {
    let start = Instant::now();
    let seq_num = protocol.next_seq_num().await;
    
    let frame = ReliableFrame::new_data(seq_num, to_addr.to_vec(), payload.to_vec());
    let frame_bytes = frame.serialize();
    
    for attempt in 0..MAX_RETRIES {
        {
            let mut radio_guard = radio.lock().await;
            match radio_guard.send(to_addr, &frame_bytes) {
                Ok(_) => {
                    drop(radio_guard);
                    
                    // Simulate ACK reception (in real system, worker would process)
                    // For eval, we assume ACK arrives if packet wasn't dropped
                    let tx_count_before = {
                        let guard = radio.lock().await;
                        guard.get_tx_count()
                    };
                    
                    match protocol.wait_for_ack(seq_num).await {
                        Ok(true) => {
                            let latency = start.elapsed().as_millis() as u64;
                            return Ok((latency, attempt + 1));
                        }
                        Ok(false) | Err(_) => {
                            // Timeout or no ACK, retry
                            continue;
                        }
                    }
                }
                Err(_) => {
                    return Err("Radio disconnected".to_string());
                }
            }
        }
    }
    
    Err("Max retries exceeded".to_string())
}

/// Run evaluation for a given loss rate and mode
async fn run_eval(
    loss_rate: f64,
    mode: ReliabilityMode,
    num_messages: u32,
    seed: u32,
) -> EvalResult {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("eval.db");
    
    let mut radio = Radio::new(None);
    radio.initialize().unwrap();
    
    // Configure simulated lossy channel
    radio.configure_lossy_channel(loss_rate, 5, 0.0, 0, seed);
    
    let radio = Arc::new(Mutex::new(radio));
    let protocol = StopAndWaitProtocol::new(ACK_TIMEOUT_MS);
    
    let mut delivered = 0u32;
    let mut dead = 0u32;
    let mut total_retries = 0u32;
    let mut total_latency = 0u64;
    
    let to_addr = b"testnode";
    
    for i in 0..num_messages {
        let payload = format!("msg_{}", i);
        
        match mode {
            ReliabilityMode::FireAndForget => {
                match send_fire_and_forget(&radio, to_addr, payload.as_bytes()).await {
                    Ok(latency) => {
                        // Fire-and-forget doesn't know about drops
                        // Assume delivered (but may be lost)
                        delivered += 1;
                        total_latency += latency;
                    }
                    Err(_) => {
                        dead += 1;
                    }
                }
            }
            ReliabilityMode::StopAndWait => {
                // For ARQ, simulate ACK after successful TX
                match send_stop_and_wait(&radio, &protocol, to_addr, payload.as_bytes()).await {
                    Ok((latency, retries)) => {
                        delivered += 1;
                        total_retries += retries;
                        total_latency += latency;
                        
                        // Simulate ACK reception
                        let seq_num = i;
                        protocol.process_ack(seq_num).await;
                    }
                    Err(_) => {
                        dead += 1;
                    }
                }
            }
        }
    }
    
    // For fire-and-forget, actual delivery rate depends on loss
    // Check actual TX stats
    let radio_guard = radio.lock().await;
    let tx_count = radio_guard.get_tx_count();
    let tx_dropped = radio_guard.get_tx_dropped();
    drop(radio_guard);
    
    if mode == ReliabilityMode::FireAndForget {
        // Adjust delivered count based on actual drops
        let actual_delivered = tx_count - tx_dropped;
        delivered = actual_delivered.min(num_messages as u64) as u32;
    }
    
    let mean_latency = if delivered > 0 {
        (total_latency as f64) / (delivered as f64)
    } else {
        0.0
    };
    
    EvalResult {
        mode,
        loss_rate,
        messages_sent: num_messages,
        messages_delivered: delivered,
        messages_dead: dead,
        total_retries,
        mean_latency_ms: mean_latency,
    }
}

/// Main evaluation: sweep loss rates for both modes
#[tokio::test]
async fn eval_lossy_channel_sweep() {
    let loss_rates = vec![0.0, 0.1, 0.3, 0.5];
    let num_messages = 100;
    let seed = 42;
    
    println!("\n=== Lossy Channel Evaluation ===");
    println!("SIMULATED packet loss (NOT real RF measurements)");
    println!("Messages per trial: {}", num_messages);
    println!();
    
    println!("{:<15} {:<20} {:<12} {:<12} {:<12} {:<12}", 
             "Loss Rate", "Mode", "Delivered", "Dead", "Retries", "Latency (ms)");
    println!("{}", "-".repeat(90));
    
    for &loss_rate in &loss_rates {
        // Fire-and-forget baseline
        let ff_result = run_eval(loss_rate, ReliabilityMode::FireAndForget, num_messages, seed).await;
        println!("{:<15.1} {:<20} {:<12} {:<12} {:<12} {:<12.2}",
                 loss_rate * 100.0,
                 "Fire-and-forget",
                 ff_result.messages_delivered,
                 ff_result.messages_dead,
                 ff_result.total_retries,
                 ff_result.mean_latency_ms);
        
        // Stop-and-wait ARQ
        let arq_result = run_eval(loss_rate, ReliabilityMode::StopAndWait, num_messages, seed + 1).await;
        println!("{:<15.1} {:<20} {:<12} {:<12} {:<12} {:<12.2}",
                 loss_rate * 100.0,
                 "Stop-and-wait ARQ",
                 arq_result.messages_delivered,
                 arq_result.messages_dead,
                 arq_result.total_retries,
                 arq_result.mean_latency_ms);
        
        println!();
    }
    
    println!("Evaluation complete.");
    println!("Note: This is simulated channel loss, not real RF field measurements.");
}

#[tokio::test]
async fn eval_single_run_example() {
    println!("\nRunning single evaluation example (10% loss)...");
    
    let result = run_eval(0.1, ReliabilityMode::StopAndWait, 50, 12345).await;
    
    println!("Mode: {:?}", result.mode);
    println!("Loss rate: {:.1}%", result.loss_rate * 100.0);
    println!("Messages sent: {}", result.messages_sent);
    println!("Messages delivered: {}", result.messages_delivered);
    println!("Delivery rate: {:.2}%", result.delivery_rate() * 100.0);
    println!("Messages dead: {}", result.messages_dead);
    println!("Total retries: {}", result.total_retries);
    println!("Mean latency: {:.2} ms", result.mean_latency_ms);
}
