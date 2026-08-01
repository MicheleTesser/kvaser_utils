use std::collections::{HashMap, HashSet};
use std::ffi::{CString, c_void};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::raw::{c_char, c_long, c_uint};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use libloading::Library;

use crate::bindings;
use crate::can::CanBitrate;
use crate::error::{KvaserError, Result};
use crate::ffi::{c_buffer_to_string, load_library, load_symbol};

const KVMLIB_LABEL: &str = "kvmlib";
const KVLCLIB_LABEL: &str = "kvlclib";
const KVADBLIB_LABEL: &str = "kvadblib";
const KVAMEMOLIBXML_LABEL: &str = "kvamemolibxml";

const KVMLIB_CANDIDATES: &[&str] = &["libkvmlib.so", "libkvmlib.so.1"];
const KVLCLIB_CANDIDATES: &[&str] = &["libkvlclib.so", "libkvlclib.so.1"];
const KVADBLIB_CANDIDATES: &[&str] = &["libkvadblib.so", "libkvadblib.so.1"];
const KVAMEMOLIBXML_CANDIDATES: &[&str] = &["libkvamemolibxml.so", "libkvamemolibxml.so.1"];

const KVM_OK: i32 = 0;
const KVM_ERR_NOLOGMSG: i32 = -10;
const KVM_EOF: i32 = -12;
const KVM_DEVICE_MHYDRA_EXT: i32 = 1;
const KVM_FS_FAT16: i32 = 0;
const KVM_FS_FAT32: i32 = 1;

const KVM_LOG_FILE_TYPE_ERR: i32 = 0;
const KVM_LOG_FILE_TYPE_ALL: i32 = 1;

const KVM_LOG_TYPE_INVALID: u32 = bindings::kvmLOG_TYPE_INVALID;
const KVM_LOG_TYPE_CLOCK: u32 = bindings::kvmLOG_TYPE_CLOCK;
const KVM_LOG_TYPE_MSG: u32 = bindings::kvmLOG_TYPE_MSG;
const KVM_LOG_TYPE_TRIGGER: u32 = bindings::kvmLOG_TYPE_TRIGGER;
const KVM_LOG_TYPE_VERSION: u32 = bindings::kvmLOG_TYPE_VERSION;
const KVM_LOG_TYPE_LINMSG: u32 = bindings::kvmLOG_TYPE_LINMSG;

const KVLC_OK: i32 = 0;
const KVLC_ERR_NOT_IMPLEMENTED: i32 = -4;
const KVLC_ERR_PARAM: i32 = -2;
const KVLC_ERR_DECREASING_TIME: i32 = -32;
const KVLC_FILE_FORMAT_MEMO_LOG: i32 = 6;
const KVLC_FILE_FORMAT_MDF_4X: i32 = 107;
const KVLC_FILE_FORMAT_MDF_4X_SIGNAL: i32 = 108;
const KVLC_PROPERTY_START_OF_MEASUREMENT: u32 = 1;
const KVLC_PROPERTY_CREATION_DATE: u32 = 27;
const KVLC_PROPERTY_OVERWRITE: u32 = 28;
const KVLC_PROPERTY_SIGNAL_BASED: u32 = 1001;

const KVADB_OK: i32 = 0;
const KVADB_ERR_DB_FILE_PARSE: i32 = -15;
const KVAXML_OK: i32 = 0;
const PARAM_FILE_SIZE: usize = 320 * 32 * 1024;
static DBC_TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

type KvmStatus = i32;
type KvmHandle = *mut c_void;
type KvlcStatus = i32;
type KvlcHandle = *mut c_void;
type KvaDbStatus = i32;
type KvaDbHnd = bindings::KvaDbHnd;
type KvaXmlStatus = i32;
type KvaXmlValidationStatus = i32;

type KvmInitialize = unsafe extern "C" fn();
type KvmGetErrorText = unsafe extern "C" fn(KvmStatus, *mut u8, usize) -> KvmStatus;
type KvmClose = unsafe extern "C" fn(KvmHandle) -> KvmStatus;
type KvmDeviceOpen = unsafe extern "C" fn(i32, *mut KvmStatus, i32) -> KvmHandle;
type KvmDeviceMountKmfEx = unsafe extern "C" fn(KvmHandle, *mut i32, *mut i32) -> KvmStatus;
type KvmDeviceFormatDisk = unsafe extern "C" fn(KvmHandle, i32, u32, u32) -> KvmStatus;
type KvmKmfWriteConfig = unsafe extern "C" fn(KvmHandle, *mut c_void, usize) -> KvmStatus;
type KvmLogFileGetCount = unsafe extern "C" fn(KvmHandle, *mut u32) -> KvmStatus;
type KvmLogFileGetType = unsafe extern "C" fn(KvmHandle, u32, *mut i32) -> KvmStatus;
type KvmLogFileMount = unsafe extern "C" fn(KvmHandle, u32, *mut u32) -> KvmStatus;
type KvmLogFileMountEx = unsafe extern "C" fn(KvmHandle, u32, *mut i64) -> KvmStatus;
type KvmLogFileDismount = unsafe extern "C" fn(KvmHandle) -> KvmStatus;
type KvmLogFileGetStartTime = unsafe extern "C" fn(KvmHandle, *mut u32) -> KvmStatus;
type KvmLogFileGetEndTime = unsafe extern "C" fn(KvmHandle, *mut u32) -> KvmStatus;
type KvmLogFileGetCreatorSerial = unsafe extern "C" fn(KvmHandle, *mut u32) -> KvmStatus;
type KvmLogFileReadEvent = unsafe extern "C" fn(KvmHandle, *mut KvmLogEventEx) -> KvmStatus;

type KvlcGetErrorText = unsafe extern "C" fn(KvlcStatus, *mut u8, u32) -> KvlcStatus;
type KvlcCreateConverter = unsafe extern "C" fn(*mut KvlcHandle, *const i8, i32) -> KvlcStatus;
type KvlcDeleteConverter = unsafe extern "C" fn(KvlcHandle) -> KvlcStatus;
type KvlcFeedSelectFormat = unsafe extern "C" fn(KvlcHandle, i32) -> KvlcStatus;
type KvlcFeedLogEvent = unsafe extern "C" fn(KvlcHandle, *mut c_void) -> KvlcStatus;
type KvlcConvertEvent = unsafe extern "C" fn(KvlcHandle) -> KvlcStatus;
type KvlcSetProperty = unsafe extern "C" fn(KvlcHandle, u32, *mut c_void, u32) -> KvlcStatus;
type KvlcEventCountEx = unsafe extern "C" fn(KvlcHandle, *mut i64) -> KvlcStatus;
type KvlcGetOutputFilename = unsafe extern "C" fn(KvlcHandle, *mut c_char, i32) -> KvlcStatus;
type KvlcAddDatabaseFile = unsafe extern "C" fn(KvlcHandle, *const c_char, c_uint) -> KvlcStatus;
type KvlcAddDatabase = unsafe extern "C" fn(KvlcHandle, KvaDbHnd, c_uint) -> KvlcStatus;

type KvaDbOpen = unsafe extern "C" fn(*mut KvaDbHnd) -> KvaDbStatus;
type KvaDbCreate = unsafe extern "C" fn(KvaDbHnd, *const c_char, *const c_char) -> KvaDbStatus;
type KvaDbReadFile = unsafe extern "C" fn(KvaDbHnd, *const c_char) -> KvaDbStatus;
type KvaDbClose = unsafe extern "C" fn(KvaDbHnd) -> KvaDbStatus;
type KvaDbGetErrorText = unsafe extern "C" fn(KvaDbStatus, *mut c_char, usize) -> KvaDbStatus;
type KvaDbGetLastParseError = unsafe extern "C" fn(*mut c_char, *mut c_uint) -> KvaDbStatus;

type KvaXmlInitialize = unsafe extern "C" fn() -> KvaXmlStatus;
type KvaXmlGetLastError =
    unsafe extern "C" fn(*mut c_char, c_uint, *mut KvaXmlStatus) -> KvaXmlStatus;
type KvaXmlToBuffer = unsafe extern "C" fn(
    *const c_char,
    c_uint,
    *mut c_char,
    *mut c_uint,
    *mut c_long,
) -> KvaXmlStatus;
type KvaXmlValidate = unsafe extern "C" fn(*const c_char, c_uint) -> KvaXmlStatus;
type KvaXmlGetValidationStatusCount = unsafe extern "C" fn(*mut i32, *mut i32) -> KvaXmlStatus;
type KvaXmlGetValidationError =
    unsafe extern "C" fn(*mut KvaXmlValidationStatus, *mut c_char, c_uint) -> KvaXmlStatus;
type KvaXmlGetValidationWarning =
    unsafe extern "C" fn(*mut KvaXmlValidationStatus, *mut c_char, c_uint) -> KvaXmlStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFileType {
    ErrorFrames,
    AllFrames,
    Unknown(i32),
}

