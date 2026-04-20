use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use kvaser_core::CanBitrate;

#[derive(Debug, Parser)]
#[command(name = "kvaser-utils")]
#[command(about = "Linux utilities for Kvaser Memorator Pro 2")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Tui {
        #[arg(long, default_value_t = 0)]
        device_channel: u32,
    },
    Channels,
    Mode {
        #[arg(long, default_value_t = 0)]
        channel: u32,
        #[arg(value_enum)]
        mode: DeviceModeArg,
    },
    Logs {
        #[command(subcommand)]
        command: LogsCommand,
    },
    Socketcan {
        #[command(subcommand)]
        command: SocketCanCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum LogsCommand {
    List {
        #[arg(long, default_value_t = 0)]
        device_channel: u32,
    },
    Preview {
        #[arg(long, default_value_t = 0)]
        device_channel: u32,
        #[arg(long)]
        index: u32,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    Export {
        #[arg(long, default_value_t = 0)]
        device_channel: u32,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, value_delimiter = ',', num_args = 1..)]
        logs: Vec<u32>,
        #[arg(long, default_value_t = true)]
        overwrite: bool,
    },
    Reset {
        #[arg(long, default_value_t = 0)]
        device_channel: u32,
        #[arg(long, value_enum, default_value = "fat32")]
        filesystem: FormatFilesystem,
        #[arg(long, default_value_t = 0)]
        reserve_mb: u32,
        #[arg(long, default_value_t = 0)]
        database_mb: u32,
        #[arg(long)]
        yes: bool,
    },
    RecordUsb {
        #[arg(long, default_value_t = 0)]
        device_channel: u32,
        #[arg(long, default_value_t = 0)]
        can_channel: u32,
        #[arg(long)]
        both: bool,
        #[arg(long, default_value = "500k")]
        bitrate: CanBitrate,
        #[arg(long, default_value_t = 10)]
        seconds: u64,
        #[arg(long)]
        leave_logger: bool,
    },
    Dbc {
        #[command(subcommand)]
        command: DbcCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum DbcCommand {
    List,
    Add {
        #[arg(long)]
        channel: u32,
        #[arg(long)]
        file: PathBuf,
    },
    Remove {
        #[arg(long)]
        file: PathBuf,
    },
    ClearChannel {
        #[arg(long)]
        channel: u32,
    },
    Clear,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FormatFilesystem {
    Fat16,
    Fat32,
}

#[derive(Debug, Subcommand)]
pub enum SocketCanCommand {
    Up {
        #[arg(long, default_value = "can0")]
        if0: String,
        #[arg(long, default_value = "500k")]
        bitrate0: CanBitrate,
        #[arg(long, default_value = "can1")]
        if1: String,
        #[arg(long, default_value = "250k")]
        bitrate1: CanBitrate,
    },
    Down {
        #[arg(long, default_value = "can0")]
        iface: String,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DeviceModeArg {
    Interface,
    Logger,
}
