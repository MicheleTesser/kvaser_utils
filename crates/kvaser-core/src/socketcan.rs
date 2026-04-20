use std::process::Command;

use crate::{CanBitrate, CoreError, Result};

#[derive(Debug, Clone)]
pub struct SocketCanConfig {
    pub iface: String,
    pub bitrate: CanBitrate,
}

impl SocketCanConfig {
    pub fn new(iface: impl Into<String>, bitrate: CanBitrate) -> Self {
        Self {
            iface: iface.into(),
            bitrate,
        }
    }
}

pub struct SocketCanController;

impl SocketCanController {
    pub fn apply_pair(first: &SocketCanConfig, second: &SocketCanConfig) -> Result<()> {
        Self::set_up(first)?;
        Self::set_up(second)?;
        Ok(())
    }

    pub fn set_up(config: &SocketCanConfig) -> Result<()> {
        Self::run_ip(["link", "set", config.iface.as_str(), "down"])?;
        let bitrate = config.bitrate.as_bps().to_string();
        Self::run_ip([
            "link",
            "set",
            config.iface.as_str(),
            "up",
            "type",
            "can",
            "bitrate",
            bitrate.as_str(),
        ])
    }

    pub fn set_down(iface: &str) -> Result<()> {
        Self::run_ip(["link", "set", iface, "down"])
    }

    fn run_ip<const N: usize>(args: [&str; N]) -> Result<()> {
        let output = Command::new("ip").args(args).output()?;
        if output.status.success() {
            return Ok(());
        }

        Err(CoreError::SocketCanCommand {
            command: format!("ip {}", args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        })
    }
}
