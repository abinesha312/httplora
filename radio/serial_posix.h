#pragma once

#include "radio_hal.h"
#include <string>

namespace httplora {

// Real POSIX serial backend for actual LoRa dongles
class SerialRadioHAL : public RadioHAL {
public:
    explicit SerialRadioHAL(const std::string& device_path);
    ~SerialRadioHAL() override;
    
    RadioStatus initialize() override;
    RadioStatus send_frame(const RadioFrame& frame) override;
    RadioStatus receive_frame(RadioFrame& frame, bool& has_frame) override;
    RadioBackend backend_type() const override { return RadioBackend::REAL_SERIAL; }
    bool is_connected() const override { return fd_ >= 0; }
    
private:
    std::string device_path_;
    int fd_;
    
    bool open_serial_port();
    void close_serial_port();
    bool configure_serial_port();
    bool write_frame_to_serial(const RadioFrame& frame);
    bool read_frame_from_serial(RadioFrame& frame);
};

} // namespace httplora
