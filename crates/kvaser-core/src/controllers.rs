use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use kvaser_api::{
    CanBitrate, CanChannel, CanDeviceMode, CanFrame, CanHandleGuard, Canlib, DbcBinding,
    ExportProgress, ExportSummary, KvaserRuntime, LogEventPreview, LogFileInfo,
    LoggerCanChannelConfig, LoggerConfigSummary, Memorator, MemoratorFileSystem,
};

use crate::error::{CoreError, Result};
use crate::socketcan::{LiveSocketCanTunnel, SocketCanTunnelConfig};

pub struct AppController {
    can: CanController,
    logs: LogController,
}

impl AppController {
    pub fn load() -> Result<Self> {
        let runtime = KvaserRuntime::load()?;
        Ok(Self {
            can: CanController::new(runtime.canlib()),
            logs: LogController::new(runtime.memorator()),
        })
    }

    pub fn can(&self) -> &CanController {
        &self.can
    }

    pub fn can_mut(&mut self) -> &mut CanController {
        &mut self.can
    }

    pub fn logs(&self) -> &LogController {
        &self.logs
    }
}

pub struct CanController {
    canlib: Arc<Canlib>,
    active: BTreeMap<u32, ActiveCanChannel>,
    live_tunnel: Option<LiveSocketCanTunnel>,
}

impl CanController {
    pub fn new(canlib: Arc<Canlib>) -> Self {
        Self {
            canlib,
            active: BTreeMap::new(),
            live_tunnel: None,
        }
    }

    pub fn list_channels(&self) -> Result<Vec<CanChannel>> {
        Ok(self.canlib.list_channels()?)
    }

    pub fn activate(&mut self, channel: u32, bitrate: CanBitrate) -> Result<()> {
        self.active.remove(&channel);
        let guard = self.canlib.open_on_bus(channel, bitrate)?;
        self.active
            .insert(channel, ActiveCanChannel { bitrate, guard });
        Ok(())
    }

    pub fn activate_pair(
        &mut self,
        first_channel: u32,
        first_bitrate: CanBitrate,
        second_channel: u32,
        second_bitrate: CanBitrate,
    ) -> Result<()> {
        self.active.remove(&first_channel);
        self.active.remove(&second_channel);

        let first = self.canlib.open_on_bus(first_channel, first_bitrate)?;
        let second = self.canlib.open_on_bus(second_channel, second_bitrate)?;

        self.active.insert(
            first_channel,
            ActiveCanChannel {
                bitrate: first_bitrate,
                guard: first,
            },
        );
        self.active.insert(
            second_channel,
            ActiveCanChannel {
                bitrate: second_bitrate,
                guard: second,
            },
        );
        Ok(())
    }

    pub fn deactivate(&mut self, channel: u32) {
        self.active.remove(&channel);
    }

    pub fn deactivate_all(&mut self) {
        self.active.clear();
    }

    pub fn configure_live_tunnel(&mut self, config: &SocketCanTunnelConfig) -> Result<()> {
        self.live_tunnel = Some(LiveSocketCanTunnel::open(config)?);
        Ok(())
    }

    pub fn disable_live_tunnel(&mut self) {
        self.live_tunnel = None;
    }

    pub fn active_channels(&self) -> Vec<(u32, CanBitrate)> {
        self.active
            .iter()
            .map(|(channel, active)| (*channel, active.bitrate))
            .collect()
    }

    pub fn is_active(&self, channel: u32) -> bool {
        self.active.contains_key(&channel)
    }

    pub fn set_device_mode(&mut self, channel: u32, mode: CanDeviceMode) -> Result<()> {
        self.deactivate_all();
        Ok(self.canlib.set_device_mode(channel, mode)?)
    }

    pub fn get_device_mode(&self, channel: u32) -> Result<CanDeviceMode> {
        Ok(self.canlib.get_device_mode(channel)?)
    }

    pub fn start_script(&self, channel: u32, slot: u32) -> Result<()> {
        Ok(self.canlib.start_script(channel, slot)?)
    }

    pub fn stop_script(&self, channel: u32, slot: u32) -> Result<()> {
        Ok(self.canlib.stop_script(channel, slot)?)
    }