impl From<i32> for LogFileType {
    fn from(value: i32) -> Self {
        match value {
            KVM_LOG_FILE_TYPE_ERR => LogFileType::ErrorFrames,
            KVM_LOG_FILE_TYPE_ALL => LogFileType::AllFrames,
            other => LogFileType::Unknown(other),
        }
    }
}

impl std::fmt::Display for LogFileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogFileType::ErrorFrames => f.write_str("error"),
            LogFileType::AllFrames => f.write_str("all"),
            LogFileType::Unknown(value) => write!(f, "unknown({value})"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum MemoratorFileSystem {
    Fat16,
    #[default]
    Fat32,
}

impl MemoratorFileSystem {
    fn as_kvm_code(self) -> i32 {
        match self {
            MemoratorFileSystem::Fat16 => KVM_FS_FAT16,
            MemoratorFileSystem::Fat32 => KVM_FS_FAT32,
        }
    }
}

impl std::fmt::Display for MemoratorFileSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoratorFileSystem::Fat16 => f.write_str("fat16"),
            MemoratorFileSystem::Fat32 => f.write_str("fat32"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogFileInfo {
    pub index: u32,
    pub file_type: LogFileType,
    pub event_count: i64,
    pub start_time_utc: Option<u32>,
    pub end_time_utc: Option<u32>,
    pub creator_serial: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogEventKind {
    Invalid,
    RtcClock,
    CanMessage,
    Trigger,
    Version,
    LinMessage,
    Unknown(u32),
}

#[derive(Debug, Clone)]
pub struct LogEventPreview {
    pub kind: LogEventKind,
    pub timestamp_ns: Option<i64>,
    pub channel: Option<u32>,
    pub id: Option<u32>,
    pub dlc: Option<u32>,
    pub flags: Option<u32>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ExportSummary {
    pub output: PathBuf,
    pub outputs: Vec<PathBuf>,
    pub files: Vec<u32>,
    pub events: u64,
}

#[derive(Debug, Clone)]
pub struct DbcBinding {
    pub path: PathBuf,
    pub channel_mask: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ExportProgress {
    pub completed_events: u64,
    pub total_events: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoggerCanChannelConfig {
    pub channel: u32,
    pub bitrate: CanBitrate,
}

impl LoggerCanChannelConfig {
    pub fn new(channel: u32, bitrate: CanBitrate) -> Self {
        Self { channel, bitrate }
    }
}

#[derive(Debug, Clone)]
pub struct LoggerConfigSummary {
    pub device_channel: u32,
    pub channels: Vec<LoggerCanChannelConfig>,
    pub binary_size: usize,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct Memorator {
    kvmlib: Arc<Kvmlib>,
    kvlclib: Arc<Kvlclib>,
    kvadblib: Option<Arc<Kvadblib>>,
}

impl Memorator {
    pub fn load() -> Result<Self> {
        Ok(Self {
            kvmlib: Kvmlib::load()?,
            kvlclib: Kvlclib::load()?,
            kvadblib: Kvadblib::try_load(),
        })
    }

    pub fn open_device(&self, device_channel: u32) -> Result<MemoratorDevice> {
        let handle = self.open_device_handle(device_channel)?;

        let mut ldf_major = 0;
        let mut ldf_minor = 0;
        let status = unsafe {
            (self.kvmlib.kvm_device_mount_kmf_ex)(handle, &mut ldf_major, &mut ldf_minor)
        };
        if status != KVM_OK {
            let _ = unsafe { (self.kvmlib.kvm_close)(handle) };
            return Err(self.kvmlib.status_error("kvmDeviceMountKmfEx", status));
        }

        Ok(MemoratorDevice {
            kvmlib: Arc::clone(&self.kvmlib),
            kvlclib: Arc::clone(&self.kvlclib),
            kvadblib: self.kvadblib.clone(),
            handle,
            ldf_major,
            ldf_minor,
        })
    }

    pub fn format_disk(
        &self,
        device_channel: u32,
        file_system: MemoratorFileSystem,
        reserve_space_mb: u32,
        database_space_mb: u32,
    ) -> Result<()> {
        let handle = self.open_device_handle(device_channel)?;
        let status = unsafe {
            (self.kvmlib.kvm_device_format_disk)(
                handle,
                file_system.as_kvm_code(),
                reserve_space_mb,
                database_space_mb,
            )
        };
        let close_status = unsafe { (self.kvmlib.kvm_close)(handle) };

        self.kvmlib.ensure_status("kvmDeviceFormatDisk", status)?;
        self.kvmlib.ensure_status("kvmClose", close_status)
    }

    pub fn write_log_all_config(
        &self,
        device_channel: u32,
        channels: &[LoggerCanChannelConfig],
    ) -> Result<LoggerConfigSummary> {
        if channels.is_empty() {
            return Err(KvaserError::Config(
                "logger config needs at least one CAN channel".to_owned(),
            ));
        }

        let xml = build_log_all_xml(channels)?;
        let kvaxml = KvaMemoLibXml::load()?;
        let converted = kvaxml.xml_to_config_buffer(&xml)?;

        let device = self.open_device(device_channel)?;
        device.write_config_buffer(&converted.buffer)?;

        Ok(LoggerConfigSummary {
            device_channel,
            channels: channels.to_vec(),
            binary_size: converted.buffer.len(),
            warnings: converted.warnings,
        })
    }

    fn open_device_handle(&self, device_channel: u32) -> Result<KvmHandle> {
        let mut status = KVM_OK;
        let handle = unsafe {
            (self.kvmlib.kvm_device_open)(device_channel as i32, &mut status, KVM_DEVICE_MHYDRA_EXT)
        };
        if status != KVM_OK || handle.is_null() {
            return Err(self.kvmlib.status_error("kvmDeviceOpen", status));
        }
        Ok(handle)
    }
}

pub struct MemoratorDevice {
    kvmlib: Arc<Kvmlib>,
    kvlclib: Arc<Kvlclib>,
    kvadblib: Option<Arc<Kvadblib>>,
    handle: KvmHandle,
    pub ldf_major: i32,
    pub ldf_minor: i32,
}

impl MemoratorDevice {
    fn write_config_buffer(&self, buffer: &[u8]) -> Result<()> {
        let mut owned = buffer.to_vec();
        let status = unsafe {
            (self.kvmlib.kvm_kmf_write_config)(
                self.handle,
                owned.as_mut_ptr().cast::<c_void>(),
                owned.len(),
            )
        };
        self.kvmlib.ensure_status("kvmKmfWriteConfig", status)
    }

    pub fn list_logs(&self) -> Result<Vec<LogFileInfo>> {
        let count = self.log_count()?;
        let mut files = Vec::with_capacity(count as usize);
        for index in 0..count {
            files.push(self.read_log_info(index)?);
        }

        Ok(files)
    }

    pub fn list_logs_recent_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<LogFileInfo>, u32)> {
        let count = self.log_count()?;
        if limit == 0 || offset >= count {
            return Ok((Vec::new(), count));
        }

        let end = offset.saturating_add(limit).min(count);
        let mut files = Vec::with_capacity((end - offset) as usize);
        for position in offset..end {
            let index = count - 1 - position;
            files.push(self.read_log_info(index)?);
        }
        Ok((files, count))
    }

    pub fn preview_log(&self, index: u32, limit: usize) -> Result<Vec<LogEventPreview>> {
        let _mounted = self.mount_log(index)?;
        let mut previews = Vec::with_capacity(limit);

        while previews.len() < limit {
            let mut event = KvmLogEventEx::default();
            match self.read_event(&mut event)? {
                ReadEvent::Event => previews.push(event.preview()),
                ReadEvent::EndOfFile => break,
            }
        }

        Ok(previews)
    }

    pub fn export_mf4(
        &self,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<ExportSummary> {
        self.export_mf4_with_dbc(selected_logs, output, overwrite, &[])
    }

    pub fn export_mf4_with_dbc(
        &self,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
        dbc_files: &[DbcBinding],
    ) -> Result<ExportSummary> {
        self.export_mf4_with_progress_and_dbc(selected_logs, output, overwrite, dbc_files, |_| {})
    }

    pub fn export_mf4_with_progress<F>(
        &self,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
        on_progress: F,
    ) -> Result<ExportSummary>
    where
        F: FnMut(ExportProgress),
    {
        self.export_mf4_with_progress_and_dbc(selected_logs, output, overwrite, &[], on_progress)
    }

    pub fn export_mf4_with_progress_and_dbc<F>(
        &self,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
        dbc_files: &[DbcBinding],
        mut on_progress: F,
    ) -> Result<ExportSummary>
    where
        F: FnMut(ExportProgress),
    {
        let output = output.as_ref();
        if let Some(parent) = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }

        let export_order = self.resolve_export_order(selected_logs)?;
        let export_infos: Vec<LogFileInfo> = export_order
            .iter()
            .copied()
            .filter_map(|index| self.read_log_info(index).ok())
            .collect();
        let export_total_events = export_infos
            .iter()
            .map(|info| info.event_count.max(0) as u64)
            .sum::<u64>();
        let export_reference_time = export_infos
            .first()
            .and_then(|info| info.start_time_utc.or(info.end_time_utc));
        let mut part = 1_u32;
        let mut part_path = output_path_for_segment(output, export_reference_time, part, &[]);
        let mut converter = self.create_export_converter(
            &part_path,
            overwrite,
            dbc_files,
            export_reference_time,
        )?;
        let mut outputs = Vec::new();

        let mut total_events = 0_u64;
        let mut skipped_events = 0_u64;
        let mut completed_source_events = 0_u64;
        let mut last_timestamp_in_part: Option<i64> = None;

        on_progress(ExportProgress {
            completed_events: 0,
            total_events: export_total_events,
        });

        for (position, index) in export_order.iter().copied().enumerate() {
            let info = export_infos
                .get(position)
                .ok_or_else(|| KvaserError::Config(format!("missing log info for export index {index}")))?;
            let first_ts = self.first_event_timestamp(index)?;
            if let (Some(prev), Some(next_first)) = (last_timestamp_in_part, first_ts) {
                if next_first < prev {
                    let finalized = finalize_converter(converter, &part_path)?;
                    outputs.push(finalized);
                    part += 1;
                    let segment_reference_time = info.start_time_utc.or(info.end_time_utc);
                    part_path =
                        output_path_for_segment(output, segment_reference_time, part, &outputs);
                    converter = self.create_export_converter(
                        &part_path,
                        true,
                        dbc_files,
                        segment_reference_time.or(export_reference_time),
                    )?;
                    last_timestamp_in_part = None;
                }
            }

            let _mounted = self.mount_log(index)?;
            loop {
                let mut event = KvmLogEventEx::default();
                match self.read_event(&mut event)? {
                    ReadEvent::Event => {
                        if !event.is_supported_for_export() {
                            skipped_events += 1;
                            continue;
                        }
                        let mut memo_event = event.memo_event();
                        memo_event.normalize_for_export();
                        if let Err(error) = converter.feed_event_memo(&mut memo_event) {
                            if is_kvlc_parameter_error(&error, "kvlcFeedLogEvent") {
                                skipped_events += 1;
                                continue;
                            }
                            if is_kvlc_decreasing_time_error(&error, "kvlcFeedLogEvent") {
                                skipped_events += 1;
                                continue;
                            }
                            return Err(error);
                        }
                        converter.convert_event()?;
                        total_events += 1;
                        if let Some(ts) = event.timestamp_ns_for_export() {
                            last_timestamp_in_part = Some(
                                last_timestamp_in_part
                                    .map(|current| current.max(ts))
                                    .unwrap_or(ts),
                            );
                        }
                    }
                    ReadEvent::EndOfFile => break,
                }
            }
            // Keep a single continuous stream across selected logs so MDF viewers
            // don't split data into per-file channel groups.
            completed_source_events = completed_source_events.saturating_add(_mounted.event_count.max(0) as u64);
            on_progress(ExportProgress {
                completed_events: completed_source_events.min(export_total_events),
                total_events: export_total_events,
            });
        }
        outputs.push(finalize_converter(converter, &part_path)?);
        let output_path = outputs
            .first()
            .cloned()
            .unwrap_or_else(|| part_path.clone());
        let converted_events = total_events as i64;
        if !output_path.is_file() {
            return Err(KvaserError::Config(format!(
                "kvlclib conversion completed but output file was not created at {} (converted_events={converted_events}, input_events={total_events}, skipped_events={skipped_events})",
                output_path.display(),
            )));
        }

        Ok(ExportSummary {
            output: output_path,
            outputs,
            files: export_order,
            events: total_events,
        })
    }

    fn resolve_export_order(&self, selected_logs: &[u32]) -> Result<Vec<u32>> {
        let infos = self.list_logs()?;
        let mut ordered = Vec::with_capacity(selected_logs.len());
        for &index in selected_logs {
            let Some(info) = infos.iter().find(|info| info.index == index) else {
                return Err(KvaserError::Config(format!(
                    "selected log index {index} was not found on device"
                )));
            };
            ordered.push(info.clone());
        }

        // kvlclib MF4 conversion requires non-decreasing time across files.
        ordered.sort_by(|a, b| {
            a.start_time_utc
                .unwrap_or(0)
                .cmp(&b.start_time_utc.unwrap_or(0))
                .then_with(|| {
                    a.end_time_utc
                        .unwrap_or(0)
                        .cmp(&b.end_time_utc.unwrap_or(0))
                })
                .then_with(|| a.index.cmp(&b.index))
        });

        Ok(ordered.into_iter().map(|info| info.index).collect())
    }

    fn create_export_converter(
        &self,
        output: &Path,
        overwrite: bool,
        dbc_files: &[DbcBinding],
        reference_unix_time: Option<u32>,
    ) -> Result<Converter> {
        let mut converter = match self
            .kvlclib
            .create_converter(output, KVLC_FILE_FORMAT_MDF_4X_SIGNAL)
        {
            Ok(converter) => converter,
            Err(error)
                if is_kvlc_parameter_error(&error, "kvlcCreateConverter")
                    || is_kvlc_not_implemented_error(&error, "kvlcCreateConverter") =>
            {
                self.kvlclib
                    .create_converter(output, KVLC_FILE_FORMAT_MDF_4X)?
            }
            Err(error) => return Err(error),
        };
        if overwrite {
            converter.set_overwrite(true)?;
        }
        // Keep absolute MF4 timestamps anchored to logger wall clock.
        let _ = converter.set_start_of_measurement(true);
        if let Some(unix_time) = reference_unix_time {
            let _ = converter.set_creation_date(unix_time as i64);
        }
        // Some SDK builds still require this property even when output format is
        // MDF_4X_SIGNAL. Ignore unsupported status and continue.
        let _ = converter.set_signal_based(true);
        let mut ordered_dbc = dbc_files.to_vec();
        ordered_dbc.sort_by(|a, b| {
            a.channel_mask
                .cmp(&b.channel_mask)
                .then_with(|| a.path.cmp(&b.path))
        });
        for dbc in &ordered_dbc {
            let mut added = false;
            let prepared_dbc = PreparedDbcFile::new(&dbc.path)?;

            if let Some(kvadblib) = self.kvadblib.clone() {
                match with_silenced_stdio(|| {
                    kvadblib.open_database_file(prepared_dbc.path(), &dbc.path)
                }) {
                    Ok(db_handle) => {
                        match converter.add_database_handle(db_handle, dbc.channel_mask) {
                            Ok(()) => {
                                converter.track_database_handle(kvadblib, db_handle);
                                added = true;
                            }
                            Err(error)
                                if is_kvlc_not_implemented_error(&error, "kvlcAddDatabase") =>
                            {
                                kvadblib.close_database_handle(db_handle);
                            }
                            Err(error) => {
                                kvadblib.close_database_handle(db_handle);
                                return Err(error);
                            }
                        }
                    }
                    Err(error) => return Err(error),
                }
            }

            if !added {
                let _ = with_silenced_stdio(|| {
                    converter.add_database_file(prepared_dbc.path(), dbc.channel_mask)
                })?;
            }

            // Do not attach raw DBC files to the converter metadata: some Linux
            // SDK builds print parser warnings directly to terminal.
        }
        converter.feed_select_format(KVLC_FILE_FORMAT_MEMO_LOG)?;
        Ok(converter)
    }

    fn first_event_timestamp(&self, index: u32) -> Result<Option<i64>> {
        let _mounted = self.mount_log(index)?;
        loop {
            let mut event = KvmLogEventEx::default();
            match self.read_event(&mut event)? {
                ReadEvent::Event => {
                    if let Some(timestamp) = event.timestamp_ns_for_export() {
                        return Ok(Some(timestamp));
                    }
                }
                ReadEvent::EndOfFile => return Ok(None),
            }
        }
    }

    fn mount_log(&self, index: u32) -> Result<MountedLog<'_>> {
        let mut log_type = 0_i32;
        let type_status =
            unsafe { (self.kvmlib.kvm_log_file_get_type)(self.handle, index, &mut log_type) };
        self.kvmlib
            .ensure_status("kvmLogFileGetType", type_status)?;

        let mut event_count = 0_i64;
        let status =
            unsafe { (self.kvmlib.kvm_log_file_mount_ex)(self.handle, index, &mut event_count) };
        if status != KVM_OK {
            let mut short_count = 0_u32;
            let fallback_status =
                unsafe { (self.kvmlib.kvm_log_file_mount)(self.handle, index, &mut short_count) };
            if fallback_status == KVM_OK {
                event_count = i64::from(short_count);
            } else {
                return Err(self.kvmlib.status_error("kvmLogFileMountEx", status));
            }
        }
        Ok(MountedLog {
            device: self,
            event_count,
            mounted: true,
        })
    }

    fn log_count(&self) -> Result<u32> {
        let mut count = 0_u32;
        let status = unsafe { (self.kvmlib.kvm_log_file_get_count)(self.handle, &mut count) };
        self.kvmlib.ensure_status("kvmLogFileGetCount", status)?;
        Ok(count)
    }

    fn read_log_info(&self, index: u32) -> Result<LogFileInfo> {
        let mut raw_type = 0_i32;
        let status =
            unsafe { (self.kvmlib.kvm_log_file_get_type)(self.handle, index, &mut raw_type) };
        self.kvmlib.ensure_status("kvmLogFileGetType", status)?;

        let mounted = self.mount_log(index)?;
        let info = LogFileInfo {
            index,
            file_type: raw_type.into(),
            event_count: mounted.event_count,
            start_time_utc: self.get_optional_u32("kvmLogFileGetStartTime", |value| unsafe {
                (self.kvmlib.kvm_log_file_get_start_time)(self.handle, value)
            })?,
            end_time_utc: self.get_optional_u32("kvmLogFileGetEndTime", |value| unsafe {
                (self.kvmlib.kvm_log_file_get_end_time)(self.handle, value)
            })?,
            creator_serial: self.get_optional_u32(
                "kvmLogFileGetCreatorSerial",
                |value| unsafe {
                    (self.kvmlib.kvm_log_file_get_creator_serial)(self.handle, value)
                },
            )?,
        };
        drop(mounted);
        Ok(info)
    }

    fn read_event(&self, event: &mut KvmLogEventEx) -> Result<ReadEvent> {
        let status = unsafe { (self.kvmlib.kvm_log_file_read_event)(self.handle, event) };
        match status {
            KVM_OK => Ok(ReadEvent::Event),
            KVM_ERR_NOLOGMSG | KVM_EOF => Ok(ReadEvent::EndOfFile),
            other => Err(self.kvmlib.status_error("kvmLogFileReadEvent", other)),
        }
    }

    fn get_optional_u32(
        &self,
        operation: &'static str,
        call: impl FnOnce(*mut u32) -> KvmStatus,
    ) -> Result<Option<u32>> {
        let mut value = 0_u32;
        let status = call(&mut value);
        match status {
            KVM_OK => Ok(Some(value)),
            KVM_ERR_NOLOGMSG | KVM_EOF => Ok(None),
            other => Err(self.kvmlib.status_error(operation, other)),
        }
    }
}

impl Drop for MemoratorDevice {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            let _ = unsafe { (self.kvmlib.kvm_close)(self.handle) };
        }
    }
}

struct MountedLog<'a> {
    device: &'a MemoratorDevice,
    event_count: i64,
    mounted: bool,
}

impl Drop for MountedLog<'_> {
    fn drop(&mut self) {
        if self.mounted {
            let _ = unsafe { (self.device.kvmlib.kvm_log_file_dismount)(self.device.handle) };
        }
    }
}

enum ReadEvent {
    Event,
    EndOfFile,
}

struct Kvmlib {
    _library: Library,
    kvm_get_error_text: KvmGetErrorText,
    kvm_close: KvmClose,
    kvm_device_open: KvmDeviceOpen,
    kvm_device_mount_kmf_ex: KvmDeviceMountKmfEx,
    kvm_device_format_disk: KvmDeviceFormatDisk,
    kvm_kmf_write_config: KvmKmfWriteConfig,
    kvm_log_file_get_count: KvmLogFileGetCount,
    kvm_log_file_get_type: KvmLogFileGetType,
    kvm_log_file_mount: KvmLogFileMount,
    kvm_log_file_mount_ex: KvmLogFileMountEx,
    kvm_log_file_dismount: KvmLogFileDismount,
    kvm_log_file_get_start_time: KvmLogFileGetStartTime,
    kvm_log_file_get_end_time: KvmLogFileGetEndTime,
    kvm_log_file_get_creator_serial: KvmLogFileGetCreatorSerial,
    kvm_log_file_read_event: KvmLogFileReadEvent,
}

impl Kvmlib {
    fn load() -> Result<Arc<Self>> {
        let (library, library_name) = load_library(KVMLIB_LABEL, KVMLIB_CANDIDATES)?;

        let kvm_initialize: KvmInitialize =
            unsafe { load_symbol(&library, library_name, "kvmInitialize")? };
        let kvm_get_error_text = unsafe { load_symbol(&library, library_name, "kvmGetErrorText")? };
        let kvm_close = unsafe { load_symbol(&library, library_name, "kvmClose")? };
        let kvm_device_open = unsafe { load_symbol(&library, library_name, "kvmDeviceOpen")? };
        let kvm_device_mount_kmf_ex =
            unsafe { load_symbol(&library, library_name, "kvmDeviceMountKmfEx")? };
        let kvm_device_format_disk =
            unsafe { load_symbol(&library, library_name, "kvmDeviceFormatDisk")? };
        let kvm_kmf_write_config =
            unsafe { load_symbol(&library, library_name, "kvmKmfWriteConfig")? };
        let kvm_log_file_get_count =
            unsafe { load_symbol(&library, library_name, "kvmLogFileGetCount")? };
        let kvm_log_file_get_type =
            unsafe { load_symbol(&library, library_name, "kvmLogFileGetType")? };
        let kvm_log_file_mount = unsafe { load_symbol(&library, library_name, "kvmLogFileMount")? };
        let kvm_log_file_mount_ex =
            unsafe { load_symbol(&library, library_name, "kvmLogFileMountEx")? };
        let kvm_log_file_dismount =
            unsafe { load_symbol(&library, library_name, "kvmLogFileDismount")? };
        let kvm_log_file_get_start_time =
            unsafe { load_symbol(&library, library_name, "kvmLogFileGetStartTime")? };
        let kvm_log_file_get_end_time =
            unsafe { load_symbol(&library, library_name, "kvmLogFileGetEndTime")? };
        let kvm_log_file_get_creator_serial =
            unsafe { load_symbol(&library, library_name, "kvmLogFileGetCreatorSerial")? };
        let kvm_log_file_read_event =
            unsafe { load_symbol(&library, library_name, "kvmLogFileReadEvent")? };

        unsafe { kvm_initialize() };

        Ok(Arc::new(Self {
            _library: library,
            kvm_get_error_text,
            kvm_close,
            kvm_device_open,
            kvm_device_mount_kmf_ex,
            kvm_device_format_disk,
            kvm_kmf_write_config,
            kvm_log_file_get_count,
            kvm_log_file_get_type,
            kvm_log_file_mount,
            kvm_log_file_mount_ex,
            kvm_log_file_dismount,
            kvm_log_file_get_start_time,
            kvm_log_file_get_end_time,
            kvm_log_file_get_creator_serial,
            kvm_log_file_read_event,
        }))
    }

    fn ensure_status(&self, operation: &'static str, status: KvmStatus) -> Result<()> {
        if status == KVM_OK {
            return Ok(());
        }
        Err(self.status_error(operation, status))
    }

    fn status_error(&self, operation: &'static str, status: KvmStatus) -> KvaserError {
        KvaserError::Status {
            library: KVMLIB_LABEL,
            operation,
            code: status,
            message: self.error_text(status),
        }
    }

    fn error_text(&self, status: KvmStatus) -> String {
        let mut buffer = [0_u8; 256];
        let text_status =
            unsafe { (self.kvm_get_error_text)(status, buffer.as_mut_ptr(), buffer.len()) };
        if text_status == KVM_OK {
            let text = unsafe { c_buffer_to_string(&buffer) };
            if !text.is_empty() {
                return text;
            }
        }
        format!("status {status}")
    }
}

struct Kvlclib {
    _library: Library,
    kvlc_get_error_text: KvlcGetErrorText,
    kvlc_create_converter: KvlcCreateConverter,
    kvlc_delete_converter: KvlcDeleteConverter,
    kvlc_feed_select_format: KvlcFeedSelectFormat,
    kvlc_feed_log_event: KvlcFeedLogEvent,
    kvlc_convert_event: KvlcConvertEvent,
    kvlc_set_property: KvlcSetProperty,
    kvlc_event_count_ex: KvlcEventCountEx,
    kvlc_get_output_filename: KvlcGetOutputFilename,
    kvlc_add_database_file: Option<KvlcAddDatabaseFile>,
    kvlc_add_database: Option<KvlcAddDatabase>,
}

impl Kvlclib {
    fn load() -> Result<Arc<Self>> {
        let (library, library_name) = load_library(KVLCLIB_LABEL, KVLCLIB_CANDIDATES)?;

        let kvlc_get_error_text =
            unsafe { load_symbol(&library, library_name, "kvlcGetErrorText")? };
        let kvlc_create_converter =
            unsafe { load_symbol(&library, library_name, "kvlcCreateConverter")? };
        let kvlc_delete_converter =
            unsafe { load_symbol(&library, library_name, "kvlcDeleteConverter")? };
        let kvlc_feed_select_format =
            unsafe { load_symbol(&library, library_name, "kvlcFeedSelectFormat")? };
        let kvlc_feed_log_event =
            unsafe { load_symbol(&library, library_name, "kvlcFeedLogEvent")? };
        let kvlc_convert_event =
            unsafe { load_symbol(&library, library_name, "kvlcConvertEvent")? };
        let kvlc_set_property = unsafe { load_symbol(&library, library_name, "kvlcSetProperty")? };
        let kvlc_event_count_ex =
            unsafe { load_symbol(&library, library_name, "kvlcEventCountEx")? };
        let kvlc_get_output_filename =
            unsafe { load_symbol(&library, library_name, "kvlcGetOutputFilename")? };
        let kvlc_add_database_file = unsafe {
            library
                .get::<KvlcAddDatabaseFile>(b"kvlcAddDatabaseFile")
                .ok()
                .map(|symbol| *symbol)
        };
        let kvlc_add_database = unsafe {
            library
                .get::<KvlcAddDatabase>(b"kvlcAddDatabase")
                .ok()
                .map(|symbol| *symbol)
        };

        Ok(Arc::new(Self {
            _library: library,
            kvlc_get_error_text,
            kvlc_create_converter,
            kvlc_delete_converter,
            kvlc_feed_select_format,
            kvlc_feed_log_event,
            kvlc_convert_event,
            kvlc_set_property,
            kvlc_event_count_ex,
            kvlc_get_output_filename,
            kvlc_add_database_file,
            kvlc_add_database,
        }))
    }

    fn create_converter(self: &Arc<Self>, output: &Path, format: i32) -> Result<Converter> {
        let path = CString::new(output.as_os_str().as_bytes())?;
        let mut handle = std::ptr::null_mut();
        let status = unsafe { (self.kvlc_create_converter)(&mut handle, path.as_ptr(), format) };
        self.ensure_status("kvlcCreateConverter", status)?;
        Ok(Converter {
            api: Arc::clone(self),
            handle,
            db_handles: Vec::new(),
        })
    }

    fn ensure_status(&self, operation: &'static str, status: KvlcStatus) -> Result<()> {
        if status == KVLC_OK {
            return Ok(());
        }
        Err(self.status_error(operation, status))
    }

    fn status_error(&self, operation: &'static str, status: KvlcStatus) -> KvaserError {
        KvaserError::Status {
            library: KVLCLIB_LABEL,
            operation,
            code: status,
            message: self.error_text(status),
        }
    }

    fn error_text(&self, status: KvlcStatus) -> String {
        let mut buffer = [0_u8; 256];
        let text_status =
            unsafe { (self.kvlc_get_error_text)(status, buffer.as_mut_ptr(), buffer.len() as u32) };
        if text_status == KVLC_OK {
            let text = unsafe { c_buffer_to_string(&buffer) };
            if !text.is_empty() {
                return text;
            }
        }
        format!("status {status}")
    }
}

struct Converter {
    api: Arc<Kvlclib>,
    handle: KvlcHandle,
    db_handles: Vec<(Arc<Kvadblib>, KvaDbHnd)>,
}

impl Converter {
    fn set_overwrite(&self, overwrite: bool) -> Result<()> {
        self.set_property_i32(KVLC_PROPERTY_OVERWRITE, i32::from(overwrite))
    }

    fn set_signal_based(&self, enabled: bool) -> Result<()> {
        self.set_property_i32(KVLC_PROPERTY_SIGNAL_BASED, i32::from(enabled))
    }

    fn set_start_of_measurement(&self, enabled: bool) -> Result<()> {
        self.set_property_i32(KVLC_PROPERTY_START_OF_MEASUREMENT, i32::from(enabled))
    }

    fn set_creation_date(&self, unix_time: i64) -> Result<()> {
        self.set_property_i64(KVLC_PROPERTY_CREATION_DATE, unix_time)
    }

    fn set_property_i32(&self, property: u32, value: i32) -> Result<()> {
        let mut value = value;
        let status = unsafe {
            (self.api.kvlc_set_property)(
                self.handle,
                property,
                (&mut value as *mut i32).cast::<c_void>(),
                std::mem::size_of::<i32>() as u32,
            )
        };
        self.api.ensure_status("kvlcSetProperty", status)
    }

    fn set_property_i64(&self, property: u32, value: i64) -> Result<()> {
        let mut value = value;
        let status = unsafe {
            (self.api.kvlc_set_property)(
                self.handle,
                property,
                (&mut value as *mut i64).cast::<c_void>(),
                std::mem::size_of::<i64>() as u32,
            )
        };
        self.api.ensure_status("kvlcSetProperty", status)
    }

    fn feed_select_format(&self, format: i32) -> Result<()> {
        let status = unsafe { (self.api.kvlc_feed_select_format)(self.handle, format) };
        self.api.ensure_status("kvlcFeedSelectFormat", status)
    }

    fn add_database_file(&self, file: &Path, channel_mask: u32) -> Result<bool> {
        let Some(callback) = self.api.kvlc_add_database_file else {
            return Ok(false);
        };
        let file = CString::new(file.as_os_str().as_bytes())?;
        let status = unsafe { callback(self.handle, file.as_ptr(), channel_mask) };
        if status == KVLC_ERR_NOT_IMPLEMENTED {
            return Ok(false);
        }
        self.api.ensure_status("kvlcAddDatabaseFile", status)?;
        Ok(true)
    }

    fn add_database_handle(&self, db_handle: KvaDbHnd, channel_mask: u32) -> Result<()> {
        let Some(callback) = self.api.kvlc_add_database else {
            return Err(KvaserError::Config(
                "kvlclib in use does not expose kvlcAddDatabase".to_owned(),
            ));
        };
        let status = unsafe { callback(self.handle, db_handle, channel_mask) };
        self.api.ensure_status("kvlcAddDatabase", status)
    }

    fn track_database_handle(&mut self, api: Arc<Kvadblib>, db_handle: KvaDbHnd) {
        self.db_handles.push((api, db_handle));
    }

    fn feed_event_memo(&self, event: &mut MemoLogEventEx) -> Result<()> {
        let status = unsafe { (self.api.kvlc_feed_log_event)(self.handle, event.as_mut_void()) };
        self.api.ensure_status("kvlcFeedLogEvent", status)
    }

    fn convert_event(&self) -> Result<()> {
        let status = unsafe { (self.api.kvlc_convert_event)(self.handle) };
        self.api.ensure_status("kvlcConvertEvent", status)
    }

    fn event_count(&self) -> Result<i64> {
        let mut count = 0_i64;
        let status = unsafe { (self.api.kvlc_event_count_ex)(self.handle, &mut count) };
        self.api.ensure_status("kvlcEventCountEx", status)?;
        Ok(count)
    }

    fn output_filename(&self) -> Result<PathBuf> {
        let mut buffer = [0_i8; 4096];
        let status = unsafe {
            (self.api.kvlc_get_output_filename)(
                self.handle,
                buffer.as_mut_ptr(),
                buffer.len() as i32,
            )
        };
        self.api.ensure_status("kvlcGetOutputFilename", status)?;
        let text = c_char_buffer_to_string(&buffer);
        if text.is_empty() {
            return Err(KvaserError::Config(
                "kvlclib returned an empty output filename".to_owned(),
            ));
        }
        Ok(PathBuf::from(text))
    }

    fn close(mut self) -> Result<()> {
        self.close_database_handles();
        if self.handle.is_null() {
            return Ok(());
        }
        let handle = self.handle;
        self.handle = std::ptr::null_mut();
        let status = unsafe { (self.api.kvlc_delete_converter)(handle) };
        self.api.ensure_status("kvlcDeleteConverter", status)
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        self.close_database_handles();
        if !self.handle.is_null() {
            let _ = unsafe { (self.api.kvlc_delete_converter)(self.handle) };
        }
    }
}

impl Converter {
    fn close_database_handles(&mut self) {
        for (api, handle) in self.db_handles.drain(..) {
            api.close_database_handle(handle);
        }
    }
}

struct PreparedDbcFile {
    path: PathBuf,
    temporary: bool,
}

impl PreparedDbcFile {
    fn new(source: &Path) -> Result<Self> {
        let input = fs::read(source)?;
        let (contents, changed) = prepare_dbc_contents(&input);
        if !changed {
            return Ok(Self {
                path: source.to_path_buf(),
                temporary: false,
            });
        }

        for _ in 0..100 {
            let sequence = DBC_TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "kvaser-utils-{}-{sequence}.dbc",
                std::process::id()
            ));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file.write_all(&contents) {
                        drop(file);
                        let _ = fs::remove_file(&path);
                        return Err(error.into());
                    }
                    return Ok(Self {
                        path,
                        temporary: true,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            }
        }

        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "could not allocate a temporary DBC file",
        )
        .into())
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for PreparedDbcFile {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn prepare_dbc_contents(input: &[u8]) -> (Vec<u8>, bool) {
    let lines: Vec<&[u8]> = input.split_inclusive(|byte| *byte == b'\n').collect();
    let mut value_tables = HashMap::<Vec<u8>, Vec<u8>>::new();
    let mut message_ids = HashSet::<Vec<u8>>::new();

    for line in &lines {
        let (body, _) = split_dbc_line(line);
        let tokens: Vec<&[u8]> = body
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|token| !token.is_empty())
            .collect();
        if tokens.first() == Some(&b"BO_".as_slice()) {
            if let Some(id) = tokens.get(1) {
                message_ids.insert((*id).to_vec());
            }
        }
        if let Some((name, values)) = parse_dbc_value_table(body) {
            value_tables.insert(name.to_vec(), values.to_vec());
        }
    }

    let mut output = Vec::with_capacity(input.len());
    let mut changed = false;
    for line in lines {
        let (body, ending) = split_dbc_line(line);
        let tokens: Vec<&[u8]> = body
            .split(|byte| byte.is_ascii_whitespace())
            .filter(|token| !token.is_empty())
            .collect();

        if let Some(id) = dbc_record_message_id(&tokens) {
            if !message_ids.contains(id) {
                changed = true;
                continue;
            }
        }

        if tokens.len() == 4 && tokens[0] == b"VAL_" {
            let table_token = tokens[3];
            if table_token.last() == Some(&b';') {
                let table_name = &table_token[..table_token.len() - 1];
                if let Some(values) = value_tables.get(table_name) {
                    output.extend_from_slice(b"VAL_ ");
                    output.extend_from_slice(tokens[1]);
                    output.push(b' ');
                    output.extend_from_slice(tokens[2]);
                    output.push(b' ');
                    output.extend_from_slice(values);
                    output.push(b';');
                    output.extend_from_slice(ending);
                    changed = true;
                    continue;
                }
            }
        }

        output.extend_from_slice(line);
    }

    (output, changed)
}

