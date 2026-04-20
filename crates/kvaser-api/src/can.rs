use std::ffi::c_void;
use std::fmt;
use std::os::raw::{c_long, c_uint, c_ulong};
use std::str::FromStr;
use std::sync::Arc;

use libloading::Library;

use crate::error::{KvaserError, Result};
use crate::ffi::{c_buffer_to_string, load_library, load_symbol};

const CANLIB_LABEL: &str = "CANlib";
const CANLIB_CANDIDATES: &[&str] = &["libcanlib.so", "libcanlib.so.1"];

const CAN_OK: i32 = 0;
const CAN_ERR_NOMSG: i32 = -2;

const CAN_OPEN_REQUIRE_INIT_ACCESS: i32 = 0x0080;
const CAN_OPEN_ACCEPT_LARGE_DLC: i32 = 0x0200;

const CAN_DRIVER_NORMAL: c_uint = 4;

const CAN_CHANNELDATA_CHANNEL_CAP: i32 = 1;
const CAN_CHANNELDATA_CARD_NUMBER: i32 = 5;
const CAN_CHANNELDATA_CHAN_NO_ON_CARD: i32 = 6;
const CAN_CHANNELDATA_CARD_SERIAL_NO: i32 = 7;
const CAN_CHANNELDATA_CHANNEL_NAME: i32 = 13;
const CAN_CHANNELDATA_DEVDESCR_ASCII: i32 = 26;

const CAN_CHANNEL_CAP_CAN_FD: u32 = 0x0008_0000;
const CAN_CHANNEL_CAP_LOGGER: u32 = 0x0080_0000;

const CAN_MSG_RTR: u32 = 0x0001;
const CAN_MSG_EXT: u32 = 0x0004;
const CAN_MSG_ERROR_FRAME: u32 = 0x0020;
const CAN_MSG_TXACK: u32 = 0x0040;
const CAN_FDMSG_FDF: u32 = 0x010000;
const CAN_FDMSG_BRS: u32 = 0x020000;
const CAN_FDMSG_ESI: u32 = 0x040000;
const CAN_MSGERR_OVERRUN: u32 = 0x0600;

const KV_DEVICE_MODE_INTERFACE: i32 = 0x00;
const KV_DEVICE_MODE_LOGGER: i32 = 0x01;
const KV_SCRIPT_STOP_NORMAL: i32 = 0;

type CanStatus = i32;
type CanHandle = i32;

type CanInitializeLibrary = unsafe extern "C" fn();
type CanGetNumberOfChannels = unsafe extern "C" fn(*mut i32) -> CanStatus;
type CanOpenChannel = unsafe extern "C" fn(i32, i32) -> CanHandle;
type CanClose = unsafe extern "C" fn(CanHandle) -> CanStatus;
type CanBusOn = unsafe extern "C" fn(CanHandle) -> CanStatus;
type CanBusOff = unsafe extern "C" fn(CanHandle) -> CanStatus;
type CanRead = unsafe extern "C" fn(
    CanHandle,
    *mut c_long,
    *mut c_void,
    *mut c_uint,
    *mut c_uint,
    *mut c_ulong,
) -> CanStatus;
type CanSetBusParams =
    unsafe extern "C" fn(CanHandle, c_long, c_uint, c_uint, c_uint, c_uint, c_uint) -> CanStatus;
type CanSetBusOutputControl = unsafe extern "C" fn(CanHandle, c_uint) -> CanStatus;
type CanGetErrorText = unsafe extern "C" fn(CanStatus, *mut u8, c_uint) -> CanStatus;
type CanGetChannelData = unsafe extern "C" fn(i32, i32, *mut c_void, usize) -> CanStatus;
type KvDeviceSetMode = unsafe extern "C" fn(CanHandle, i32) -> CanStatus;
type KvDeviceGetMode = unsafe extern "C" fn(CanHandle, *mut i32) -> CanStatus;
type KvScriptStart = unsafe extern "C" fn(CanHandle, i32) -> CanStatus;
type KvScriptStop = unsafe extern "C" fn(CanHandle, i32, i32) -> CanStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CanBitrate {
    B10K,
    B50K,
    B62K,
    B83K,
    B100K,
    B125K,
    B250K,
    B500K,
    B1M,
    Custom(u32),
}

impl CanBitrate {
    pub const PRESETS: [CanBitrate; 2] = [CanBitrate::B500K, CanBitrate::B1M];

