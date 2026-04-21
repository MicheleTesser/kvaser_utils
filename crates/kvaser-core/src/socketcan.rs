use std::collections::BTreeMap;
use std::ffi::CString;
use std::io;
use std::mem::{self, size_of};
use std::os::fd::RawFd;
use std::ptr;
use std::process::Command;

use crate::{CanBitrate, CanFrame, CoreError, Result};

const KVASER_FLAG_RTR: u32 = 0x0001;
const KVASER_FLAG_EXT: u32 = 0x0004;
const KVASER_FLAG_ERROR_FRAME: u32 = 0x0020;
const KVASER_FLAG_TXACK: u32 = 0x0040;
const KVASER_FLAG_FD: u32 = 0x010000;
const KVASER_FLAG_FD_BRS: u32 = 0x020000;
const KVASER_FLAG_FD_ESI: u32 = 0x040000;

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

#[derive(Debug, Clone, Default)]
pub struct SocketCanTunnelConfig {
    pub channel_ifaces: BTreeMap<u32, String>,
}

impl SocketCanTunnelConfig {
    pub fn pair(if0: impl Into<String>, if1: impl Into<String>) -> Self {
        let mut channel_ifaces = BTreeMap::new();
        channel_ifaces.insert(0, if0.into());
        channel_ifaces.insert(1, if1.into());
        Self { channel_ifaces }
    }
}

pub struct LiveSocketCanTunnel {
    endpoints: BTreeMap<u32, CanRawEndpoint>,
}

impl LiveSocketCanTunnel {
    pub fn open(config: &SocketCanTunnelConfig) -> Result<Self> {
        let mut endpoints = BTreeMap::new();
        for (channel, iface) in &config.channel_ifaces {
            let endpoint = CanRawEndpoint::open(iface)?;
            endpoints.insert(*channel, endpoint);
        }
        Ok(Self { endpoints })
    }

    pub fn send_frames(&self, frames: &[CanFrame]) -> Result<()> {
        for frame in frames {
            if frame.flags & KVASER_FLAG_TXACK != 0 {
                continue;
            }
            let Some(endpoint) = self.endpoints.get(&frame.channel) else {
                continue;
            };
            endpoint.send(frame)?;
        }
        Ok(())
    }

    pub fn receive_frames(&self, limit_per_channel: usize) -> Result<Vec<CanFrame>> {
        let mut frames = Vec::new();
        for (channel, endpoint) in &self.endpoints {
            frames.extend(endpoint.read_available(*channel, limit_per_channel)?);
        }
        Ok(frames)
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

struct CanRawEndpoint {
    fd: RawFd,
    supports_fd: bool,
}

impl CanRawEndpoint {
    fn open(iface: &str) -> Result<Self> {
        let iface_c = CString::new(iface).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid interface name: {iface}"),
            )
        })?;