fn split_dbc_line(line: &[u8]) -> (&[u8], &[u8]) {
    if line.ends_with(b"\r\n") {
        (&line[..line.len() - 2], &line[line.len() - 2..])
    } else if line.ends_with(b"\n") {
        (&line[..line.len() - 1], &line[line.len() - 1..])
    } else {
        (line, &[])
    }
}

fn parse_dbc_value_table(line: &[u8]) -> Option<(&[u8], &[u8])> {
    let mut rest = trim_ascii_start(line);
    rest = rest.strip_prefix(b"VAL_TABLE_")?;
    rest = trim_ascii_start(rest);
    let name_end = rest.iter().position(|byte| byte.is_ascii_whitespace())?;
    let name = &rest[..name_end];
    let values = trim_ascii(&rest[name_end..]);
    if values.last() != Some(&b';') {
        return None;
    }
    Some((name, trim_ascii(&values[..values.len() - 1])))
}

fn trim_ascii_start(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    value
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    value = trim_ascii_start(value);
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn dbc_record_message_id<'a>(tokens: &[&'a [u8]]) -> Option<&'a [u8]> {
    match tokens.first().copied()? {
        b"BO_TX_BU_" | b"VAL_" | b"SIG_VALTYPE_" | b"SG_MUL_VAL_" | b"SIG_GROUP_" => {
            tokens.get(1).copied()
        }
        b"CM_" | b"BA_" => tokens
            .iter()
            .position(|token| *token == b"BO_" || *token == b"SG_")
            .and_then(|position| tokens.get(position + 1).copied()),
        _ => None,
    }
}