    pub fn as_bps(self) -> u32 {
        match self {
            CanBitrate::B10K => 10_000,
            CanBitrate::B50K => 50_000,
            CanBitrate::B62K => 62_000,
            CanBitrate::B83K => 83_000,
            CanBitrate::B100K => 100_000,
            CanBitrate::B125K => 125_000,
            CanBitrate::B250K => 250_000,
            CanBitrate::B500K => 500_000,
            CanBitrate::B1M => 1_000_000,
            CanBitrate::Custom(value) => value,
        }
    }

    pub fn as_canlib_freq(self) -> c_long {
        match self {
            CanBitrate::B1M => -1,
            CanBitrate::B500K => -2,
            CanBitrate::B250K => -3,
            CanBitrate::B125K => -4,
            CanBitrate::B100K => -5,
            CanBitrate::B62K => -6,
            CanBitrate::B50K => -7,
            CanBitrate::B83K => -8,
            CanBitrate::B10K => -9,
            CanBitrate::Custom(value) => value as c_long,
        }
    }
}

impl fmt::Display for CanBitrate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanBitrate::B10K => f.write_str("10k"),
            CanBitrate::B50K => f.write_str("50k"),
            CanBitrate::B62K => f.write_str("62k"),
            CanBitrate::B83K => f.write_str("83k"),
            CanBitrate::B100K => f.write_str("100k"),
            CanBitrate::B125K => f.write_str("125k"),
            CanBitrate::B250K => f.write_str("250k"),
            CanBitrate::B500K => f.write_str("500k"),
            CanBitrate::B1M => f.write_str("1M"),
            CanBitrate::Custom(value) => write!(f, "{value}"),
        }
    }
}

impl FromStr for CanBitrate {
    type Err = KvaserError;

