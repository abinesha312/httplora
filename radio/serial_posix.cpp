#include "serial_posix.h"
#include <fcntl.h>
#include <termios.h>
#include <unistd.h>
#include <cstring>
#include <cerrno>
#include <sys/select.h>

namespace httplora {

SerialRadioHAL::SerialRadioHAL(const std::string& device_path)
    : device_path_(device_path), fd_(-1) {}

SerialRadioHAL::~SerialRadioHAL() {
    close_serial_port();
}

RadioStatus SerialRadioHAL::initialize() {
    if (device_path_.empty()) {
        return RadioStatus::DISCONNECTED;
    }
    
    if (!open_serial_port()) {
        return RadioStatus::DISCONNECTED;
    }
    
    if (!configure_serial_port()) {
        close_serial_port();
        return RadioStatus::DEVICE_ERROR;
    }
    
    return RadioStatus::OK;
}

bool SerialRadioHAL::open_serial_port() {
    fd_ = open(device_path_.c_str(), O_RDWR | O_NOCTTY | O_NONBLOCK);
    if (fd_ < 0) {
        return false;
    }
    return true;
}

void SerialRadioHAL::close_serial_port() {
    if (fd_ >= 0) {
        close(fd_);
        fd_ = -1;
    }
}

bool SerialRadioHAL::configure_serial_port() {
    struct termios tty;
    std::memset(&tty, 0, sizeof(tty));
    
    if (tcgetattr(fd_, &tty) != 0) {
        return false;
    }
    
    // 115200 baud, 8N1
    cfsetospeed(&tty, B115200);
    cfsetispeed(&tty, B115200);
    
    tty.c_cflag &= ~PARENB;        // No parity
    tty.c_cflag &= ~CSTOPB;        // 1 stop bit
    tty.c_cflag &= ~CSIZE;
    tty.c_cflag |= CS8;            // 8 data bits
    tty.c_cflag &= ~CRTSCTS;       // No hardware flow control
    tty.c_cflag |= CREAD | CLOCAL; // Enable receiver, ignore modem control
    
    // Raw mode
    tty.c_lflag &= ~ICANON;
    tty.c_lflag &= ~ECHO;
    tty.c_lflag &= ~ECHOE;
    tty.c_lflag &= ~ECHONL;
    tty.c_lflag &= ~ISIG;
    
    tty.c_iflag &= ~(IXON | IXOFF | IXANY);
    tty.c_iflag &= ~(IGNBRK | BRKINT | PARMRK | ISTRIP | INLCR | IGNCR | ICRNL);
    
    tty.c_oflag &= ~OPOST;
    tty.c_oflag &= ~ONLCR;
    
    // Non-blocking read with timeout
    tty.c_cc[VTIME] = 1;
    tty.c_cc[VMIN] = 0;
    
    if (tcsetattr(fd_, TCSANOW, &tty) != 0) {
        return false;
    }
    
    return true;
}

RadioStatus SerialRadioHAL::send_frame(const RadioFrame& frame) {
    if (fd_ < 0) {
        return RadioStatus::DISCONNECTED;
    }
    
    if (frame.payload_len > LORA_MAX_PAYLOAD) {
        return RadioStatus::INVALID_PAYLOAD;
    }
    
    if (!write_frame_to_serial(frame)) {
        // Serial write failed - device may be disconnected
        close_serial_port();
        return RadioStatus::DISCONNECTED;
    }
    
    return RadioStatus::OK;
}

bool SerialRadioHAL::write_frame_to_serial(const RadioFrame& frame) {
    // Simple framing protocol:
    // [0xAA 0x55] (start marker)
    // [addr_len: 1 byte]
    // [to_addr: 8 bytes]
    // [payload_len: 2 bytes, little-endian]
    // [payload: N bytes]
    // [checksum: 1 byte, simple XOR]
    
    uint8_t buffer[512];
    size_t pos = 0;
    
    // Start marker
    buffer[pos++] = 0xAA;
    buffer[pos++] = 0x55;
    
    // Address length (fixed 8)
    buffer[pos++] = 8;
    
    // Destination address
    std::memcpy(&buffer[pos], frame.to_addr, 8);
    pos += 8;
    
    // Payload length (little-endian)
    uint16_t payload_len = static_cast<uint16_t>(frame.payload_len);
    buffer[pos++] = payload_len & 0xFF;
    buffer[pos++] = (payload_len >> 8) & 0xFF;
    
    // Payload
    std::memcpy(&buffer[pos], frame.payload, frame.payload_len);
    pos += frame.payload_len;
    
    // Checksum (XOR of all data bytes)
    uint8_t checksum = 0;
    for (size_t i = 2; i < pos; i++) {
        checksum ^= buffer[i];
    }
    buffer[pos++] = checksum;
    
    // Write to serial
    ssize_t written = write(fd_, buffer, pos);
    if (written < 0 || static_cast<size_t>(written) != pos) {
        return false;
    }
    
    // Flush output
    tcdrain(fd_);
    
    return true;
}

RadioStatus SerialRadioHAL::receive_frame(RadioFrame& frame, bool& has_frame) {
    has_frame = false;
    
    if (fd_ < 0) {
        return RadioStatus::DISCONNECTED;
    }
    
    if (read_frame_from_serial(frame)) {
        has_frame = true;
        return RadioStatus::OK;
    }
    
    return RadioStatus::OK;
}

bool SerialRadioHAL::read_frame_from_serial(RadioFrame& frame) {
    // Non-blocking read with timeout
    fd_set read_fds;
    FD_ZERO(&read_fds);
    FD_SET(fd_, &read_fds);
    
    struct timeval timeout;
    timeout.tv_sec = 0;
    timeout.tv_usec = 10000; // 10ms
    
    int ret = select(fd_ + 1, &read_fds, nullptr, nullptr, &timeout);
    if (ret <= 0) {
        return false;
    }
    
    uint8_t buffer[512];
    ssize_t bytes_read = read(fd_, buffer, sizeof(buffer));
    
    if (bytes_read < 14) { // Minimum frame size
        return false;
    }
    
    size_t pos = 0;
    
    // Look for start marker
    while (pos < static_cast<size_t>(bytes_read - 1)) {
        if (buffer[pos] == 0xAA && buffer[pos + 1] == 0x55) {
            break;
        }
        pos++;
    }
    
    if (pos >= static_cast<size_t>(bytes_read - 13)) {
        return false;
    }
    
    pos += 2; // Skip start marker
    
    // Address length
    uint8_t addr_len = buffer[pos++];
    if (addr_len != 8 || pos + 8 >= static_cast<size_t>(bytes_read)) {
        return false;
    }
    
    // Source address (stored in to_addr for simplicity)
    std::memcpy(frame.to_addr, &buffer[pos], 8);
    pos += 8;
    
    // Payload length
    if (pos + 2 >= static_cast<size_t>(bytes_read)) {
        return false;
    }
    uint16_t payload_len = buffer[pos] | (buffer[pos + 1] << 8);
    pos += 2;
    
    if (payload_len > LORA_MAX_PAYLOAD || pos + payload_len + 1 > static_cast<size_t>(bytes_read)) {
        return false;
    }
    
    // Payload
    std::memcpy(frame.payload, &buffer[pos], payload_len);
    frame.payload_len = payload_len;
    pos += payload_len;
    
    // Verify checksum
    uint8_t expected_checksum = buffer[pos];
    uint8_t calculated_checksum = 0;
    for (size_t i = 2; i < pos; i++) {
        calculated_checksum ^= buffer[i];
    }
    
    if (expected_checksum != calculated_checksum) {
        return false;
    }
    
    return true;
}

} // namespace httplora
