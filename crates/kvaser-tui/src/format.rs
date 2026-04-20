use kvaser_core::{CanChannel, CanFrame, LogEventKind, LogEventPreview, LogFileInfo};

pub fn format_channel(channel: &CanChannel) -> String {
    format!(
        "ch={} name={} device={} serial={} logger={} can_fd={}",
        channel.index,
        channel.name.as_deref().unwrap_or("-"),
        channel.description.as_deref().unwrap_or("-"),
        channel
            .serial
            .map(|serial| serial.to_string())
            .unwrap_or_else(|| "-".to_owned()),
        channel.logger,
        channel.can_fd
    )
}

pub fn format_log_file(file: &LogFileInfo) -> String {
    format!(
        "idx={} type={} events={} start={} end={}",
        file.index,
        file.file_type,
        file.event_count,
        format_log_time(file.start_time_utc),
        format_log_time(file.end_time_utc)
    )
}

pub fn format_event(event: &LogEventPreview) -> String {
    match event.kind {
        LogEventKind::CanMessage | LogEventKind::LinMessage => format!(
            "{:?} ts={} ch={} id={} dlc={} flags=0x{:x} data={}",
            event.kind,
            event
                .timestamp_ns
                .map(|ts| ts.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            event
                .channel
                .map(|channel| channel.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            event
                .id
                .map(|id| format!("0x{id:x}"))
                .unwrap_or_else(|| "-".to_owned()),
            event
                .dlc
                .map(|dlc| dlc.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            event.flags.unwrap_or_default(),
            format_data(&event.data)
        ),
        _ => format!(
            "{:?} ts={}",
            event.kind,
            event
                .timestamp_ns
                .map(|ts| ts.to_string())
                .unwrap_or_else(|| "-".to_owned())
        ),
    }
}

pub fn format_can_id(frame: &CanFrame) -> String {
    if frame.id >= 0 {
        format!("0x{:x}", frame.id)
    } else {
        frame.id.to_string()
    }
}

pub fn format_can_flags(frame: &CanFrame) -> String {
    let labels = frame.flag_labels();
    if labels.is_empty() {
        format!("0x{:x}", frame.flags)
    } else {
        labels.join(",")
    }
}

pub fn format_can_data(frame: &CanFrame) -> String {
    format_data(&frame.data)
}

pub fn format_log_time(time: Option<u32>) -> String {
    time.map(format_unix_local)
        .unwrap_or_else(|| "-".to_owned())
}

fn format_data(data: &[u8]) -> String {
    if data.is_empty() {
        return "-".to_owned();
    }

    data.iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_unix_local(value: u32) -> String {
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
    let seconds = value as libc::time_t;
    let result = unsafe { libc::localtime_r(&seconds, &mut tm) };
    if result.is_null() {
        return value.to_string();
    }

    format!(
        "{:04}/{:02}/{:02} {:02}.{:02}.{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}