    fn from_str(input: &str) -> Result<Self> {
        let normalized = input.trim().to_ascii_lowercase().replace('_', "");
        if normalized.is_empty() {
            return Err(KvaserError::InvalidBitrate(input.to_owned()));
        }

        let bps = if let Some(value) = normalized.strip_suffix("kbps") {
            value.parse::<u32>().ok().and_then(|v| v.checked_mul(1_000))
        } else if let Some(value) = normalized.strip_suffix('k') {
            value.parse::<u32>().ok().and_then(|v| v.checked_mul(1_000))
        } else if let Some(value) = normalized.strip_suffix("mbps") {
            value
                .parse::<u32>()
                .ok()
                .and_then(|v| v.checked_mul(1_000_000))
        } else if let Some(value) = normalized.strip_suffix('m') {
            value
                .parse::<u32>()
                .ok()
                .and_then(|v| v.checked_mul(1_000_000))
        } else {
            normalized.parse::<u32>().ok()
        };

        match bps {
            Some(10_000) => Ok(CanBitrate::B10K),
            Some(50_000) => Ok(CanBitrate::B50K),
            Some(62_000) | Some(62_500) => Ok(CanBitrate::B62K),
            Some(83_000) | Some(83_333) => Ok(CanBitrate::B83K),
            Some(100_000) => Ok(CanBitrate::B100K),
            Some(125_000) => Ok(CanBitrate::B125K),
            Some(250_000) => Ok(CanBitrate::B250K),
            Some(500_000) => Ok(CanBitrate::B500K),
            Some(1_000_000) => Ok(CanBitrate::B1M),
            Some(value) if value > 0 => Ok(CanBitrate::Custom(value)),
            _ => Err(KvaserError::InvalidBitrate(input.to_owned())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CanChannel {
    pub index: u32,
    pub name: Option<String>,
    pub description: Option<String>,
    pub card_number: Option<u32>,
    pub channel_on_card: Option<u32>,
    pub serial: Option<u64>,
    pub can_fd: bool,
    pub logger: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanDeviceMode {
    Interface,
    Logger,
    Unknown(i32),
}

impl CanDeviceMode {
    fn as_kv_code(self) -> i32 {
        match self {
            CanDeviceMode::Interface => KV_DEVICE_MODE_INTERFACE,
            CanDeviceMode::Logger => KV_DEVICE_MODE_LOGGER,
            CanDeviceMode::Unknown(value) => value,
        }
    }
}

impl From<i32> for CanDeviceMode {
    fn from(value: i32) -> Self {
        match value {
            KV_DEVICE_MODE_INTERFACE => CanDeviceMode::Interface,
            KV_DEVICE_MODE_LOGGER => CanDeviceMode::Logger,
            other => CanDeviceMode::Unknown(other),
        }
    }
}

impl fmt::Display for CanDeviceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CanDeviceMode::Interface => f.write_str("interface"),
            CanDeviceMode::Logger => f.write_str("logger"),
            CanDeviceMode::Unknown(value) => write!(f, "unknown({value})"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CanFrame {
    pub channel: u32,
    pub id: i64,
    pub dlc: u32,
    pub flags: u32,
    pub timestamp_ms: u64,
    pub data: Vec<u8>,
}

impl CanFrame {
    pub fn flag_labels(&self) -> Vec<&'static str> {
        let mut labels = Vec::new();
        if self.flags & CAN_MSG_EXT != 0 {
            labels.push("ext");
        }
        if self.flags & CAN_MSG_RTR != 0 {
            labels.push("rtr");
        }
        if self.flags & CAN_MSG_ERROR_FRAME != 0 {
            labels.push("err");
        }
        if self.flags & CAN_MSG_TXACK != 0 {
            labels.push("txack");
        }
        if self.flags & CAN_FDMSG_FDF != 0 {
            labels.push("fd");
        }
        if self.flags & CAN_FDMSG_BRS != 0 {
            labels.push("brs");
        }
        if self.flags & CAN_FDMSG_ESI != 0 {
            labels.push("esi");
        }
        if self.flags & CAN_MSGERR_OVERRUN != 0 {
            labels.push("overrun");
        }
        labels
    }
}

pub struct Canlib {
    _library: Library,
    can_get_number_of_channels: CanGetNumberOfChannels,
    can_open_channel: CanOpenChannel,
    can_close: CanClose,
    can_bus_on: CanBusOn,
    can_bus_off: CanBusOff,
    can_read: CanRead,
    can_set_bus_params: CanSetBusParams,
    can_set_bus_output_control: CanSetBusOutputControl,
    can_get_error_text: CanGetErrorText,
    can_get_channel_data: CanGetChannelData,
    kv_device_set_mode: KvDeviceSetMode,
    kv_device_get_mode: KvDeviceGetMode,
    kv_script_start: KvScriptStart,
    kv_script_stop: KvScriptStop,
}

impl Canlib {
    pub fn load() -> Result<Arc<Self>> {
        let (library, library_name) = load_library(CANLIB_LABEL, CANLIB_CANDIDATES)?;

        let can_initialize_library: CanInitializeLibrary =
            unsafe { load_symbol(&library, library_name, "canInitializeLibrary")? };
        let can_get_number_of_channels =
            unsafe { load_symbol(&library, library_name, "canGetNumberOfChannels")? };
        let can_open_channel = unsafe { load_symbol(&library, library_name, "canOpenChannel")? };
        let can_close = unsafe { load_symbol(&library, library_name, "canClose")? };
        let can_bus_on = unsafe { load_symbol(&library, library_name, "canBusOn")? };
        let can_bus_off = unsafe { load_symbol(&library, library_name, "canBusOff")? };
        let can_read = unsafe { load_symbol(&library, library_name, "canRead")? };
        let can_set_bus_params = unsafe { load_symbol(&library, library_name, "canSetBusParams")? };
        let can_set_bus_output_control =
            unsafe { load_symbol(&library, library_name, "canSetBusOutputControl")? };
        let can_get_error_text = unsafe { load_symbol(&library, library_name, "canGetErrorText")? };
        let can_get_channel_data =
            unsafe { load_symbol(&library, library_name, "canGetChannelData")? };
        let kv_device_set_mode = unsafe { load_symbol(&library, library_name, "kvDeviceSetMode")? };
        let kv_device_get_mode = unsafe { load_symbol(&library, library_name, "kvDeviceGetMode")? };
        let kv_script_start = unsafe { load_symbol(&library, library_name, "kvScriptStart")? };
        let kv_script_stop = unsafe { load_symbol(&library, library_name, "kvScriptStop")? };

        unsafe { can_initialize_library() };

        Ok(Arc::new(Self {
            _library: library,
            can_get_number_of_channels,
            can_open_channel,
            can_close,
            can_bus_on,
            can_bus_off,
            can_read,
            can_set_bus_params,
            can_set_bus_output_control,
            can_get_error_text,
            can_get_channel_data,
            kv_device_set_mode,
            kv_device_get_mode,
            kv_script_start,
            kv_script_stop,
        }))
    }

    pub fn list_channels(&self) -> Result<Vec<CanChannel>> {
        let mut count = 0;
        let status = unsafe { (self.can_get_number_of_channels)(&mut count) };
        self.ensure_status("canGetNumberOfChannels", status)?;

        let mut channels = Vec::with_capacity(count.max(0) as usize);
        for index in 0..count {
            let caps = self
                .query_u32(index, CAN_CHANNELDATA_CHANNEL_CAP)
                .unwrap_or_default();
            channels.push(CanChannel {
                index: index as u32,
                name: self.query_string(index, CAN_CHANNELDATA_CHANNEL_NAME),
                description: self.query_string(index, CAN_CHANNELDATA_DEVDESCR_ASCII),
                card_number: self.query_u32(index, CAN_CHANNELDATA_CARD_NUMBER),
                channel_on_card: self.query_u32(index, CAN_CHANNELDATA_CHAN_NO_ON_CARD),
                serial: self.query_u64(index, CAN_CHANNELDATA_CARD_SERIAL_NO),
                can_fd: caps & CAN_CHANNEL_CAP_CAN_FD != 0,
                logger: caps & CAN_CHANNEL_CAP_LOGGER != 0,
            });
        }

        Ok(channels)
    }

    pub fn open_on_bus(
        self: &Arc<Self>,
        channel: u32,
        bitrate: CanBitrate,
    ) -> Result<CanHandleGuard> {
        let flags = CAN_OPEN_REQUIRE_INIT_ACCESS | CAN_OPEN_ACCEPT_LARGE_DLC;
        let handle = unsafe { (self.can_open_channel)(channel as i32, flags) };
        if handle < 0 {
            return Err(self.status_error("canOpenChannel", handle));
        }

        let mut guard = CanHandleGuard {
            api: Arc::clone(self),
            handle,
            channel,
            on_bus: false,
        };

        let status =
            unsafe { (self.can_set_bus_params)(handle, bitrate.as_canlib_freq(), 0, 0, 0, 0, 0) };
        guard.api.ensure_status("canSetBusParams", status)?;

        let status = unsafe { (guard.api.can_set_bus_output_control)(handle, CAN_DRIVER_NORMAL) };
        guard.api.ensure_status("canSetBusOutputControl", status)?;

        let status = unsafe { (guard.api.can_bus_on)(handle) };
        guard.api.ensure_status("canBusOn", status)?;
        guard.on_bus = true;

        Ok(guard)
    }

    pub fn set_device_mode(&self, channel: u32, mode: CanDeviceMode) -> Result<()> {
        let handle = self.open_raw_handle(
            channel,
            CAN_OPEN_REQUIRE_INIT_ACCESS | CAN_OPEN_ACCEPT_LARGE_DLC,
        )?;
        let status = unsafe { (self.kv_device_set_mode)(handle, mode.as_kv_code()) };
        let close_status = unsafe { (self.can_close)(handle) };
        self.ensure_status("kvDeviceSetMode", status)?;
        self.ensure_status("canClose", close_status)
    }

    pub fn get_device_mode(&self, channel: u32) -> Result<CanDeviceMode> {
        let handle = self.open_raw_handle(channel, CAN_OPEN_ACCEPT_LARGE_DLC)?;
        let mut mode = 0_i32;
        let status = unsafe { (self.kv_device_get_mode)(handle, &mut mode) };
        let close_status = unsafe { (self.can_close)(handle) };
        self.ensure_status("kvDeviceGetMode", status)?;
        self.ensure_status("canClose", close_status)?;
        Ok(mode.into())
    }

    pub fn start_script(&self, channel: u32, slot: u32) -> Result<()> {
        let handle = self.open_raw_handle(channel, CAN_OPEN_ACCEPT_LARGE_DLC)?;
        let status = unsafe { (self.kv_script_start)(handle, slot as i32) };
        let close_status = unsafe { (self.can_close)(handle) };
        self.ensure_status("kvScriptStart", status)?;
        self.ensure_status("canClose", close_status)
    }

    pub fn stop_script(&self, channel: u32, slot: u32) -> Result<()> {
        let handle = self.open_raw_handle(channel, CAN_OPEN_ACCEPT_LARGE_DLC)?;
        let status = unsafe { (self.kv_script_stop)(handle, slot as i32, KV_SCRIPT_STOP_NORMAL) };
        let close_status = unsafe { (self.can_close)(handle) };
        self.ensure_status("kvScriptStop", status)?;
        self.ensure_status("canClose", close_status)
    }

    fn open_raw_handle(&self, channel: u32, flags: i32) -> Result<CanHandle> {
        let handle = unsafe { (self.can_open_channel)(channel as i32, flags) };
        if handle < 0 {
            return Err(self.status_error("canOpenChannel", handle));
        }
        Ok(handle)
    }

    fn query_string(&self, channel: i32, item: i32) -> Option<String> {
        let mut buffer = [0_u8; 256];
        let status = unsafe {
            (self.can_get_channel_data)(
                channel,
                item,
                buffer.as_mut_ptr().cast::<c_void>(),
                buffer.len(),
            )
        };
        if status != CAN_OK {
            return None;
        }

        let value = unsafe { c_buffer_to_string(&buffer) };
        (!value.is_empty()).then_some(value)
    }

    fn query_u32(&self, channel: i32, item: i32) -> Option<u32> {
        let mut value = 0_u32;
        let status = unsafe {
            (self.can_get_channel_data)(
                channel,
                item,
                (&mut value as *mut u32).cast::<c_void>(),
                std::mem::size_of::<u32>(),
            )
        };
        (status == CAN_OK).then_some(value)
    }

    fn query_u64(&self, channel: i32, item: i32) -> Option<u64> {
        let mut value = 0_u64;
        let status = unsafe {
            (self.can_get_channel_data)(
                channel,
                item,
                (&mut value as *mut u64).cast::<c_void>(),
                std::mem::size_of::<u64>(),
            )
        };
        (status == CAN_OK && value != 0).then_some(value)
    }

    fn ensure_status(&self, operation: &'static str, status: CanStatus) -> Result<()> {
        if status == CAN_OK {
            return Ok(());
        }
        Err(self.status_error(operation, status))
    }

    fn status_error(&self, operation: &'static str, status: CanStatus) -> KvaserError {
        KvaserError::Status {
            library: CANLIB_LABEL,
            operation,
            code: status,
            message: self.error_text(status),
        }
    }

    fn error_text(&self, status: CanStatus) -> String {
        let mut buffer = [0_u8; 256];
        let text_status =
            unsafe { (self.can_get_error_text)(status, buffer.as_mut_ptr(), buffer.len() as u32) };
        if text_status == CAN_OK {
            let text = unsafe { c_buffer_to_string(&buffer) };
            if !text.is_empty() {
                return text;
            }
        }
        format!("status {status}")
    }
}

pub struct CanHandleGuard {
    api: Arc<Canlib>,
    handle: CanHandle,
    channel: u32,
    on_bus: bool,
}

impl CanHandleGuard {
    pub fn channel(&self) -> u32 {
        self.channel
    }

    pub fn read_available(&self, limit: usize) -> Result<Vec<CanFrame>> {
        let mut frames = Vec::new();
        for _ in 0..limit {
            let mut id = 0 as c_long;
            let mut data = [0_u8; 64];
            let mut dlc = 0 as c_uint;
            let mut flags = 0 as c_uint;
            let mut timestamp = 0 as c_ulong;
            let status = unsafe {
                (self.api.can_read)(
                    self.handle,
                    &mut id,
                    data.as_mut_ptr().cast::<c_void>(),
                    &mut dlc,
                    &mut flags,
                    &mut timestamp,
                )
            };

            match status {
                CAN_OK => {
                    let len = (dlc as usize).min(data.len());
                    frames.push(CanFrame {
                        channel: self.channel,
                        id: id as i64,
                        dlc: dlc as u32,
                        flags: flags as u32,
                        timestamp_ms: timestamp as u64,
                        data: data[..len].to_vec(),
                    });
                }
                CAN_ERR_NOMSG => break,
                other => return Err(self.api.status_error("canRead", other)),
            }
        }
        Ok(frames)
    }
}

impl Drop for CanHandleGuard {
    fn drop(&mut self) {
        if self.on_bus {
            let _ = unsafe { (self.api.can_bus_off)(self.handle) };
        }
        let _ = unsafe { (self.api.can_close)(self.handle) };
    }
}
