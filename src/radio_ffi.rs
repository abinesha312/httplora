use cxx::{type_id, ExternType};

pub const LORA_MAX_PAYLOAD: usize = 200;

#[cxx::bridge]
pub mod ffi {
    #[repr(u32)]
    enum RadioStatus {
        OK = 0,
        DISCONNECTED = 1,
        DEVICE_ERROR = 2,
        INVALID_PAYLOAD = 3,
    }
    
    #[repr(u32)]
    enum RadioBackend {
        MOCK = 0,
        REAL_SERIAL = 1,
    }
    
    unsafe extern "C++" {
        include!("httplora/src/radio_bridge.h");
        
        type RadioHandle;
        
        fn create_radio(device_path: &str) -> *mut RadioHandle;
        fn destroy_radio(handle: *mut RadioHandle);
        fn radio_initialize(handle: *mut RadioHandle) -> RadioStatus;
        fn radio_send(handle: *mut RadioHandle, to_addr: &[u8], payload: &[u8]) -> RadioStatus;
        fn radio_receive(handle: *mut RadioHandle, buffer: &mut [u8], addr_out: &mut [u8]) -> i32;
        fn radio_is_connected(handle: *const RadioHandle) -> bool;
        fn radio_backend_type(handle: *const RadioHandle) -> RadioBackend;
        
        // Mock control (for testing)
        fn mock_radio_set_connected(handle: *mut RadioHandle, connected: bool);
        fn mock_radio_simulate_receive(handle: *mut RadioHandle, from_addr: &[u8], payload: &[u8]);
        
        // Lossy channel simulation (SIMULATED, not real RF)
        fn mock_radio_configure_lossy_channel(
            handle: *mut RadioHandle,
            drop_prob: f64,
            delay_ms: u32,
            burst_loss_prob: f64,
            burst_loss_count: u32,
            seed: u32,
        );
        
        // Statistics
        fn mock_radio_get_tx_count(handle: *const RadioHandle) -> u64;
        fn mock_radio_get_tx_dropped(handle: *const RadioHandle) -> u64;
        fn mock_radio_get_rx_count(handle: *const RadioHandle) -> u64;
    }
}

pub use ffi::*;