    pub fn poll_live_frames(&self, limit_per_channel: usize) -> Result<Vec<CanFrame>> {
        let mut frames = Vec::new();
        for active in self.active.values() {
            frames.extend(active.guard.read_available(limit_per_channel)?);
        }
        Ok(frames)
    }

    pub fn poll_live_frames_and_tunnel(&self, limit_per_channel: usize) -> Result<Vec<CanFrame>> {
        let frames = self.poll_live_frames(limit_per_channel)?;
        if let Some(tunnel) = &self.live_tunnel {
            tunnel.send_frames(&frames)?;
            let inbound = tunnel.receive_frames(limit_per_channel)?;
            for frame in inbound {
                let Some(active) = self.active.get(&frame.channel) else {
                    continue;
                };
                active.guard.write_frame(&frame)?;
            }
        }
        Ok(frames)
    }
}

struct ActiveCanChannel {
    bitrate: CanBitrate,
    guard: CanHandleGuard,
}

pub struct LogController {
    memorator: Memorator,
}

impl LogController {
    pub fn new(memorator: Memorator) -> Self {
        Self { memorator }
    }

    pub fn list_logs(&self, device_channel: u32) -> Result<Vec<LogFileInfo>> {
        let device = self.memorator.open_device(device_channel)?;
        Ok(device.list_logs()?)
    }

    pub fn list_logs_recent_page(
        &self,
        device_channel: u32,
        offset: u32,
        limit: u32,
    ) -> Result<(Vec<LogFileInfo>, u32)> {
        let device = self.memorator.open_device(device_channel)?;
        Ok(device.list_logs_recent_page(offset, limit)?)
    }

    pub fn preview_log(
        &self,
        device_channel: u32,
        index: u32,
        limit: usize,
    ) -> Result<Vec<LogEventPreview>> {
        let device = self.memorator.open_device(device_channel)?;
        Ok(device.preview_log(index, limit)?)
    }

    pub fn export_mf4(
        &self,
        device_channel: u32,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
    ) -> Result<ExportSummary> {
        self.export_mf4_with_dbc(device_channel, selected_logs, output, overwrite, &[])
    }

    pub fn export_mf4_with_dbc(
        &self,
        device_channel: u32,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
        dbc_files: &[DbcBinding],
    ) -> Result<ExportSummary> {
        if selected_logs.is_empty() {
            return Err(CoreError::EmptyLogSelection);
        }

        let device = self.memorator.open_device(device_channel)?;
        Ok(device.export_mf4_with_dbc(selected_logs, output, overwrite, dbc_files)?)
    }

    pub fn export_mf4_with_progress<F>(
        &self,
        device_channel: u32,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
        on_progress: F,
    ) -> Result<ExportSummary>
    where
        F: FnMut(ExportProgress),
    {
        self.export_mf4_with_progress_and_dbc(
            device_channel,
            selected_logs,
            output,
            overwrite,
            &[],
            on_progress,
        )
    }

    pub fn export_mf4_with_progress_and_dbc<F>(
        &self,
        device_channel: u32,
        selected_logs: &[u32],
        output: impl AsRef<Path>,
        overwrite: bool,
        dbc_files: &[DbcBinding],
        on_progress: F,
    ) -> Result<ExportSummary>
    where
        F: FnMut(ExportProgress),
    {
        if selected_logs.is_empty() {
            return Err(CoreError::EmptyLogSelection);
        }

        let device = self.memorator.open_device(device_channel)?;
        Ok(device.export_mf4_with_progress_and_dbc(
            selected_logs,
            output,
            overwrite,
            dbc_files,
            on_progress,
        )?)
    }

    pub fn format_disk(
        &self,
        device_channel: u32,
        file_system: MemoratorFileSystem,
        reserve_space_mb: u32,
        database_space_mb: u32,
    ) -> Result<()> {
        Ok(self.memorator.format_disk(
            device_channel,
            file_system,
            reserve_space_mb,
            database_space_mb,
        )?)
    }

    pub fn write_log_all_config(
        &self,
        device_channel: u32,
        channels: &[LoggerCanChannelConfig],
    ) -> Result<LoggerConfigSummary> {
        Ok(self
            .memorator
            .write_log_all_config(device_channel, channels)?)
    }
}
