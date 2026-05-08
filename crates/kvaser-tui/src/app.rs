use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use kvaser_core::{
    AppController, CanBitrate, CanChannel, CanDeviceMode, CanFrame, ExportProgress, ExportSummary,
    LogEventPreview, LogFileInfo, LoggerCanChannelConfig, Result as CoreResult,
    SocketCanTunnelConfig,
};

use crate::config::AppConfig;

const LIVE_FRAME_LIMIT: usize = 200;
const LIVE_POLL_LIMIT_PER_CHANNEL: usize = 64;
const DEVICE_PROBE_INTERVAL: Duration = Duration::from_millis(500);
const LOG_PAGE_SIZE: u32 = 20;
const EXPORTS_DIR: &str = "exports";
const EXPORTED_MANIFEST: &str = ".exported_logs_manifest.tsv";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Can,
    Logs,
    Live,
}

impl Tab {
    pub fn index(self) -> usize {
        match self {
            Tab::Can => 0,
            Tab::Logs => 1,
            Tab::Live => 2,
        }
    }
}

pub struct App {
    pub controller: AppController,
    pub device_channel: u32,
    pub tab: Tab,
    pub channels: Vec<CanChannel>,
    pub logs: Vec<LogFileInfo>,
    pub preview: Vec<LogEventPreview>,
    pub live_frames: VecDeque<CanFrame>,
    pub device_mode: Option<CanDeviceMode>,
    pub selected_channel: usize,
    pub selected_log: usize,
    pub selected_logs: BTreeSet<u32>,
    pub bitrate_ch0: CanBitrate,
    pub bitrate_ch1: CanBitrate,
    pub status: String,
    pub should_quit: bool,
    pub logs_total: usize,
    logs_loaded: bool,
    range_anchor: Option<usize>,
    export_rx: Option<Receiver<ExportMessage>>,
    export_pending_keys: Vec<String>,
    last_device_probe: Instant,
}

impl App {
    pub fn new(device_channel: u32) -> anyhow::Result<Self> {
        let controller = AppController::load().context("loading Kvaser runtime")?;
        let mut app = Self {
            controller,
            device_channel,
            tab: Tab::Can,
            channels: Vec::new(),
            logs: Vec::new(),
            preview: Vec::new(),
            live_frames: VecDeque::new(),
            device_mode: None,
            selected_channel: 0,
            selected_log: 0,
            selected_logs: BTreeSet::new(),
            bitrate_ch0: CanBitrate::B500K,
            bitrate_ch1: CanBitrate::B500K,
            status: String::new(),
            should_quit: false,
            logs_loaded: false,
            range_anchor: None,
            export_rx: None,
            export_pending_keys: Vec::new(),
            last_device_probe: Instant::now(),
            logs_total: 0,
        };
        app.refresh_channels();
        app.refresh_device_mode();
        let tunnel_config = SocketCanTunnelConfig::pair("can0", "can1");
        let _ = app
            .controller
            .can_mut()
            .configure_live_tunnel(&tunnel_config);
        Ok(app)
    }

