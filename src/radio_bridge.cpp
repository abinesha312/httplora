#include "httplora/src/radio_bridge.h"
#include "../radio/radio_hal.h"
#include "../radio/mock_serial.h"
#include <cstring>

namespace httplora {
namespace bridge {

struct RadioHandle {
    httplora::RadioHAL* hal;
};

extern "C" {

RadioHandle* create_radio(const char* device_path) {
    RadioHandle* handle = new RadioHandle;
    handle->hal = httplora::create_radio_hal(device_path);
    return handle;
}

void destroy_radio(RadioHandle* handle) {
    if (handle) {
        delete handle->hal;
        delete handle;
    }
}

uint32_t radio_initialize(RadioHandle* handle) {
    if (!handle || !handle->hal) {
        return static_cast<uint32_t>(httplora::RadioStatus::DEVICE_ERROR);
    }
    return static_cast<uint32_t>(handle->hal->initialize());
}

uint32_t radio_send(RadioHandle* handle, const uint8_t* to_addr, size_t to_addr_len,
                    const uint8_t* payload, size_t payload_len) {
    if (!handle || !handle->hal) {
        return static_cast<uint32_t>(httplora::RadioStatus::DEVICE_ERROR);
    }
    
    if (payload_len > httplora::LORA_MAX_PAYLOAD) {
        return static_cast<uint32_t>(httplora::RadioStatus::INVALID_PAYLOAD);
    }
    
    httplora::RadioFrame frame{};
    size_t addr_copy_len = (to_addr_len < 8) ? to_addr_len : 8;
    std::memcpy(frame.to_addr, to_addr, addr_copy_len);
    std::memcpy(frame.payload, payload, payload_len);
    frame.payload_len = payload_len;
    
    return static_cast<uint32_t>(handle->hal->send_frame(frame));
}

int32_t radio_receive(RadioHandle* handle, uint8_t* buffer, size_t buffer_len,
                      uint8_t* addr_out, size_t addr_len) {
    if (!handle || !handle->hal) {
        return -1;
    }
    
    httplora::RadioFrame frame{};
    bool has_frame = false;
    httplora::RadioStatus status = handle->hal->receive_frame(frame, has_frame);
    
    if (status != httplora::RadioStatus::OK || !has_frame) {
        return 0;
    }
    
    size_t copy_len = (frame.payload_len < buffer_len) ? frame.payload_len : buffer_len;
    std::memcpy(buffer, frame.payload, copy_len);
    
    if (addr_out && addr_len >= 8) {
        std::memcpy(addr_out, frame.to_addr, 8);
    }
    
    return static_cast<int32_t>(copy_len);
}

bool radio_is_connected(const RadioHandle* handle) {
    if (!handle || !handle->hal) {
        return false;
    }
    return handle->hal->is_connected();
}

uint32_t radio_backend_type(const RadioHandle* handle) {
    if (!handle || !handle->hal) {
        return static_cast<uint32_t>(httplora::RadioBackend::MOCK);
    }
    return static_cast<uint32_t>(handle->hal->backend_type());
}

void mock_radio_set_connected(RadioHandle* handle, bool connected) {
    if (!handle || !handle->hal) {
        return;
    }
    
    if (auto* mock = dynamic_cast<httplora::MockRadioHAL*>(handle->hal)) {
        mock->set_connected(connected);
    }
}

void mock_radio_simulate_receive(RadioHandle* handle, const uint8_t* from_addr, size_t addr_len,
                                  const uint8_t* payload, size_t payload_len) {
    if (!handle || !handle->hal) {
        return;
    }
    
    auto* mock = dynamic_cast<httplora::MockRadioHAL*>(handle->hal);
    if (!mock) {
        return;
    }
    
    httplora::RadioFrame frame{};
    size_t addr_copy_len = (addr_len < 8) ? addr_len : 8;
    std::memcpy(frame.to_addr, from_addr, addr_copy_len);
    std::memcpy(frame.payload, payload, payload_len);
    frame.payload_len = payload_len;
    
    mock->simulate_receive(frame);
}

} // extern "C"

} // namespace bridge
} // namespace httplora
