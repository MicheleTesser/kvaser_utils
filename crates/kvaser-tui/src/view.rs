use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{
    Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, TableState, Tabs, Wrap,
};

use crate::app::{App, Tab};
use crate::format::{
    format_can_data, format_can_flags, format_can_id, format_event, format_log_time,
};

pub fn draw(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(7),
            Constraint::Length(5),
            Constraint::Length(3),
        ])
        .split(area);

    draw_tabs(frame, app, chunks[0]);
    match app.tab {
        Tab::Can => draw_can(frame, app, chunks[1]),
        Tab::Logs => draw_logs(frame, app, chunks[1]),
        Tab::Live => draw_live(frame, app, chunks[1]),
    }
    draw_preview(frame, app, chunks[2]);
    draw_commands(frame, app, chunks[3]);
    draw_status(frame, app, chunks[4]);
}

fn draw_tabs(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let tabs = Tabs::new(vec!["CAN", "Logs", "Live"])
        .block(Block::default().borders(Borders::ALL).title("kvaser_utils"))
        .select(app.tab.index())
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_widget(tabs, area);
}

fn draw_can(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let active = app.active_channel_indexes();
    let rows = app.channels.iter().map(|channel| {
        let state = if active.contains(&channel.index) {
            "on"
        } else {
            "off"
        };
        Row::new(vec![
            Cell::from(channel.index.to_string()),
            Cell::from(state),
            Cell::from(app.bitrate_for_channel(channel.index).to_string()),
            Cell::from(
                channel
                    .description
                    .clone()
                    .unwrap_or_else(|| "-".to_owned()),
            ),
            Cell::from(
                channel
                    .serial
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
            ),
            Cell::from(if channel.logger { "yes" } else { "no" }),
        ])
    });

    let title = format!(
        "CAN channels | mode {}",
        app.device_mode
            .map(|mode| mode.to_string())
            .unwrap_or_else(|| "unknown".to_owned())
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Min(20),
            Constraint::Length(14),
            Constraint::Length(8),
        ],
    )
    .header(
        Row::new(vec!["ch", "bus", "bitrate", "device", "serial", "logger"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan));

    let mut state = TableState::default();
    if !app.channels.is_empty() {
        state.select(Some(app.selected_channel));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_live(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let rows = app.live_frames.iter().rev().map(|can_frame| {
        Row::new(vec![
            Cell::from(can_frame.channel.to_string()),
            Cell::from(can_frame.timestamp_ms.to_string()),
            Cell::from(format_can_id(can_frame)),
            Cell::from(can_frame.dlc.to_string()),
            Cell::from(format_can_flags(can_frame)),
            Cell::from(format_can_data(can_frame)),
        ])
    });

    let title = format!(
        "Live CAN frames | {} buffered | active {}",
        app.live_frames.len(),
        app.active_channel_indexes()
            .into_iter()
            .map(|channel| channel.to_string())
            .collect::<Vec<_>>()
            .join(",")
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(12),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Length(18),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(vec!["ch", "time_ms", "id", "dlc", "flags", "data"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(table, area);
}

fn draw_logs(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let rows = app.logs.iter().enumerate().map(|(row, file)| {
        let selected = app.selected_logs.contains(&file.index);
        let anchor = app.is_range_anchor_row(row);
        let marker = match (selected, anchor) {
            (true, true) => "*>",
            (true, false) => "* ",
            (false, true) => "->",
            (false, false) => "  ",
        };
        Row::new(vec![
            Cell::from(marker),
            Cell::from(file.index.to_string()),
            Cell::from(file.file_type.to_string()),
            Cell::from(file.event_count.to_string()),
            Cell::from(format_log_time(file.start_time_utc)),
            Cell::from(format_log_time(file.end_time_utc)),
        ])
    });

    let title = format!(
        "Memorator logs on device channel {} ({}/{})",
        app.device_channel,
        app.logs.len(),
        app.logs_total
    );
    let table = Table::new(
        rows,
        [
            Constraint::Length(4),
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(12),
            Constraint::Length(20),
            Constraint::Length(20),
        ],
    )
    .header(
        Row::new(vec!["", "idx", "type", "events", "start", "end"])
            .style(Style::default().add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(title))
    .row_highlight_style(Style::default().fg(Color::Black).bg(Color::Cyan));

    let visible_rows = area.height.saturating_sub(3) as usize;
    let centered_offset = if visible_rows == 0 || app.logs.is_empty() {
        0
    } else {
        let half = visible_rows / 2;
        let max_offset = app.logs.len().saturating_sub(visible_rows);
        app.selected_log.saturating_sub(half).min(max_offset)
    };

    let mut state = TableState::default().with_offset(centered_offset);
    if !app.logs.is_empty() {
        state.select(Some(app.selected_log));
    }
    frame.render_stateful_widget(table, area, &mut state);
}

fn draw_preview(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .preview
        .iter()
        .map(|event| ListItem::new(format_event(event)))
        .collect();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Preview"))
        .style(Style::default().fg(Color::White));
    frame.render_widget(list, area);
}

fn draw_commands(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from("Globali: 1 CAN | 2 Logs | 3 Live | r refresh | q quit"),
        Line::from("Modo: S config+logger USB | s solo logger USB | i interface"),
    ];

    lines.push(match app.tab {
        Tab::Can => {
            Line::from("CAN: b bitrate 500k/1M | a on CAN0+CAN1 | o on selected | f off | D picker DBC CAN1+CAN2 | j/k move")
        }
        Tab::Logs => {
            Line::from("Logs: v start-range (->) | space toggle/apply-range | A all | c clear | p preview | e export MF4 | D picker DBC CAN1+CAN2 | j/k move")
        }
        Tab::Live => Line::from("Live: c clear | usa 1 CAN per accendere canali e settare bitrate"),
    });
    lines.push(Line::from(
        "DBC CLI: `kvaser-utils logs dbc add --channel 0 --file /path/file.dbc`",
    ));

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Comandi"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

fn draw_status(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let paragraph = Paragraph::new(Line::from(app.status.as_str()))
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}
