mod app;
mod cli;
mod config;
mod format;
mod tui;
mod view;

use std::thread;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use kvaser_core::{
    AppController, CanDeviceMode, LoggerCanChannelConfig, MemoratorFileSystem, SocketCanConfig,
    SocketCanController,
};

use crate::cli::DeviceModeArg;
use crate::cli::{Cli, Command, DbcCommand, FormatFilesystem, LogsCommand, SocketCanCommand};
use crate::config::{AppConfig, config_path};
use crate::format::{format_channel, format_event, format_log_file};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::Tui { device_channel: 0 }) {
        Command::Tui { device_channel } => tui::run(device_channel),
        Command::Channels => list_channels(),
        Command::Mode { channel, mode } => run_mode(channel, mode),
        Command::Logs { command } => run_logs(command),
        Command::Socketcan { command } => run_socketcan(command),
    }
}

fn list_channels() -> anyhow::Result<()> {
    let controller = AppController::load().context("loading Kvaser runtime")?;
    for channel in controller.can().list_channels()? {
        println!("{}", format_channel(&channel));
    }
    Ok(())
}

fn run_mode(channel: u32, mode: DeviceModeArg) -> anyhow::Result<()> {
    let mut controller = AppController::load().context("loading Kvaser runtime")?;
    let target_mode = match mode {
        DeviceModeArg::Interface => CanDeviceMode::Interface,
        DeviceModeArg::Logger => CanDeviceMode::Logger,
    };
    controller.can_mut().set_device_mode(channel, target_mode)?;
    let current = controller.can().get_device_mode(channel)?;
    println!("channel {channel} mode={current}");
    Ok(())
}

