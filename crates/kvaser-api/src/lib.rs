#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(not(target_os = "linux"))]
compile_error!(
    "kvaser-api is Linux-only because the target tool uses Kvaser Linux SDK and SocketCAN workflows."
);

mod bindings;
pub mod can;
pub mod error;
mod ffi;
pub mod logs;

use std::sync::Arc;

pub use can::{CanBitrate, CanChannel, CanDeviceMode, CanFrame, CanHandleGuard, Canlib};
pub use error::{KvaserError, Result};
pub use logs::{
    DbcBinding, ExportProgress, ExportSummary, LogEventKind, LogEventPreview, LogFileInfo,
    LogFileType, LoggerCanChannelConfig, LoggerConfigSummary, Memorator, MemoratorFileSystem,
};

#[derive(Clone)]
pub struct KvaserRuntime {
    canlib: Arc<Canlib>,
    memorator: Memorator,
}

impl KvaserRuntime {
    pub fn load() -> Result<Self> {
        Ok(Self {
            canlib: Canlib::load()?,
            memorator: Memorator::load()?,
        })
    }

    pub fn canlib(&self) -> Arc<Canlib> {
        Arc::clone(&self.canlib)
    }

    pub fn memorator(&self) -> Memorator {
        self.memorator.clone()
    }
}