struct Kvadblib {
    _library: Library,
    kva_db_open: KvaDbOpen,
    kva_db_create: Option<KvaDbCreate>,
    kva_db_read_file: KvaDbReadFile,
    kva_db_close: KvaDbClose,
    kva_db_get_error_text: KvaDbGetErrorText,
    kva_db_get_last_parse_error: Option<KvaDbGetLastParseError>,
}

impl Kvadblib {
    fn try_load() -> Option<Arc<Self>> {
        Self::load().ok()
    }

    fn load() -> Result<Arc<Self>> {
        let (library, library_name) = load_library(KVADBLIB_LABEL, KVADBLIB_CANDIDATES)?;
        let kva_db_open = unsafe { load_symbol(&library, library_name, "kvaDbOpen")? };
        let kva_db_create = unsafe {
            library
                .get::<KvaDbCreate>(b"kvaDbCreate")
                .ok()
                .map(|symbol| *symbol)
        };
        let kva_db_read_file = unsafe { load_symbol(&library, library_name, "kvaDbReadFile")? };
        let kva_db_close = unsafe { load_symbol(&library, library_name, "kvaDbClose")? };
        let kva_db_get_error_text =
            unsafe { load_symbol(&library, library_name, "kvaDbGetErrorText")? };
        let kva_db_get_last_parse_error = unsafe {
            library
                .get::<KvaDbGetLastParseError>(b"kvaDbGetLastParseError")
                .ok()
                .map(|symbol| *symbol)
        };

        Ok(Arc::new(Self {
            _library: library,
            kva_db_open,
            kva_db_create,
            kva_db_read_file,
            kva_db_close,
            kva_db_get_error_text,
            kva_db_get_last_parse_error,
        }))
    }

