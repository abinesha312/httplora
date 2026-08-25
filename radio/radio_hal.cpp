#include "radio_hal.h"
#include "mock_serial.h"
#include <cstdlib>

namespace httplora {

// Factory: create appropriate radio HAL based on environment
// If HTTPLORA_REAL_SERIAL is set and device_path is valid, use real serial
// Otherwise, use mock (default)
RadioHAL* create_radio_hal(const char* device_path) {
    const char* use_real = std::getenv("HTTPLORA_REAL_SERIAL");
    
    if (use_real != nullptr && device_path != nullptr) {
        // Real serial support would go here (not implemented in toy version)
        // For now, fail-closed: return mock
        return new MockRadioHAL();
    }
    
    return new MockRadioHAL();
}

} // namespace httplora
