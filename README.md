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

- `HTTPLORA_BIND_ADDR`: HTTP server bind address (default: `127.0.0.1:8080`)
- `HTTPLORA_DB_PATH`: Path to SQLite database (default: `httplora.db`)
- `HTTPLORA_TOKEN`: Bearer token for authentication (optional, if set then required on all non-health endpoints)
- `HTTPLORA_REAL_SERIAL`: Set to `1` to enable real serial backend instead of mock (default: unset, uses mock)
- `HTTPLORA_DEVICE_PATH`: Path to serial device (required when `HTTPLORA_REAL_SERIAL=1`, e.g., `/dev/ttyUSB0`)

### Start the Gateway

```bash
# Default: mock radio (no hardware, no RF)
cargo run --release

# With authentication
HTTPLORA_TOKEN=my-secret-token cargo run --release

# With real serial backend (requires LoRa dongle)
HTTPLORA_REAL_SERIAL=1 \
HTTPLORA_DEVICE_PATH=/dev/ttyUSB0 \
cargo run --release

# Full configuration example
HTTPLORA_BIND_ADDR=0.0.0.0:8080 \
HTTPLORA_DB_PATH=/var/lib/httplora/messages.db \
HTTPLORA_TOKEN=my-secret-token \
HTTPLORA_REAL_SERIAL=1 \
HTTPLORA_DEVICE_PATH=/dev/ttyUSB0 \
cargo run --release
```

**Health check shows backend type:**
- `{"status":"ok","radio":"mock"}` - Mock backend (no RF)
- `{"status":"ok","radio":"up"}` - Real serial connected
- `{"status":"ok","radio":"missing"}` - Real serial mode but device disconnected

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

httplora supports two backends: mock (default) and real serial (optional).

### Mock Backend (Default)

The mock backend simulates a radio dongle without requiring hardware. It:
- Always reports as connected (unless explicitly disconnected via test API)
- Accepts send operations and returns success
- Supports simulating received messages for testing the full loop
- **No actual RF transmission** - purely in-memory simulation

This is the **default** backend when `HTTPLORA_REAL_SERIAL` is not set. Requires no hardware.

**Use cases:**
- Development and testing without a dongle
- CI/CD pipelines
- Learning the HTTP API

### Real Serial Backend (POSIX)

The real serial backend communicates with an actual LoRa radio dongle via POSIX serial (termios).

**Configuration:**
- Serial: 115200 baud, 8N1 (8 data bits, no parity, 1 stop bit)
- Framing: Simple binary protocol with start marker, address, payload, checksum
- Device: Configurable via `HTTPLORA_DEVICE_PATH` (e.g., `/dev/ttyUSB0`, `/dev/ttyACM0`)

**Enabling real serial:**
```bash
export HTTPLORA_REAL_SERIAL=1
export HTTPLORA_DEVICE_PATH=/dev/ttyUSB0
cargo run --release
```

**Fail-closed behavior:**
- If `HTTPLORA_REAL_SERIAL=1` but device path is missing/empty → `is_connected()=false`
- If device cannot be opened (missing, permission denied, etc.) → `is_connected()=false`
- If send fails (device disconnected during operation) → returns `DISCONNECTED` error
- Messages are **never** marked as `sent` when the radio is disconnected
- Worker retries with exponential backoff, marks `dead` after 5 attempts

**Important limitations:**
- This is a **toy protocol**, not Meshtastic or any standard LoRa protocol
- No real Meshtastic mesh networking features
- Simple XOR checksum (not CRC)
- No encryption by default
- No delivery acknowledgments from remote nodes
- Not suitable for production use

**Frame format:**
```
[0xAA 0x55]           Start marker (2 bytes)
[addr_len]            Address length: 8 (1 byte)
[to_addr]             Destination address (8 bytes)
[payload_len]         Payload length, little-endian (2 bytes)
[payload]             Payload data (0-200 bytes)
[checksum]            XOR checksum (1 byte)
```

## Testing

The test suite includes:
- **Mock send → sent status**: Successful mock send transitions message to `sent`
- **Unplugged mock → retries then dead, never sent**: Disconnected mock retries 5 times, marks `dead`, never falsely marks `sent`
- **Real serial with missing device → disconnected**: `HTTPLORA_REAL_SERIAL=1` with invalid path fails-closed (disconnected)
- **Payload > 200 bytes → HTTP 400, not queued**: Oversized payloads rejected immediately, never enter queue
- **Durable queue survives reopening SQLite file**: Messages persist across database close/reopen (restart simulation)
- **Retry logic with exponential backoff**: Worker implements backoff with jitter
- **Inbox receive flow**: Mock RX can simulate incoming messages
- **C++ mock compilation via FFI**: Verifies FFI bridge works

```bash
# Run tests (owner responsibility - NOT run during build)
cargo test
```

**Note**: Tests are written but not executed automatically. The owner should run tests manually after build.

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

**This is a toy learning project. Not production-ready.**

- ❌ **Not Meshtastic**: Custom toy protocol, not compatible with Meshtastic mesh networks
- ❌ **Not a production radio stack**: No proper LoRa PHY/MAC layer implementation
- ❌ **Mock has no RF**: Default backend is pure simulation, no radio waves
- ❌ **Real serial is toy framing**: Simple binary protocol, not a standard
- ❌ **No encryption**: Messages transmitted in plaintext
- ❌ **No delivery ACKs**: "Sent" means transmitted to dongle, not received by remote node
- ❌ **No mesh networking**: Single gateway, no multi-hop routing
- ❌ **No frequency management**: Assumes radio firmware handles LoRa modulation
- ❌ **No range claims**: RF characteristics depend entirely on your hardware
- ❌ **No carrier-grade reliability**: SQLite queue + retry logic, but not hardened for critical systems

**What this IS useful for:**
- Learning HTTP → hardware gateway patterns
- Understanding durable queues and retry logic
- Prototyping IoT data collection workflows
- Teaching fail-closed error handling

**For production LoRa/Meshtastic:**
Use official Meshtastic firmware, devices, and clients. This project is not affiliated with or endorsed by Meshtastic.
