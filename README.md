# httplora

HTTP in, durable queue, LoRa/Meshtastic out.

**Toy gateway.** Not Meshtastic. Not a production radio stack. Hardware is optional: a mock serial port so the tree compiles without a dongle.

## What is this?

httplora is a high-reliability asynchronous gateway that accepts HTTP POST requests from internet APIs and forwards them as radio packets over a LoRa dongle. Messages are persisted in a durable SQLite queue and retried with exponential backoff if transmission fails.

**This is a toy implementation for learning and experimentation.** It is not affiliated with Meshtastic, does not implement the full Meshtastic protocol stack, and is not production-ready. The radio framing is simplified for demonstration purposes.

## Architecture

### Stack

- **Rust**: HTTP server (axum), durable queue (SQLite), worker with retry/backoff logic, API
- **C++**: Radio dongle HAL (serial frames) with mock backend
- **FFI**: Rust calls C++ via cxx bridge

### Components

1. **HTTP API**: RESTful endpoints for sending messages, checking status, and reading inbox
2. **Durable Queue**: SQLite database persists messages across restarts
3. **Worker**: Background task that processes queued messages and handles retries
4. **Radio HAL**: Abstraction layer for radio hardware with mock implementation

### Frame Size Limit

The toy LoRa frame MTU is **200 bytes** for the payload. Larger payloads are rejected with a 400 error and not queued.

## Features

### Behavior

1. **POST /v1/messages**: Accept message with `{to, payload}` → return 202 with message ID, persist to SQLite, worker attempts radio transmission
2. **GET /v1/messages/{id}**: Check message status: `queued`, `sent`, `failed`, or `dead`
3. **GET /v1/inbox**: Retrieve received messages (from mock RX or real dongle)
4. **GET /health**: Health check endpoint, returns `ok` if process is running, plus radio status: `mock`, `missing`, or `up`
5. **Fail-closed**: If radio is disconnected, messages stay queued and are never marked as `sent` incorrectly
6. **Retry logic**: Exponential backoff with jitter (1s base, max 60s, 5 attempts before marking `dead`)
7. **Optional authentication**: Bearer token auth via `HTTPLORA_TOKEN` environment variable

## Building

### Prerequisites

- Rust (edition 2021)
- CMake 3.15+
- C++17 compiler (g++, clang++)

### Build Steps

```bash
# Build C++ radio HAL
cd radio
mkdir -p build && cd build
cmake ..
make
cd ../..

# Build Rust gateway
cargo build --release
```

The build system uses `cxx` for FFI and compiles the C++ radio HAL via CMake automatically when running `cargo build`.

## Running

### Environment Variables

- `HTTPLORA_DB_PATH`: Path to SQLite database (default: `httplora.db`)
- `HTTPLORA_DEVICE_PATH`: Path to serial device (optional, for real radio)
- `HTTPLORA_BIND_ADDR`: HTTP server bind address (default: `127.0.0.1:8080`)
- `HTTPLORA_TOKEN`: Bearer token for authentication (optional)
- `HTTPLORA_REAL_SERIAL`: Enable real serial backend (requires dongle)

### Start the Gateway

```bash
# With default settings (mock radio)
cargo run --release

# With custom settings
HTTPLORA_BIND_ADDR=0.0.0.0:8080 \
HTTPLORA_DB_PATH=/var/lib/httplora/messages.db \
HTTPLORA_TOKEN=my-secret-token \
cargo run --release
```

### Example API Usage

```bash
# Send a message
curl -X POST http://localhost:8080/v1/messages \
  -H "Content-Type: application/json" \
  -d '{"to": "node01", "payload": "Hello LoRa"}'
# Response: {"id":"550e8400-e29b-41d4-a716-446655440000","status":"queued"}

# Check message status
curl http://localhost:8080/v1/messages/550e8400-e29b-41d4-a716-446655440000
# Response: {"id":"...","status":"sent","attempts":1,"created_at":...,"updated_at":...}

# Check health
curl http://localhost:8080/health
# Response: {"status":"ok","radio":"mock"}

# Get inbox messages
curl http://localhost:8080/v1/inbox?limit=10
# Response: [{"id":1,"from_addr":"sender01","payload":"incoming","received_at":...}]

# With authentication
curl -X POST http://localhost:8080/v1/messages \
  -H "Authorization: Bearer my-secret-token" \
  -H "Content-Type: application/json" \
  -d '{"to": "node01", "payload": "Hello LoRa"}'
```

## Radio Backends

### Mock (Default)

The mock backend simulates a radio dongle without requiring hardware. It:
- Always reports as connected (unless explicitly disconnected via test API)
- Accepts send operations and returns success
- Supports simulating received messages for testing the full loop

This is the **default** backend and requires no hardware.

### Real Serial (Optional)

To use a real LoRa dongle:
1. Set `HTTPLORA_REAL_SERIAL=1`
2. Provide device path via `HTTPLORA_DEVICE_PATH` (e.g., `/dev/ttyUSB0`)
3. The gateway will fail-closed if the device is missing or disconnected

**Note**: Real serial support is a stub in this toy version. You would need to implement actual serial communication and LoRa framing for production use.

## Testing

The test suite includes:
- Durable persistence (restart simulation)
- Mock radio send success
- Fail-closed behavior when radio disconnected
- Retry logic with exponential backoff
- Dead letter queue after max retries
- Payload size validation
- Inbox receive flow
- C++ mock compilation via FFI

```bash
# Run tests (written but NOT executed per requirements)
cargo test
```

**Note**: Tests are written but not run during the build process. The owner is expected to run tests manually.

## Failure Modes

### Radio Disconnected

If the radio becomes unavailable:
- New send attempts return `RadioError::Disconnected`
- Messages remain in `queued` status (never marked `sent`)
- Worker retries with exponential backoff
- After 5 attempts, message is marked `dead`

### Database Unavailable

If SQLite is corrupted or unavailable:
- Server will fail to start
- No messages are accepted

### Oversized Payload

If payload exceeds 200 bytes:
- HTTP 400 error returned immediately
- Message is NOT queued

## License

MIT

## Limitations & Disclaimers

- **Not Meshtastic**: This does not implement the full Meshtastic protocol
- **Toy project**: Not suitable for production use
- **No real radio**: Default backend is a mock; real radio support is minimal
- **Simplified framing**: Does not implement proper LoRa modulation or frequency management
- **No encryption**: Messages are not encrypted by default
- **No delivery confirmation**: "Sent" status means transmitted, not acknowledged
- **Single node**: Not a mesh network

For production LoRa/Meshtastic applications, use the official Meshtastic firmware and clients.
