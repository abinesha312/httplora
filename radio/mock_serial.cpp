#include "mock_serial.h"
#include <cstring>
#include <thread>

namespace httplora {

MockRadioHAL::MockRadioHAL() 
    : connected_(true), 
      lossy_config_(),
      rng_(lossy_config_.random_seed),
      burst_remaining_(0),
      tx_count_(0),
      tx_dropped_(0),
      rx_count_(0) {}

MockRadioHAL::MockRadioHAL(const LossyChannelConfig& config)
    : connected_(true),
      lossy_config_(config),
      rng_(config.random_seed),
      burst_remaining_(0),
      tx_count_(0),
      tx_dropped_(0),
      rx_count_(0) {}

RadioStatus MockRadioHAL::initialize() {
    if (!connected_) {
        return RadioStatus::DISCONNECTED;
    }
    return RadioStatus::OK;
}

bool MockRadioHAL::should_drop_packet() {
    // Burst loss mode
    if (burst_remaining_ > 0) {
        burst_remaining_--;
        return true;
    }
    
    // Check if entering burst loss
    if (lossy_config_.burst_loss_prob > 0.0) {
        std::uniform_real_distribution<double> dist(0.0, 1.0);
        if (dist(rng_) < lossy_config_.burst_loss_prob) {
            burst_remaining_ = lossy_config_.burst_loss_count;
            if (burst_remaining_ > 0) {
                burst_remaining_--;
                return true;
            }
        }
    }
    
    // Regular packet loss
    if (lossy_config_.drop_probability > 0.0) {
        std::uniform_real_distribution<double> dist(0.0, 1.0);
        return dist(rng_) < lossy_config_.drop_probability;
    }
    
    return false;
}

RadioStatus MockRadioHAL::send_frame(const RadioFrame& frame) {
    if (!connected_) {
        return RadioStatus::DISCONNECTED;
    }
    
    if (frame.payload_len > LORA_MAX_PAYLOAD) {
        return RadioStatus::INVALID_PAYLOAD;
    }
    
    tx_count_++;
    
    // Simulate lossy channel (NOT real RF)
    if (should_drop_packet()) {
        tx_dropped_++;
        // Packet dropped - from sender's perspective, it succeeded
        // (we don't know about loss without ARQ)
        return RadioStatus::OK;
    }
    
    // Simulate transmission delay
    if (lossy_config_.delay_ms > 0) {
        std::this_thread::sleep_for(std::chrono::milliseconds(lossy_config_.delay_ms));
    }
    
    return RadioStatus::OK;
}

RadioStatus MockRadioHAL::receive_frame(RadioFrame& frame, bool& has_frame) {
    if (!connected_) {
        has_frame = false;
        return RadioStatus::DISCONNECTED;
    }
    
    std::lock_guard<std::mutex> lock(rx_mutex_);
    
    if (rx_queue_.empty()) {
        has_frame = false;
        return RadioStatus::OK;
    }
    
    frame = rx_queue_.front();
    rx_queue_.erase(rx_queue_.begin());
    has_frame = true;
    rx_count_++;
    
    return RadioStatus::OK;
}

void MockRadioHAL::simulate_receive(const RadioFrame& frame) {
    std::lock_guard<std::mutex> lock(rx_mutex_);
    rx_queue_.push_back(frame);
}

void MockRadioHAL::configure_lossy_channel(const LossyChannelConfig& config) {
    lossy_config_ = config;
    rng_.seed(config.random_seed);
    burst_remaining_ = 0;
}

} // namespace httplora
