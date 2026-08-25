#pragma once

#include <cstdint>
#include <cstddef>
#include <string>

namespace httplora {

// Maximum payload size for a LoRa frame (toy MTU)
constexpr size_t LORA_MAX_PAYLOAD = 200;

enum class RadioStatus {
    OK = 0,
    DISCONNECTED = 1,
    DEVICE_ERROR = 2,
    INVALID_PAYLOAD = 3,
};

enum class RadioBackend {
    MOCK = 0,
    REAL_SERIAL = 1,
};

struct RadioFrame {
    uint8_t to_addr[8];       // Destination address (simplified)
    uint8_t payload[LORA_MAX_PAYLOAD];
    size_t payload_len;
};

// C++ interface for radio HAL
class RadioHAL {
public:
    virtual ~RadioHAL() = default;
    
    virtual RadioStatus initialize() = 0;
    virtual RadioStatus send_frame(const RadioFrame& frame) = 0;
    virtual RadioStatus receive_frame(RadioFrame& frame, bool& has_frame) = 0;
    virtual RadioBackend backend_type() const = 0;
    virtual bool is_connected() const = 0;
};

// Factory function
RadioHAL* create_radio_hal(const char* device_path);

} // namespace httplora