fn run_logs(command: LogsCommand) -> anyhow::Result<()> {
    match command {
        LogsCommand::List { device_channel } => {
            let controller = AppController::load().context("loading Kvaser runtime")?;
            let mut files = controller.logs().list_logs(device_channel)?;
            files.sort_by(|a, b| {
                b.end_time_utc
                    .unwrap_or(0)
                    .cmp(&a.end_time_utc.unwrap_or(0))
                    .then_with(|| {
                        b.start_time_utc
                            .unwrap_or(0)
                            .cmp(&a.start_time_utc.unwrap_or(0))
                    })
                    .then_with(|| b.index.cmp(&a.index))
            });
            for file in files {
                println!("{}", format_log_file(&file));
            }
        }
        LogsCommand::Preview {
            device_channel,
            index,
            limit,
        } => {
            let controller = AppController::load().context("loading Kvaser runtime")?;
            for event in controller
                .logs()
                .preview_log(device_channel, index, limit)?
            {
                println!("{}", format_event(&event));
            }
        }
        LogsCommand::Export {
            device_channel,
            output,
            logs,
            overwrite,
        } => {
            let controller = AppController::load().context("loading Kvaser runtime")?;
            let dbc_files = AppConfig::load().bindings();
            let summary = controller.logs().export_mf4_with_dbc(
                device_channel,
                &logs,
                output,
                overwrite,
                &dbc_files,
            )?;
            if summary.outputs.len() <= 1 {
                println!(
                    "exported {} events from logs {:?} to {}",
                    summary.events,
                    summary.files,
                    summary.output.display()
                );
            } else {
                println!(
                    "exported {} events from logs {:?} to {} files: {}",
                    summary.events,
                    summary.files,
                    summary.outputs.len(),
                    summary
                        .outputs
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }
        }
        LogsCommand::Reset {
            device_channel,
            filesystem,
            reserve_mb,
            database_mb,
            yes,
        } => {
            if !yes {
                anyhow::bail!(
                    "refusing to format the Memorator SD card without --yes; this deletes all logs"
                );
            }

            let controller = AppController::load().context("loading Kvaser runtime")?;
            controller.logs().format_disk(
                device_channel,
                filesystem.into(),
                reserve_mb,
                database_mb,
            )?;
            println!(
                "formatted device channel {device_channel} as {} (reserve={} MB, database={} MB)",
                MemoratorFileSystem::from(filesystem),
                reserve_mb,
                database_mb
            );
        }
        LogsCommand::RecordUsb {
            device_channel,
            can_channel,
            both,
            bitrate,
            seconds,
            leave_logger,
        } => {
            let mut controller = AppController::load().context("loading Kvaser runtime")?;
            let channels = if both {
                vec![
                    LoggerCanChannelConfig::new(0, bitrate),
                    LoggerCanChannelConfig::new(1, bitrate),
                ]
            } else {
                vec![LoggerCanChannelConfig::new(can_channel, bitrate)]
            };
            let summary = controller
                .logs()
                .write_log_all_config(device_channel, &channels)?;

            println!(
                "wrote log-all config to device channel {}: {} ({} bytes)",
                summary.device_channel,
                summary
                    .channels
                    .iter()
                    .map(|channel| format!("CAN {} at {}", channel.channel, channel.bitrate))
                    .collect::<Vec<_>>()
                    .join(", "),
                summary.binary_size
            );
            for warning in summary.warnings {
                println!("config warning: {warning}");
            }

            controller
                .can_mut()
                .set_device_mode(can_channel, CanDeviceMode::Logger)?;
            println!("logger mode active for {seconds}s; keep sending CAN frames now");

            if seconds > 0 {
                thread::sleep(Duration::from_secs(seconds));
            }

            if !leave_logger {
                controller
                    .can_mut()
                    .set_device_mode(can_channel, CanDeviceMode::Interface)?;
                println!("interface mode restored");
            }

            for file in controller.logs().list_logs(device_channel)? {
                println!("{}", format_log_file(&file));
            }
        }
        LogsCommand::Dbc { command } => run_dbc(command)?,
    }
    Ok(())
}

fn run_dbc(command: DbcCommand) -> anyhow::Result<()> {
    let mut config = AppConfig::load();
    match command {
        DbcCommand::List => {
            println!("config: {}", config_path().display());
            if config.bindings().is_empty() {
                println!("no DBC configured");
            } else {
                for (channel, files) in &config.dbc_by_channel {
                    for file in files {
                        println!("CAN{} {}", channel + 1, file.display());
                    }
                }
                if !config.dbc_files.is_empty() {
                    for file in &config.dbc_files {
                        println!("all {}", file.display());
                    }
                }
            }
        }
        DbcCommand::Add { channel, file } => {
            let path = file.canonicalize().unwrap_or(file);
            if !path.is_file() {
                anyhow::bail!("DBC file not found: {}", path.display());
            }
            config.add_dbc_for_channel(channel, path.clone());
            config.save()?;
            println!("added DBC for CAN{}: {}", channel + 1, path.display());
        }
        DbcCommand::Remove { file } => {
            let path = file.canonicalize().unwrap_or(file);
            config.remove_dbc(&path);
            config.save()?;
            println!("removed DBC: {}", path.display());
        }
        DbcCommand::ClearChannel { channel } => {
            config.clear_channel(channel);
            config.save()?;
            println!("cleared DBC for CAN{}", channel + 1);
        }
        DbcCommand::Clear => {
            config.dbc_by_channel.clear();
            config.dbc_files.clear();
            config.save()?;
            println!("cleared all DBC files");
        }
    }
    Ok(())
}

impl From<FormatFilesystem> for MemoratorFileSystem {
    fn from(value: FormatFilesystem) -> Self {
        match value {
            FormatFilesystem::Fat16 => MemoratorFileSystem::Fat16,
            FormatFilesystem::Fat32 => MemoratorFileSystem::Fat32,
        }
    }
}

fn run_socketcan(command: SocketCanCommand) -> anyhow::Result<()> {
    match command {
        SocketCanCommand::Up {
            if0,
            bitrate0,
            if1,
            bitrate1,
        } => {
            let first = SocketCanConfig::new(if0, bitrate0);
            let second = SocketCanConfig::new(if1, bitrate1);
            SocketCanController::apply_pair(&first, &second)?;
            println!(
                "{}={} {}={}",
                first.iface, first.bitrate, second.iface, second.bitrate
            );
        }
        SocketCanCommand::Down { iface } => {
            SocketCanController::set_down(&iface)?;
            println!("{iface}=down");
        }
    }
    Ok(())
}