    fn open_database_file(&self, path: &Path, source_path: &Path) -> Result<KvaDbHnd> {
        let mut handle = std::ptr::null_mut();
        let status = unsafe { (self.kva_db_open)(&mut handle) };
        self.ensure_status("kvaDbOpen", status)?;

        let file = CString::new(path.as_os_str().as_bytes())?;
        let status = if let Some(kva_db_create) = self.kva_db_create {
            let local_name = source_path
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("dbc");
            let local_name = CString::new(local_name)?;
            unsafe { kva_db_create(handle, local_name.as_ptr(), file.as_ptr()) }
        } else {
            unsafe { (self.kva_db_read_file)(handle, file.as_ptr()) }
        };
        if status != KVADB_OK {
            let op = if self.kva_db_create.is_some() {
                "kvaDbCreate"
            } else {
                "kvaDbReadFile"
            };
            let error = self.database_file_error(op, status, source_path);
            self.close_database_handle(handle);
            return Err(error);
        }
        Ok(handle)
    }

    fn database_file_error(
        &self,
        operation: &'static str,
        status: KvaDbStatus,
        path: &Path,
    ) -> KvaserError {
        let mut message = format!("{}: {}", path.display(), self.error_text(status));
        if status == KVADB_ERR_DB_FILE_PARSE {
            if let Some(detail) = self.last_parse_error() {
                message.push_str(": ");
                message.push_str(&detail);
            }
        }
        KvaserError::Status {
            library: KVADBLIB_LABEL,
            operation,
            code: status,
            message,
        }
    }

