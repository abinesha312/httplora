#pragma once

#include "radio_hal.h"
#include <vector>
#include <mutex>

namespace httplora {

// Mock radio HAL for testing without hardware
class MockRadioHAL : public RadioHAL {
public:
    MockRadioHAL();
    ~MockRadioHAL() override = default;
    
    RadioStatus initialize() override;
    RadioStatus send_frame(const RadioFrame& frame) override;
    RadioStatus receive_frame(RadioFrame& frame, bool& has_frame) override;
    RadioBackend backend_type() const override { return RadioBackend::MOCK; }
    bool is_connected() const override { return connected_; }
    
    // Mock control methods (for testing)
    void set_connected(bool connected) { connected_ = connected; }
    void simulate_receive(const RadioFrame& frame);
    
private:
    bool connected_;
    std::mutex rx_mutex_;
    std::vector<RadioFrame> rx_queue_;
};

} // namespace httplora