    pub fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
        if self.tab == Tab::Logs && !self.logs_loaded {
            self.refresh_logs();
        }
    }

    pub fn refresh(&mut self) {
        match self.tab {
            Tab::Can => {
                self.refresh_channels();
                self.refresh_device_mode();
            }
            Tab::Logs => self.refresh_logs(),
            Tab::Live => {}
        }
    }

    pub fn tick(&mut self) {
        self.poll_export_updates();
        self.probe_devices_if_needed();

        if self.controller.can().active_channels().is_empty() {
            return;
        }

        match self
            .controller
            .can()
            .poll_live_frames_and_tunnel(LIVE_POLL_LIMIT_PER_CHANNEL)
        {
            Ok(frames) => self.push_live_frames(frames),
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn refresh_channels(&mut self) {
        match self.controller.can().list_channels() {
            Ok(channels) => {
                self.channels = channels;
                self.selected_channel = self
                    .selected_channel
                    .min(self.channels.len().saturating_sub(1));
                self.status = format!("{} CAN channel(s)", self.channels.len());
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn refresh_logs(&mut self) {
        match self
            .controller
            .logs()
            .list_logs_recent_page(self.device_channel, 0, LOG_PAGE_SIZE)
        {
            Ok((logs, total)) => {
                self.logs = logs;
                self.logs_total = total as usize;
                self.selected_log = self.selected_log.min(self.logs.len().saturating_sub(1));
                self.selected_logs
                    .retain(|index| self.logs.iter().any(|file| file.index == *index));
                self.status = format!("loaded {}/{} log file(s)", self.logs.len(), self.logs_total);
                self.logs_loaded = true;
            }
            Err(error) => {
                self.logs_loaded = false;
                self.logs_total = 0;
                self.status = error.to_string();
            }
        }
    }

    pub fn refresh_device_mode(&mut self) {
        match self.controller.can().get_device_mode(self.device_channel) {
            Ok(mode) => self.device_mode = Some(mode),
            Err(error) => {
                self.device_mode = None;
                self.status = error.to_string();
            }
        }
    }

    pub fn move_down(&mut self) {
        match self.tab {
            Tab::Can if !self.channels.is_empty() => {
                self.selected_channel = (self.selected_channel + 1).min(self.channels.len() - 1);
            }
            Tab::Logs if !self.logs.is_empty() => {
                if self.selected_log + 1 < self.logs.len() {
                    self.selected_log += 1;
                } else {
                    let previous_len = self.logs.len();
                    self.load_more_logs();
                    if self.logs.len() > previous_len {
                        self.selected_log += 1;
                    }
                }
            }
            Tab::Live => {}
            _ => {}
        }
    }

    pub fn move_up(&mut self) {
        match self.tab {
            Tab::Can => self.selected_channel = self.selected_channel.saturating_sub(1),
            Tab::Logs => {
                self.selected_log = self.selected_log.saturating_sub(1);
            }
            Tab::Live => {}
        }
    }

    pub fn cycle_bitrate(&mut self) {
        let Some(channel) = self.channels.get(self.selected_channel) else {
            return;
        };

        let current = self.bitrate_for_channel(channel.index);
        let next = next_bitrate(current);
        match channel.index {
            0 => self.bitrate_ch0 = next,
            1 => self.bitrate_ch1 = next,
            _ => {
                self.status = "bitrate rapido solo per CAN 0 e CAN 1".to_owned();
                return;
            }
        }
        self.status = format!("CAN {} bitrate {}", channel.index, next);
    }

    pub fn activate_pair(&mut self) {
        let result =
            self.controller
                .can_mut()
                .activate_pair(0, self.bitrate_ch0, 1, self.bitrate_ch1);
        self.report_core(result);
        if self.status.is_empty() {
            self.status = format!("CAN 0={} CAN 1={}", self.bitrate_ch0, self.bitrate_ch1);
        }
    }

    pub fn activate_selected(&mut self) {
        let Some(channel) = self.channels.get(self.selected_channel) else {
            return;
        };
        let channel_index = channel.index;
        let bitrate = self.bitrate_for_channel(channel_index);
        let result = self.controller.can_mut().activate(channel_index, bitrate);
        self.report_core(result);
        if self.status.is_empty() {
            self.status = format!("CAN {} on at {}", channel_index, bitrate);
        }
    }

    pub fn deactivate_selected(&mut self) {
        let Some(channel) = self.channels.get(self.selected_channel) else {
            return;
        };
        self.controller.can_mut().deactivate(channel.index);
        self.status = format!("CAN {} off", channel.index);
    }

    pub fn enable_usb_logging(&mut self) {
        let result = self
            .controller
            .can_mut()
            .set_device_mode(self.device_channel, CanDeviceMode::Logger);
        self.report_core(result);
        if self.status.is_empty() {
            self.device_mode = Some(CanDeviceMode::Logger);
            self.status = "USB logger mode active; live interface handles are closed".to_owned();
        }
    }

    pub fn configure_and_enable_usb_logging(&mut self) {
        self.controller.can_mut().deactivate_all();
        let configs = vec![
            LoggerCanChannelConfig::new(0, self.bitrate_ch0),
            LoggerCanChannelConfig::new(1, self.bitrate_ch1),
        ];
        match self
            .controller
            .logs()
            .write_log_all_config(self.device_channel, &configs)
        {
            Ok(summary) => {
                let result = self
                    .controller
                    .can_mut()
                    .set_device_mode(self.device_channel, CanDeviceMode::Logger);
                self.report_core(result);
                if self.status.is_empty() {
                    self.device_mode = Some(CanDeviceMode::Logger);
                    self.status = format!(
                        "logger config written: CAN 0 {} CAN 1 {} ({} bytes)",
                        self.bitrate_ch0, self.bitrate_ch1, summary.binary_size
                    );
                }
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn enable_interface_mode(&mut self) {
        let result = self
            .controller
            .can_mut()
            .set_device_mode(self.device_channel, CanDeviceMode::Interface);
        self.report_core(result);
        if self.status.is_empty() {
            self.device_mode = Some(CanDeviceMode::Interface);
            self.status = "interface mode active; turn CAN channels on for live view".to_owned();
        }
    }

    pub fn clear_live_frames(&mut self) {
        self.live_frames.clear();
        self.status = "live frames cleared".to_owned();
    }

    pub fn bitrate_for_channel(&self, channel: u32) -> CanBitrate {
        match channel {
            0 => self.bitrate_ch0,
            1 => self.bitrate_ch1,
            _ => self.bitrate_ch0,
        }
    }

    pub fn toggle_selected_log(&mut self) {
        let Some(file) = self.logs.get(self.selected_log) else {
            return;
        };
        if self.range_anchor.is_some() {
            self.select_range_to_current();
            return;
        }
        if !self.selected_logs.remove(&file.index) {
            self.selected_logs.insert(file.index);
        }
        self.status = format!("{} selected log(s)", self.selected_logs.len());
    }

    pub fn start_range_selection(&mut self) {
        if self.logs.get(self.selected_log).is_none() {
            return;
        }
        self.range_anchor = Some(self.selected_log);
        if let Some(file) = self.logs.get(self.selected_log) {
            self.status = format!("range start set on log {}", file.index);
        }
    }

    pub fn select_range_to_current(&mut self) {
        let Some(_) = self.logs.get(self.selected_log) else {
            return;
        };
        let anchor = self.range_anchor.unwrap_or(self.selected_log);
        let start = anchor.min(self.selected_log);
        let end = anchor.max(self.selected_log);
        for row in start..=end {
            if let Some(file) = self.logs.get(row) {
                self.selected_logs.insert(file.index);
            }
        }
        self.range_anchor = None;
        self.status = format!("{} selected log(s)", self.selected_logs.len());
    }

    pub fn select_all_logs(&mut self) {
        self.selected_logs = self.logs.iter().map(|file| file.index).collect();
        self.status = format!("{} selected log(s)", self.selected_logs.len());
    }

    pub fn select_logs_same_day(&mut self) {
        let Some(selected) = self.logs.get(self.selected_log) else {
            return;
        };
        let Some(anchor_ts) = selected.start_time_utc.or(selected.end_time_utc) else {
            self.status = "log selezionato senza timestamp".to_owned();
            return;
        };
        let Some(anchor_day) = local_day_key(anchor_ts) else {
            self.status = "impossibile determinare il giorno locale del log".to_owned();
            return;
        };

        self.selected_logs.clear();
        for file in &self.logs {
            if let Some(ts) = file.start_time_utc.or(file.end_time_utc) {
                if local_day_key(ts) == Some(anchor_day) {
                    self.selected_logs.insert(file.index);
                }
            }
        }
        self.status = format!(
            "{} selected log(s) for current day",
            self.selected_logs.len()
        );
    }

    pub fn export_logs_same_day(&mut self) {
        self.select_logs_same_day();
        if self.selected_logs.is_empty() {
            return;
        }
        self.export_selected_logs();
    }

    pub fn clear_log_selection(&mut self) {
        self.selected_logs.clear();
        self.range_anchor = None;
        self.status = "log selection cleared".to_owned();
    }

    pub fn preview_selected_log(&mut self) {
        let Some(file) = self.logs.get(self.selected_log) else {
            return;
        };
        match self
            .controller
            .logs()
            .preview_log(self.device_channel, file.index, 40)
        {
            Ok(preview) => {
                self.preview = preview;
                self.status = format!("preview log {}", file.index);
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    pub fn export_selected_logs(&mut self) {
        if self.export_rx.is_some() {
            self.status = "export già in corso".to_owned();
            return;
        }

        let selected_files: Vec<LogFileInfo> = if self.selected_logs.is_empty() {
            self.logs
                .get(self.selected_log)
                .cloned()
                .map(|file| vec![file])
                .unwrap_or_default()
        } else {
            self.logs
                .iter()
                .filter(|file| self.selected_logs.contains(&file.index))
                .cloned()
                .collect()
        };

        if selected_files.is_empty() {
            self.status = "nessun log da esportare".to_owned();
            return;
        }

        let already_exported = load_export_manifest_keys();
        let files_to_export: Vec<LogFileInfo> = selected_files
            .into_iter()
            .filter(|file| !already_exported.contains(&manifest_key(file)))
            .collect();
        if files_to_export.is_empty() {
            self.status = "tutti i log selezionati sono già esportati".to_owned();
            return;
        }

        let configured_dbc = AppConfig::load().bindings();
        if configured_dbc.is_empty() {
            self.status =
                "nessun DBC configurato: usa `kvaser-utils logs dbc add --channel 0 --file /path/file.dbc`"
                    .to_owned();
            return;
        }
        let dbc_files: Vec<_> = configured_dbc
            .into_iter()
            .filter(|binding| binding.path.is_file())
            .collect();
        if dbc_files.is_empty() {
            self.status =
                "DBC configurati non trovati su filesystem; aggiorna config DBC".to_owned();
            return;
        }

        let logs: Vec<u32> = files_to_export.iter().map(|file| file.index).collect();
        self.export_pending_keys = files_to_export.iter().map(manifest_key).collect();
        let output = export_path(&files_to_export);
        let total_events: u64 = files_to_export
            .iter()
            .map(|file| file.event_count.max(0) as u64)
            .sum();
        self.status = format!("exporting 0/{total_events} events (0%)");

        let device_channel = self.device_channel;
        let (tx, rx) = mpsc::channel();
        self.export_rx = Some(rx);

        thread::spawn(move || {
            let controller = match AppController::load() {
                Ok(controller) => controller,
                Err(error) => {
                    let _ = tx.send(ExportMessage::Failed(error.to_string()));
                    return;
                }
            };

            let export_result = controller.logs().export_mf4_with_progress_and_dbc(
                device_channel,
                &logs,
                &output,
                true,
                &dbc_files,
                |progress| {
                    let _ = tx.send(ExportMessage::Progress(progress));
                },
            );

            match export_result {
                Ok(summary) => {
                    let _ = tx.send(ExportMessage::Done(summary));
                }
                Err(error) => {
                    let _ = tx.send(ExportMessage::Failed(error.to_string()));
                }
            }
        });
    }

    pub fn active_channel_indexes(&self) -> BTreeSet<u32> {
        self.controller
            .can()
            .active_channels()
            .into_iter()
            .map(|(channel, _)| channel)
            .collect()
    }

    pub fn is_range_anchor_row(&self, row: usize) -> bool {
        self.range_anchor == Some(row)
    }

    pub fn add_dbc_for_channel(&mut self, channel: u32, path: PathBuf) {
        let canonical = path.canonicalize().unwrap_or(path);
        if !canonical.is_file() {
            self.status = format!("DBC non trovato: {}", canonical.display());
            return;
        }

        let mut config = AppConfig::load();
        config.add_dbc_for_channel(channel, canonical.clone());
        match config.save() {
            Ok(()) => {
                self.status = format!(
                    "DBC salvato per CAN{}: {}",
                    channel + 1,
                    canonical.display()
                );
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn push_live_frames(&mut self, frames: Vec<CanFrame>) {
        for frame in frames {
            self.live_frames.push_back(frame);
        }
        while self.live_frames.len() > LIVE_FRAME_LIMIT {
            self.live_frames.pop_front();
        }
    }

    fn report_core(&mut self, result: CoreResult<()>) {
        match result {
            Ok(()) => self.status.clear(),
            Err(error) => self.status = error.to_string(),
        }
    }

    fn load_more_logs(&mut self) {
        if self.logs.len() >= self.logs_total {
            return;
        }

        let offset = self.logs.len() as u32;
        match self.controller.logs().list_logs_recent_page(
            self.device_channel,
            offset,
            LOG_PAGE_SIZE,
        ) {
            Ok((more, total)) => {
                self.logs_total = total as usize;
                self.logs.extend(more);
                self.status = format!("loaded {}/{} log file(s)", self.logs.len(), self.logs_total);
            }
            Err(error) => self.status = error.to_string(),
        }
    }

    fn probe_devices_if_needed(&mut self) {
        if !self.channels.is_empty() {
            self.last_device_probe = Instant::now();
            return;
        }

        if self.last_device_probe.elapsed() < DEVICE_PROBE_INTERVAL {
            return;
        }
        self.last_device_probe = Instant::now();

        let previous_count = self.channels.len();
        self.refresh_channels();
        if previous_count == 0 && !self.channels.is_empty() {
            self.refresh_device_mode();
            self.status = format!("device detected: {} CAN channel(s)", self.channels.len());
        }
    }

    fn poll_export_updates(&mut self) {
        let mut finished = false;
        if let Some(rx) = &self.export_rx {
            while let Ok(message) = rx.try_recv() {
                match message {
                    ExportMessage::Progress(progress) => {
                        self.status = format_export_progress(progress);
                    }
                    ExportMessage::Done(summary) => {
                        persist_exported_manifest_keys(&self.export_pending_keys, &summary.outputs);
                        self.export_pending_keys.clear();
                        self.status = format_export_summary(&summary);
                        finished = true;
                    }
                    ExportMessage::Failed(error) => {
                        self.export_pending_keys.clear();
                        self.status = error;
                        finished = true;
                    }
                }
            }
        }

        if finished {
            self.export_rx = None;
        }
    }
}

enum ExportMessage {
    Progress(ExportProgress),
    Done(ExportSummary),
    Failed(String),
}

fn next_bitrate(current: CanBitrate) -> CanBitrate {
    let presets = CanBitrate::PRESETS;
    let position = presets
        .iter()
        .position(|preset| *preset == current)
        .unwrap_or(0);
    presets[(position + 1) % presets.len()]
}

fn format_export_progress(progress: ExportProgress) -> String {
    if progress.total_events == 0 {
        return "exporting 0/0 events (0%)".to_owned();
    }
    let pct = ((progress.completed_events.saturating_mul(100)) / progress.total_events).min(100);
    format!(
        "exporting {}/{} events ({}%)",
        progress.completed_events, progress.total_events, pct
    )
}

fn format_export_summary(summary: &ExportSummary) -> String {
    if summary.outputs.len() <= 1 {
        return format!(
            "exported {} events to {}",
            summary.events,
            summary.output.display()
        );
    }

    format!(
        "exported {} events to {} (+{} more files)",
        summary.events,
        summary.output.display(),
        summary.outputs.len().saturating_sub(1)
    )
}

fn export_path(files: &[LogFileInfo]) -> PathBuf {
    let stamp = files
        .iter()
        .filter_map(|file| file.start_time_utc.or(file.end_time_utc))
        .min()
        .unwrap_or_else(current_unix_seconds_u32);

    let base = export_base_name(stamp);
    let dir = PathBuf::from(EXPORTS_DIR);
    let mut candidate = dir.join(format!("{base}.mf4"));
    let mut suffix = 2_u32;
    while candidate.exists() {
        candidate = dir.join(format!("{base}_{suffix}.mf4"));
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

fn current_unix_seconds_u32() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as u32)
        .unwrap_or_default()
}

fn local_day_key(unix_ts: u32) -> Option<(i32, i32)> {
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
        return None;
    }
    Some((tm.tm_year, tm.tm_yday))
}

fn manifest_path() -> PathBuf {
    PathBuf::from(EXPORTS_DIR).join(EXPORTED_MANIFEST)
}

fn manifest_key(file: &LogFileInfo) -> String {
    format!(
        "{}:{}:{}:{}",
        file.index,
        file.start_time_utc.unwrap_or(0),
        file.end_time_utc.unwrap_or(0),
        file.event_count
    )
}

fn load_export_manifest_keys() -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let Ok(content) = fs::read_to_string(manifest_path()) else {
        return set;
    };
    for line in content.lines() {
        let mut parts = line.splitn(3, '\t');
        let key = parts.next().unwrap_or("").trim();
        let _ts = parts.next().unwrap_or("").trim();
        let output_paths = parts.next().unwrap_or("").trim();
        if key.is_empty() || output_paths.is_empty() {
            continue;
        }
        if output_paths
            .split('|')
            .map(str::trim)
            .any(|output_path| !output_path.is_empty() && Path::new(output_path).is_file())
        {
            set.insert(key.to_owned());
        }
    }
    set
}

fn persist_exported_manifest_keys(keys: &[String], outputs: &[PathBuf]) {
    if keys.is_empty() || outputs.is_empty() {
        return;
    }
    let path = manifest_path();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        let _ = fs::create_dir_all(parent);
    }

    let mut existing = load_export_manifest_keys();
    let mut lines_to_append = Vec::new();
    let output_list = outputs
        .iter()
        .map(|output| output.display().to_string())
        .collect::<Vec<_>>()
        .join("|");
    for key in keys {
        if existing.insert(key.clone()) {
            lines_to_append.push(format!(
                "{key}\t{}\t{output_list}\n",
                current_unix_seconds_u32()
            ));
        }
    }
    if lines_to_append.is_empty() {
        return;
    }

    let mut content = String::new();
    if Path::new(&path).exists() {
        if let Ok(current) = fs::read_to_string(&path) {
            content.push_str(&current);
        }
    }
    for line in lines_to_append {
        content.push_str(&line);
    }
    let _ = fs::write(path, content);
}
