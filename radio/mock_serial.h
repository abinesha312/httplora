#pragma once

#include "radio_hal.h"
#include <vector>
#include <mutex>
#include <random>
#include <chrono>

namespace httplora {

// Simulated lossy channel configuration (NOT real RF)
struct LossyChannelConfig {
    double drop_probability;     // Probability of packet loss (0.0 to 1.0)
    uint32_t delay_ms;            // Simulated transmission delay in milliseconds
    double burst_loss_prob;       // Probability of entering burst loss state
    uint32_t burst_loss_count;    // Number of packets to drop in burst
    uint32_t random_seed;         // Deterministic seed for reproducibility
    
    LossyChannelConfig()
        : drop_probability(0.0),
          delay_ms(0),
          burst_loss_prob(0.0),
          burst_loss_count(0),
          random_seed(12345) {}
};

// Mock radio HAL for testing without hardware
// INCLUDES SIMULATED LOSSY CHANNEL (not real RF characteristics)
class MockRadioHAL : public RadioHAL {
public:
    MockRadioHAL();
    explicit MockRadioHAL(const LossyChannelConfig& config);
    ~MockRadioHAL() override = default;
    
    RadioStatus initialize() override;
    RadioStatus send_frame(const RadioFrame& frame) override;
    RadioStatus receive_frame(RadioFrame& frame, bool& has_frame) override;
    RadioBackend backend_type() const override { return RadioBackend::MOCK; }
    bool is_connected() const override { return connected_; }
    
    // Mock control methods (for testing)
    void set_connected(bool connected) { connected_ = connected; }
    void simulate_receive(const RadioFrame& frame);
    void configure_lossy_channel(const LossyChannelConfig& config);
    
    // Statistics for evaluation
    uint64_t get_tx_count() const { return tx_count_; }
    uint64_t get_tx_dropped() const { return tx_dropped_; }
    uint64_t get_rx_count() const { return rx_count_; }
    
private:
    bool connected_;
    std::mutex rx_mutex_;
    std::vector<RadioFrame> rx_queue_;
    
    // Lossy channel simulation
    LossyChannelConfig lossy_config_;
    std::mt19937 rng_;
    uint32_t burst_remaining_;
    
    // Statistics
    uint64_t tx_count_;
    uint64_t tx_dropped_;
    uint64_t rx_count_;
    
    bool should_drop_packet();
};

} // namespace httplora
