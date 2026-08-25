use crate::radio_ffi::{self, RadioBackend, RadioStatus};
use std::sync::Arc;
use thiserror::Error;

pub const LORA_MAX_PAYLOAD: usize = 200;

#[derive(Debug, Error)]
pub enum RadioError {
    #[error("Radio disconnected")]
    Disconnected,
    #[error("Device error")]
    DeviceError,
    #[error("Invalid payload (max {LORA_MAX_PAYLOAD} bytes)")]
    InvalidPayload,
    #[error("Radio error: {0}")]
    Other(String),
}

pub struct Radio {
    handle: *mut radio_ffi::RadioHandle,
}

unsafe impl Send for Radio {}
unsafe impl Sync for Radio {}

impl Radio {
    pub fn new(device_path: Option<&str>) -> Self {
        let path = device_path.unwrap_or("");
        let handle = radio_ffi::create_radio(path);
        Radio { handle }
    }
    
    pub fn initialize(&mut self) -> Result<(), RadioError> {
        let status = radio_ffi::radio_initialize(self.handle);
        match status {
            RadioStatus::OK => Ok(()),
            RadioStatus::DISCONNECTED => Err(RadioError::Disconnected),
            RadioStatus::DEVICE_ERROR => Err(RadioError::DeviceError),
            _ => Err(RadioError::Other("Unknown error".into())),
        }
    }
    
    pub fn send(&mut self, to: &[u8], payload: &[u8]) -> Result<(), RadioError> {
        if payload.len() > LORA_MAX_PAYLOAD {
            return Err(RadioError::InvalidPayload);
        }
        
        let status = radio_ffi::radio_send(self.handle, to, payload);
        match status {
            RadioStatus::OK => Ok(()),
            RadioStatus::DISCONNECTED => Err(RadioError::Disconnected),
            RadioStatus::INVALID_PAYLOAD => Err(RadioError::InvalidPayload),
            RadioStatus::DEVICE_ERROR => Err(RadioError::DeviceError),
            _ => Err(RadioError::Other("Unknown error".into())),
        }
    }
    
    pub fn receive(&mut self) -> Result<Option<(Vec<u8>, Vec<u8>)>, RadioError> {
        let mut buffer = vec![0u8; LORA_MAX_PAYLOAD];
        let mut addr = vec![0u8; 8];
        
        let len = radio_ffi::radio_receive(self.handle, &mut buffer, &mut addr);
        
        if len < 0 {
            return Err(RadioError::DeviceError);
        }
        
        if len == 0 {
            return Ok(None);
        }
        
        buffer.truncate(len as usize);
        Ok(Some((addr, buffer)))
    }
    
    pub fn is_connected(&self) -> bool {
        radio_ffi::radio_is_connected(self.handle)
    }
    
    pub fn backend_type(&self) -> RadioBackend {
        radio_ffi::radio_backend_type(self.handle)
    }
    
    pub fn mock_set_connected(&mut self, connected: bool) {
        radio_ffi::mock_radio_set_connected(self.handle, connected);
    }
    
    pub fn mock_simulate_receive(&mut self, from: &[u8], payload: &[u8]) {
        radio_ffi::mock_radio_simulate_receive(self.handle, from, payload);
    }
}

impl Drop for Radio {
    fn drop(&mut self) {
        radio_ffi::destroy_radio(self.handle);
    }
}

pub type SharedRadio = Arc<tokio::sync::Mutex<Radio>>;
