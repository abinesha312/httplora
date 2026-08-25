#pragma once

#include <cstdint>
#include <cstddef>

namespace httplora {
namespace bridge {

// Opaque handle type for Rust
struct RadioHandle;

// C-compatible bridge functions
extern "C" {
    RadioHandle* create_radio(const char* device_path);
    void destroy_radio(RadioHandle* handle);
    uint32_t radio_initialize(RadioHandle* handle);
    uint32_t radio_send(RadioHandle* handle, const uint8_t* to_addr, size_t to_addr_len, 
                        const uint8_t* payload, size_t payload_len);
    int32_t radio_receive(RadioHandle* handle, uint8_t* buffer, size_t buffer_len,
                          uint8_t* addr_out, size_t addr_len);
    bool radio_is_connected(const RadioHandle* handle);
    uint32_t radio_backend_type(const RadioHandle* handle);
    
    // Mock control
    void mock_radio_set_connected(RadioHandle* handle, bool connected);
    void mock_radio_simulate_receive(RadioHandle* handle, const uint8_t* from_addr, size_t addr_len,
                                      const uint8_t* payload, size_t payload_len);
}

} // namespace bridge
} // namespace httplora
