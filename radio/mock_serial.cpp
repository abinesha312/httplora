#include "mock_serial.h"
#include <cstring>

namespace httplora {

MockRadioHAL::MockRadioHAL() : connected_(true) {}

RadioStatus MockRadioHAL::initialize() {
    if (!connected_) {
        return RadioStatus::DISCONNECTED;
    }
    return RadioStatus::OK;
}

RadioStatus MockRadioHAL::send_frame(const RadioFrame& frame) {
    if (!connected_) {
        return RadioStatus::DISCONNECTED;
    }
    
    if (frame.payload_len > LORA_MAX_PAYLOAD) {
        return RadioStatus::INVALID_PAYLOAD;
    }
    
    // Mock: just return OK
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
    
    return RadioStatus::OK;
}

void MockRadioHAL::simulate_receive(const RadioFrame& frame) {
    std::lock_guard<std::mutex> lock(rx_mutex_);
    rx_queue_.push_back(frame);
}

} // namespace httplora
