# Evaluation Harness

## Overview

httplora includes an evaluation harness for comparing reliability protocols under simulated packet loss conditions.

**CRITICAL DISCLAIMER**: This evaluation uses **SIMULATED** packet loss in software. Results are NOT:
- Real RF measurements
- Field trial data
- Hardware-specific performance
- Representative of actual LoRa radio behavior

Use ONLY for protocol comparison (fire-and-forget vs ARQ) in controlled simulated conditions.

## Running the Evaluation

```bash
# Run full evaluation sweep
cargo test --test eval_lossy -- --nocapture

# Run single example
cargo test --test eval_lossy eval_single_run_example -- --nocapture
```

## What It Measures

The harness compares two modes:

1. **Fire-and-forget** (baseline): No ACKs, sender assumes success
2. **Stop-and-wait ARQ**: Wait for ACK, retry on timeout, fail after max attempts

### Metrics

- **Delivery rate**: Fraction of messages successfully delivered (%)
- **Mean latency**: Average time from send to ACK (ms)
- **Total retries**: Number of retransmissions across all messages
- **Dead count**: Messages that exceeded max retries

### Loss Rates

Default sweep: 0%, 10%, 30%, 50% packet loss (configurable, deterministic seed)

## Expected Results (NOT MEASURED)

**Owner must run the evaluation to obtain real numbers.**

Below is the table format the harness will populate. Numbers shown here are NOT real measurements.

### Simulated Channel Evaluation Results

**Configuration**:
- Messages per trial: 100
- Max retries (ARQ): 5
- ACK timeout: 100ms
- Deterministic seed: 42

| Loss Rate | Mode               | Delivered | Dead | Retries | Latency (ms) |
|-----------|--------------------|-----------| -----|---------|--------------|
| 0.0%      | Fire-and-forget    | NOT RUN   | -    | -       | -            |
| 0.0%      | Stop-and-wait ARQ  | NOT RUN   | -    | -       | -            |
| 10.0%     | Fire-and-forget    | NOT RUN   | -    | -       | -            |
| 10.0%     | Stop-and-wait ARQ  | NOT RUN   | -    | -       | -            |
| 30.0%     | Fire-and-forget    | NOT RUN   | -    | -       | -            |
| 30.0%     | Stop-and-wait ARQ  | NOT RUN   | -    | -       | -            |
| 50.0%     | Fire-and-forget    | NOT RUN   | -    | -       | -            |
| 50.0%     | Stop-and-wait ARQ  | NOT RUN   | -    | -       | -            |

**Owner**: Run the harness and fill in this table with your results.

## Interpretation Guidelines

### What You CAN Conclude

- ARQ provides higher delivery rate than fire-and-forget under packet loss
- ARQ increases latency due to retransmissions and ACK waits
- ARQ uses more retries (as expected)
- Fail-closed property: ARQ never claims success without ACK

### What You CANNOT Conclude

- ❌ Real LoRa radio performance
- ❌ Field deployment reliability
- ❌ Comparison with Meshtastic (different protocols, no hardware)
- ❌ Hardware-specific behavior
- ❌ RF propagation characteristics
- ❌ Battery life or power consumption

## Extending the Evaluation

To modify the evaluation parameters, edit `tests/eval_lossy.rs`:

```rust
// Loss rates to sweep
let loss_rates = vec![0.0, 0.1, 0.3, 0.5];

// Messages per trial
let num_messages = 100;

// Random seed (for reproducibility)
let seed = 42;
```

## Burst Loss Evaluation

The lossy channel also supports burst loss (correlated packet drops):

```rust
// Configure burst loss
radio.configure_lossy_channel(
    0.1,    // base drop probability
    5,      // delay_ms
    0.05,   // burst_loss_prob (5% chance of burst)
    3,      // burst_loss_count (drop 3 consecutive packets)
    seed,
);
```

This simulates environments where losses are correlated (e.g., interference, obstacles).

## Publication Note

This is a toy research prototype. It is:
- ❌ NOT published
- ❌ NOT peer-reviewed
- ❌ NOT affiliated with any conference (NeurIPS, NSDI, etc.)
- ❌ NOT a Meshtastic comparison (no real Meshtastic implementation)

If you use this for academic work, clearly label results as "simulated" and do NOT claim field measurements.