        let ifindex = unsafe { libc::if_nametoindex(iface_c.as_ptr()) };
        if ifindex == 0 {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("socketcan interface not found: {iface}"),
            )
            .into());
        }

        let fd = unsafe { libc::socket(libc::PF_CAN, libc::SOCK_RAW, libc::CAN_RAW) };
        if fd < 0 {
            return Err(io::Error::last_os_error().into());
        }

        let mut supports_fd = false;
        let enable_fd: libc::c_int = 1;
        let setsockopt_status = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_CAN_BASE + libc::CAN_RAW,
                5, // CAN_RAW_FD_FRAMES
                (&enable_fd as *const libc::c_int).cast::<libc::c_void>(),
                size_of::<libc::c_int>() as libc::socklen_t,
            )
        };
        if setsockopt_status == 0 {
            supports_fd = true;
        }

        let mut sockaddr: libc::sockaddr_can = unsafe { mem::zeroed() };
        sockaddr.can_family = libc::AF_CAN as libc::sa_family_t;
        sockaddr.can_ifindex = ifindex as libc::c_int;
        let bind_status = unsafe {
            libc::bind(
                fd,
                (&sockaddr as *const libc::sockaddr_can).cast::<libc::sockaddr>(),
                size_of::<libc::sockaddr_can>() as libc::socklen_t,
            )
        };
        if bind_status < 0 {
            let error = io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(error.into());
        }

        Ok(Self { fd, supports_fd })
    }

    fn send(&self, frame: &CanFrame) -> Result<()> {
        let raw_id = frame.id as u32;
        let mut can_id: libc::canid_t = if frame.flags & KVASER_FLAG_EXT != 0 {
            (raw_id & libc::CAN_EFF_MASK) | libc::CAN_EFF_FLAG
        } else {
            raw_id & libc::CAN_SFF_MASK
        };
        if frame.flags & KVASER_FLAG_RTR != 0 {
            can_id |= libc::CAN_RTR_FLAG;
        }
        if frame.flags & KVASER_FLAG_ERROR_FRAME != 0 {
            can_id |= libc::CAN_ERR_FLAG;
        }

        let data_len = frame.data.len().min(libc::CANFD_MAX_DLEN);
        let use_fd = frame.flags & KVASER_FLAG_FD != 0 || data_len > libc::CAN_MAX_DLEN;

        if use_fd {
            if !self.supports_fd {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "socket does not support CAN FD frames",
                )
                .into());
            }
            let mut packet: libc::canfd_frame = unsafe { mem::zeroed() };
            packet.can_id = can_id;
            packet.len = data_len as u8;
            if frame.flags & KVASER_FLAG_FD_BRS != 0 {
                packet.flags |= libc::CANFD_BRS as u8;
            }
            if frame.flags & KVASER_FLAG_FD_ESI != 0 {
                packet.flags |= libc::CANFD_ESI as u8;
            }
            packet.flags |= libc::CANFD_FDF as u8;
            packet.data[..data_len].copy_from_slice(&frame.data[..data_len]);

            let wrote = unsafe {
                libc::write(
                    self.fd,
                    (&packet as *const libc::canfd_frame).cast::<libc::c_void>(),
                    libc::CANFD_MTU,
                )
            };
            if wrote as usize != libc::CANFD_MTU {
                return Err(io::Error::last_os_error().into());
            }
            return Ok(());
        }

        let mut packet: libc::can_frame = unsafe { mem::zeroed() };
        packet.can_id = can_id;
        packet.can_dlc = data_len as u8;
        packet.data[..data_len].copy_from_slice(&frame.data[..data_len]);

        let wrote = unsafe {
            libc::write(
                self.fd,
                (&packet as *const libc::can_frame).cast::<libc::c_void>(),
                libc::CAN_MTU,
            )
        };
        if wrote as usize != libc::CAN_MTU {
            return Err(io::Error::last_os_error().into());
        }
        Ok(())
    }

    fn read_available(&self, channel: u32, limit: usize) -> Result<Vec<CanFrame>> {
        let mut frames = Vec::new();
        let mut buffer = [0_u8; libc::CANFD_MTU];

        for _ in 0..limit {
            let read = unsafe {
                libc::recv(
                    self.fd,
                    buffer.as_mut_ptr().cast::<libc::c_void>(),
                    buffer.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            if read < 0 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::WouldBlock {
                    break;
                }
                return Err(error.into());
            }

            let read = read as usize;
            if read == libc::CAN_MTU {
                let packet = unsafe {
                    ptr::read_unaligned(buffer.as_ptr().cast::<libc::can_frame>())
                };
                let mut flags = 0_u32;
                if packet.can_id & libc::CAN_RTR_FLAG != 0 {
                    flags |= KVASER_FLAG_RTR;
                }
                if packet.can_id & libc::CAN_EFF_FLAG != 0 {
                    flags |= KVASER_FLAG_EXT;
                }
                if packet.can_id & libc::CAN_ERR_FLAG != 0 {
                    flags |= KVASER_FLAG_ERROR_FRAME;
                }

                let id = if flags & KVASER_FLAG_EXT != 0 {
                    (packet.can_id & libc::CAN_EFF_MASK) as i64
                } else {
                    (packet.can_id & libc::CAN_SFF_MASK) as i64
                };
                let dlc = (packet.can_dlc as usize).min(libc::CAN_MAX_DLEN);
                frames.push(CanFrame {
                    channel,
                    id,
                    dlc: dlc as u32,
                    flags,
                    timestamp_ms: 0,
                    data: packet.data[..dlc].to_vec(),
                });
                continue;
            }

            if read == libc::CANFD_MTU {
                let packet = unsafe {
                    ptr::read_unaligned(buffer.as_ptr().cast::<libc::canfd_frame>())
                };
                let mut flags = KVASER_FLAG_FD;
                if packet.can_id & libc::CAN_RTR_FLAG != 0 {
                    flags |= KVASER_FLAG_RTR;
                }
                if packet.can_id & libc::CAN_EFF_FLAG != 0 {
                    flags |= KVASER_FLAG_EXT;
                }
                if packet.can_id & libc::CAN_ERR_FLAG != 0 {
                    flags |= KVASER_FLAG_ERROR_FRAME;
                }
                if packet.flags & libc::CANFD_BRS as u8 != 0 {
                    flags |= KVASER_FLAG_FD_BRS;
                }
                if packet.flags & libc::CANFD_ESI as u8 != 0 {
                    flags |= KVASER_FLAG_FD_ESI;
                }

                let id = if flags & KVASER_FLAG_EXT != 0 {
                    (packet.can_id & libc::CAN_EFF_MASK) as i64
                } else {
                    (packet.can_id & libc::CAN_SFF_MASK) as i64
                };
                let len = (packet.len as usize).min(libc::CANFD_MAX_DLEN);
                frames.push(CanFrame {
                    channel,
                    id,
                    dlc: len as u32,
                    flags,
                    timestamp_ms: 0,
                    data: packet.data[..len].to_vec(),
                });
            }
        }

        Ok(frames)
    }
}

impl Drop for CanRawEndpoint {
    fn drop(&mut self) {
        let _ = unsafe { libc::close(self.fd) };
    }
}