    fn last_parse_error(&self) -> Option<String> {
        let callback = self.kva_db_get_last_parse_error?;
        let mut buffer = [0_i8; 1024];
        let mut length = buffer.len() as c_uint;
        let status = unsafe { callback(buffer.as_mut_ptr(), &mut length) };
        if status != KVADB_OK {
            return None;
        }
        let bytes: Vec<u8> = buffer.iter().map(|value| *value as u8).collect();
        let text = unsafe { c_buffer_to_string(&bytes) };
        let detail = text
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("Error"))
            .or_else(|| text.lines().map(str::trim).find(|line| !line.is_empty()))?;
        Some(detail.to_owned())
    }

    fn close_database_handle(&self, handle: KvaDbHnd) {
        if !handle.is_null() {
            let _ = unsafe { (self.kva_db_close)(handle) };
        }
    }

    fn ensure_status(&self, operation: &'static str, status: KvaDbStatus) -> Result<()> {
        if status == KVADB_OK {
            return Ok(());
        }
        Err(self.status_error(operation, status))
    }

    fn status_error(&self, operation: &'static str, status: KvaDbStatus) -> KvaserError {
        KvaserError::Status {
            library: KVADBLIB_LABEL,
            operation,
            code: status,
            message: self.error_text(status),
        }
    }

    fn error_text(&self, status: KvaDbStatus) -> String {
        let mut buffer = [0_i8; 256];
        let text_status =
            unsafe { (self.kva_db_get_error_text)(status, buffer.as_mut_ptr(), buffer.len()) };
        if text_status == KVADB_OK {
            let bytes: Vec<u8> = buffer.iter().map(|value| *value as u8).collect();
            let text = unsafe { c_buffer_to_string(&bytes) };
            if !text.is_empty() {
                return text;
            }
        }
        format!("status {status}")
    }
}

struct KvaMemoLibXml {
    _library: Library,
    kva_xml_get_last_error: KvaXmlGetLastError,
    kva_xml_to_buffer: KvaXmlToBuffer,
    kva_xml_validate: KvaXmlValidate,
    kva_xml_get_validation_status_count: KvaXmlGetValidationStatusCount,
    kva_xml_get_validation_error: KvaXmlGetValidationError,
    kva_xml_get_validation_warning: KvaXmlGetValidationWarning,
}

