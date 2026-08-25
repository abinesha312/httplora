#include "radio_hal.h"
#include "mock_serial.h"
#include "serial_posix.h"
#include <cstdlib>
#include <string>

namespace httplora {

// Factory: create appropriate radio HAL based on environment
// If HTTPLORA_REAL_SERIAL is set and device_path is valid, use real serial (fail-closed if device unavailable)
// Otherwise, use mock (default)
RadioHAL* create_radio_hal(const char* device_path) {
    const char* use_real = std::getenv("HTTPLORA_REAL_SERIAL");
    
    if (use_real != nullptr && std::string(use_real) == "1") {
        // Real serial mode: fail-closed if device_path is missing or empty
        if (device_path == nullptr || std::string(device_path).empty()) {
            // Create SerialRadioHAL with empty path - it will fail-closed (is_connected = false)
            return new SerialRadioHAL("");
        }
        // Create real serial HAL - if device doesn't exist, initialize() will fail and is_connected() returns false
        return new SerialRadioHAL(device_path);
    }
    
    // Default: mock backend
    return new MockRadioHAL();
}

} // namespace httplora