impl KvaMemoLibXml {
    fn load() -> Result<Self> {
        let (library, library_name) = load_library(KVAMEMOLIBXML_LABEL, KVAMEMOLIBXML_CANDIDATES)?;

        let kva_xml_initialize: KvaXmlInitialize =
            unsafe { load_symbol(&library, library_name, "kvaXmlInitialize")? };
        let kva_xml_get_last_error =
            unsafe { load_symbol(&library, library_name, "kvaXmlGetLastError")? };
        let kva_xml_to_buffer = unsafe { load_symbol(&library, library_name, "kvaXmlToBuffer")? };
        let kva_xml_validate = unsafe { load_symbol(&library, library_name, "kvaXmlValidate")? };
        let kva_xml_get_validation_status_count =
            unsafe { load_symbol(&library, library_name, "kvaXmlGetValidationStatusCount")? };
        let kva_xml_get_validation_error =
            unsafe { load_symbol(&library, library_name, "kvaXmlGetValidationError")? };
        let kva_xml_get_validation_warning =
            unsafe { load_symbol(&library, library_name, "kvaXmlGetValidationWarning")? };

        let status = unsafe { kva_xml_initialize() };
        if status != KVAXML_OK {
            return Err(KvaserError::Status {
                library: KVAMEMOLIBXML_LABEL,
                operation: "kvaXmlInitialize",
                code: status,
                message: format!("status {status}"),
            });
        }

        Ok(Self {
            _library: library,
            kva_xml_get_last_error,
            kva_xml_to_buffer,
            kva_xml_validate,
            kva_xml_get_validation_status_count,
            kva_xml_get_validation_error,
            kva_xml_get_validation_warning,
        })
    }

    fn xml_to_config_buffer(&self, xml: &str) -> Result<ConvertedLoggerConfig> {
        let xml_cstr = CString::new(xml)?;
        self.validate_xml(xml_cstr.as_ptr(), xml.len() as c_uint)?;

        let mut buffer = vec![0_u8; PARAM_FILE_SIZE];
        let mut out_len = buffer.len() as c_uint;
        let mut version = 0 as c_long;
        let status = unsafe {
            (self.kva_xml_to_buffer)(
                xml_cstr.as_ptr(),
                xml.len() as c_uint,
                buffer.as_mut_ptr().cast::<c_char>(),
                &mut out_len,
                &mut version,
            )
        };
        if status != KVAXML_OK {
            return Err(self.status_error("kvaXmlToBuffer", status));
        }
        buffer.truncate(out_len as usize);

        let warnings = self.collect_warnings()?;
        Ok(ConvertedLoggerConfig { buffer, warnings })
    }

    fn validate_xml(&self, xml: *const c_char, len: c_uint) -> Result<()> {
        let status = unsafe { (self.kva_xml_validate)(xml, len) };
        if status != KVAXML_OK {
            return Err(self.status_error("kvaXmlValidate", status));
        }

        let mut errors = 0;
        let mut warnings = 0;
        let status =
            unsafe { (self.kva_xml_get_validation_status_count)(&mut errors, &mut warnings) };
        if status != KVAXML_OK {
            return Err(self.status_error("kvaXmlGetValidationStatusCount", status));
        }

        if errors > 0 {
            let messages = self.collect_errors()?;
            return Err(KvaserError::Config(format!(
                "invalid logger XML: {}",
                messages.join("; ")
            )));
        }

        Ok(())
    }

    fn collect_errors(&self) -> Result<Vec<String>> {
        self.collect_validation_messages(
            "kvaXmlGetValidationError",
            self.kva_xml_get_validation_error,
        )
    }

    fn collect_warnings(&self) -> Result<Vec<String>> {
        self.collect_validation_messages(
            "kvaXmlGetValidationWarning",
            self.kva_xml_get_validation_warning,
        )
    }

    fn collect_validation_messages(
        &self,
        operation: &'static str,
        callback: KvaXmlGetValidationError,
    ) -> Result<Vec<String>> {
        let mut messages = Vec::new();
        loop {
            let mut validation_status = 0;
            let mut buffer = [0 as c_char; 512];
            let status = unsafe {
                callback(
                    &mut validation_status,
                    buffer.as_mut_ptr(),
                    buffer.len() as c_uint,
                )
            };
            if status != KVAXML_OK {
                return Err(self.status_error(operation, status));
            }
            if validation_status == KVAXML_OK {
                break;
            }
            let text = c_char_buffer_to_string(&buffer);
            if text.is_empty() {
                messages.push(format!("validation status {validation_status}"));
            } else {
                messages.push(text);
            }
        }
        Ok(messages)
    }

    fn status_error(&self, operation: &'static str, status: KvaXmlStatus) -> KvaserError {
        KvaserError::Status {
            library: KVAMEMOLIBXML_LABEL,
            operation,
            code: status,
            message: self.error_text(status),
        }
    }

    fn error_text(&self, fallback_status: KvaXmlStatus) -> String {
        let mut buffer = [0 as c_char; 512];
        let mut status = fallback_status;
        let text_status = unsafe {
            (self.kva_xml_get_last_error)(buffer.as_mut_ptr(), buffer.len() as c_uint, &mut status)
        };
        if text_status == KVAXML_OK {
            let text = c_char_buffer_to_string(&buffer);
            if !text.is_empty() {
                return text;
            }
        }
        format!("status {fallback_status}")
    }
}

struct ConvertedLoggerConfig {
    buffer: Vec<u8>,
    warnings: Vec<String>,
}

fn build_log_all_xml(channels: &[LoggerCanChannelConfig]) -> Result<String> {
    let mut can_bus = String::new();
    for channel in channels {
        if channel.channel > 4 {
            return Err(KvaserError::Config(format!(
                "logger CAN channel {} is out of range 0..4",
                channel.channel
            )));
        }

        let (tseg1, tseg2, sjw) = logger_timing(channel.bitrate)?;
        can_bus.push_str(&format!(
            r#"    <PARAMETERS channel="{channel}" bitrate="{bitrate}" tseg1="{tseg1}" tseg2="{tseg2}" sjw="{sjw}" silent="YES"/>"#,
            channel = channel.channel,
            bitrate = channel.bitrate.as_bps(),
        ));
        can_bus.push('\n');
    }

    Ok(format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<KVASER>
  <VERSION>2.0</VERSION>
  <BINARY_VERSION>6.0</BINARY_VERSION>
  <SETTINGS>
    <MODE log_all="YES" fifo_mode="YES"/>
    <CANPOWER timeout="0"/>
  </SETTINGS>
  <CAN_BUS>
{can_bus}  </CAN_BUS>
  <TRIGGERBLOCK>
    <TRIGGERS/>
    <STATEMENTS/>
  </TRIGGERBLOCK>
  <TRANSMIT_LISTS/>
  <MESSAGES/>
  <FILTERS/>
  <SCRIPTS/>
</KVASER>
"#
    ))
}

fn logger_timing(bitrate: CanBitrate) -> Result<(u16, u16, u8)> {
    match bitrate {
        CanBitrate::B500K => Ok((11, 4, 1)),
        CanBitrate::B1M => Ok((5, 2, 1)),
        other => Err(KvaserError::InvalidBitrate(format!(
            "{other}; logger config supports only 500k and 1M"
        ))),
    }
}

fn c_char_buffer_to_string(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().map(|value| *value as u8).collect();
    unsafe { c_buffer_to_string(&bytes) }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KvmLogEventEx {
    type_: u32,
    raw: [u8; 128],
}

const _: [(); std::mem::size_of::<KvmLogEventEx>()] =
    [(); std::mem::size_of::<bindings::kvmLogEventEx>()];

impl Default for KvmLogEventEx {
    fn default() -> Self {
        Self {
            type_: KVM_LOG_TYPE_INVALID,
            raw: [0; 128],
        }
    }
}

impl KvmLogEventEx {
    fn event_type(&self) -> u32 {
        self.type_
    }

    fn raw_copy(&self) -> [u8; 128] {
        self.raw
    }

    fn memo_event(&self) -> MemoLogEventEx {
        MemoLogEventEx {
            type_: self.type_,
            raw: self.raw,
        }
    }

    fn is_supported_for_export(&self) -> bool {
        matches!(
            self.type_,
            KVM_LOG_TYPE_MSG | KVM_LOG_TYPE_LINMSG | KVM_LOG_TYPE_CLOCK
        )
    }

    fn timestamp_ns_for_export(&self) -> Option<i64> {
        let raw = self.raw_copy();
        match self.event_type() {
            KVM_LOG_TYPE_MSG | KVM_LOG_TYPE_LINMSG => Some(read_i64(&raw, 4)),
            KVM_LOG_TYPE_CLOCK => Some(read_i64(&raw, 4)),
            KVM_LOG_TYPE_TRIGGER => Some(read_i64(&raw, 16)),
            _ => None,
        }
    }

    fn preview(&self) -> LogEventPreview {
        let raw = self.raw_copy();
        match self.event_type() {
            KVM_LOG_TYPE_MSG => preview_message(LogEventKind::CanMessage, &raw),
            KVM_LOG_TYPE_LINMSG => preview_message(LogEventKind::LinMessage, &raw),
            KVM_LOG_TYPE_CLOCK => LogEventPreview {
                kind: LogEventKind::RtcClock,
                timestamp_ns: Some(read_i64(&raw, 4)),
                channel: None,
                id: None,
                dlc: None,
                flags: None,
                data: Vec::new(),
            },
            KVM_LOG_TYPE_TRIGGER => LogEventPreview {
                kind: LogEventKind::Trigger,
                timestamp_ns: Some(read_i64(&raw, 16)),
                channel: None,
                id: None,
                dlc: None,
                flags: None,
                data: Vec::new(),
            },
            KVM_LOG_TYPE_VERSION => LogEventPreview {
                kind: LogEventKind::Version,
                timestamp_ns: None,
                channel: None,
                id: None,
                dlc: None,
                flags: None,
                data: Vec::new(),
            },
            KVM_LOG_TYPE_INVALID => LogEventPreview {
                kind: LogEventKind::Invalid,
                timestamp_ns: None,
                channel: None,
                id: None,
                dlc: None,
                flags: None,
                data: Vec::new(),
            },
            other => LogEventPreview {
                kind: LogEventKind::Unknown(other),
                timestamp_ns: None,
                channel: None,
                id: None,
                dlc: None,
                flags: None,
                data: Vec::new(),
            },
        }
    }
}

fn preview_message(kind: LogEventKind, raw: &[u8; 128]) -> LogEventPreview {
    let dlc = read_u32(raw, 16);
    let len = usize::try_from(dlc).unwrap_or(0).min(64);
    LogEventPreview {
        kind,
        timestamp_ns: Some(read_i64(raw, 4)),
        channel: Some(read_u32(raw, 12)),
        id: Some(read_u32(raw, 0)),
        dlc: Some(dlc),
        flags: Some(read_u32(raw, 20)),
        data: raw[24..24 + len].to_vec(),
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct MemoLogEventEx {
    type_: u32,
    raw: [u8; 128],
}

impl MemoLogEventEx {
    fn as_mut_void(&mut self) -> *mut c_void {
        (self as *mut MemoLogEventEx).cast::<c_void>()
    }

    fn normalize_for_export(&mut self) {
        if matches!(self.type_, KVM_LOG_TYPE_MSG | KVM_LOG_TYPE_LINMSG) {
            let dlc = read_u32(&self.raw, 16).min(64);
            write_u32(&mut self.raw, 16, dlc);
        }
    }
}

fn read_u32(raw: &[u8; 128], offset: usize) -> u32 {
    let mut bytes = [0_u8; 4];
    bytes.copy_from_slice(&raw[offset..offset + 4]);
    u32::from_ne_bytes(bytes)
}

fn read_i64(raw: &[u8; 128], offset: usize) -> i64 {
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&raw[offset..offset + 8]);
    i64::from_ne_bytes(bytes)
}

fn write_u32(raw: &mut [u8; 128], offset: usize, value: u32) {
    raw[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
}

unsafe extern "C" {
    fn dup(fd: i32) -> i32;
    fn dup2(oldfd: i32, newfd: i32) -> i32;
    fn close(fd: i32) -> i32;
}

fn with_silenced_stdio<T>(action: impl FnOnce() -> Result<T>) -> Result<T> {
    const STDOUT_FD: i32 = 1;
    const STDERR_FD: i32 = 2;

    let saved_out = unsafe { dup(STDOUT_FD) };
    let saved_err = unsafe { dup(STDERR_FD) };
    if saved_out < 0 || saved_err < 0 {
        if saved_out >= 0 {
            let _ = unsafe { close(saved_out) };
        }
        if saved_err >= 0 {
            let _ = unsafe { close(saved_err) };
        }
        return action();
    }

    let devnull = match OpenOptions::new().write(true).open("/dev/null") {
        Ok(file) => file,
        Err(_) => {
            let _ = unsafe { close(saved_out) };
            let _ = unsafe { close(saved_err) };
            return action();
        }
    };

    if unsafe { dup2(devnull.as_raw_fd(), STDOUT_FD) } < 0
        || unsafe { dup2(devnull.as_raw_fd(), STDERR_FD) } < 0
    {
        let _ = unsafe { close(saved_out) };
        let _ = unsafe { close(saved_err) };
        return action();
    }

    let result = action();
    let _ = unsafe { dup2(saved_out, STDOUT_FD) };
    let _ = unsafe { dup2(saved_err, STDERR_FD) };
    let _ = unsafe { close(saved_out) };
    let _ = unsafe { close(saved_err) };
    result
}

fn is_kvlc_parameter_error(error: &KvaserError, operation: &'static str) -> bool {
    matches!(
        error,
        KvaserError::Status {
            library,
            operation: op,
            code,
            ..
        } if *library == KVLCLIB_LABEL && *op == operation && *code == KVLC_ERR_PARAM
    )
}

fn is_kvlc_decreasing_time_error(error: &KvaserError, operation: &'static str) -> bool {
    matches!(
        error,
        KvaserError::Status {
            library,
            operation: op,
            code,
            ..
        } if *library == KVLCLIB_LABEL && *op == operation && *code == KVLC_ERR_DECREASING_TIME
    )
}

fn is_kvlc_not_implemented_error(error: &KvaserError, operation: &'static str) -> bool {
    matches!(
        error,
        KvaserError::Status {
            library,
            operation: op,
            code,
            ..
        } if *library == KVLCLIB_LABEL && *op == operation && *code == KVLC_ERR_NOT_IMPLEMENTED
    )
}

fn output_path_for_segment(
    output: &Path,
    reference_unix_time: Option<u32>,
    part: u32,
    existing_outputs: &[PathBuf],
) -> PathBuf {
    if part == 1 {
        return output.to_path_buf();
    }
    let parent = output.parent().unwrap_or_else(|| Path::new(""));
    let stem = reference_unix_time
        .map(export_base_name)
        .or_else(|| {
            output
                .file_stem()
                .and_then(|value| value.to_str())
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| format!("memorator_segment{part}"));
    let ext = output.extension().and_then(|value| value.to_str());
    let build_path = |suffix: Option<u32>| {
        let file_name = match (ext, suffix) {
            (Some(ext), Some(suffix)) if !ext.is_empty() => format!("{stem}_{suffix}.{ext}"),
            (Some(ext), None) if !ext.is_empty() => format!("{stem}.{ext}"),
            (_, Some(suffix)) => format!("{stem}_{suffix}"),
            _ => stem.clone(),
        };
        parent.join(file_name)
    };

    let mut candidate = build_path(None);
    let mut suffix = 2_u32;
    while candidate.exists() || existing_outputs.iter().any(|path| path == &candidate) {
        candidate = build_path(Some(suffix));
        suffix += 1;
    }
    candidate
}

fn export_base_name(unix_ts: u32) -> String {
    let mut tm = libc::tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    let seconds = unix_ts as libc::time_t;
    let result = unsafe { libc::localtime_r(&seconds, &mut tm) };
    if result.is_null() {
        return format!("log_{unix_ts}");
    }

    let year = (tm.tm_year + 1900) % 100;
    format!(
        "log_{year:02}_{month:02}_{day:02}-{hour:02}_{min:02}_{sec:02}",
        month = tm.tm_mon + 1,
        day = tm.tm_mday,
        hour = tm.tm_hour,
        min = tm.tm_min,
        sec = tm.tm_sec
    )
}

fn finalize_converter(converter: Converter, fallback_path: &Path) -> Result<PathBuf> {
    let output_path = converter
        .output_filename()
        .unwrap_or_else(|_| fallback_path.to_path_buf());
    let _ = converter.event_count();
    converter.close()?;
    if !output_path.is_file() {
        return Err(KvaserError::Config(format!(
            "kvlclib conversion completed but output file was not created at {}",
            output_path.display()
        )));
    }
    Ok(output_path)
}

#[cfg(test)]
mod tests {
    use super::prepare_dbc_contents;

    #[test]
    fn prepares_named_value_tables_and_removes_dangling_records() {
        let input = b"VAL_TABLE_ STATE 0 \"off\" 1 \"on\";\r\n\
BO_ 10 Status: 1 ECU\r\n\
 SG_ state : 0|1@1+ (1,0) [0|1] \"\" ECU\r\n\
VAL_ 10 state STATE;\r\n\
BA_ \"GenMsgSendType\" BO_ 99 1;\r\n";

        let (output, changed) = prepare_dbc_contents(input);

        assert!(changed);
        assert!(
            output
                .windows(b"VAL_ 10 state 0 \"off\" 1 \"on\";".len())
                .any(|window| window == b"VAL_ 10 state 0 \"off\" 1 \"on\";")
        );
        assert!(
            !output
                .windows(b"BO_ 99".len())
                .any(|window| window == b"BO_ 99")
        );
        assert!(output.ends_with(b"\r\n"));
    }

    #[test]
    fn leaves_an_already_compatible_dbc_unchanged() {
        let input = b"BO_ 10 Status: 1 ECU\nVAL_ 10 state 0 \"off\" 1 \"on\";\n";

        let (output, changed) = prepare_dbc_contents(input);

        assert!(!changed);
        assert_eq!(output, input);
    }
}
