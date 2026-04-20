use std::io::{self, Stdout};
use std::process::Command;
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::app::{App, Tab};
use crate::view;

type TuiTerminal = Terminal<CrosstermBackend<Stdout>>;

pub fn run(device_channel: u32) -> anyhow::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new(device_channel)?;

    let result = run_loop(&mut terminal, &mut app);
    restore_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> anyhow::Result<TuiTerminal> {
    enable_raw_mode().context("enabling terminal raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("entering alternate screen")?;
    Terminal::new(CrosstermBackend::new(stdout)).context("creating terminal")
}

fn restore_terminal(terminal: &mut TuiTerminal) -> anyhow::Result<()> {
    disable_raw_mode().context("disabling terminal raw mode")?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).context("leaving alternate screen")?;
    terminal.show_cursor().context("showing cursor")
}

fn run_loop(terminal: &mut TuiTerminal, app: &mut App) -> anyhow::Result<()> {
    loop {
        app.tick();
        terminal.draw(|frame| view::draw(frame, app))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => app.should_quit = true,
                    KeyCode::Char('1') => app.set_tab(Tab::Can),
                    KeyCode::Char('2') => app.set_tab(Tab::Logs),
                    KeyCode::Char('3') => app.set_tab(Tab::Live),
                    KeyCode::Char('s') => app.enable_usb_logging(),
                    KeyCode::Char('S') => app.configure_and_enable_usb_logging(),
                    KeyCode::Char('i') => app.enable_interface_mode(),
                    KeyCode::Char('r') => app.refresh(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Char('b') if app.tab == Tab::Can => app.cycle_bitrate(),
                    KeyCode::Char('a') if app.tab == Tab::Can => app.activate_pair(),
                    KeyCode::Char('o') if app.tab == Tab::Can => app.activate_selected(),
                    KeyCode::Char('f') if app.tab == Tab::Can => app.deactivate_selected(),
                    KeyCode::Char('D') if app.tab == Tab::Can || app.tab == Tab::Logs => {
                        let channels = if app.channels.len() <= 1 {
                            app.channels
                                .get(app.selected_channel)
                                .map(|channel| vec![channel.index])
                                .unwrap_or_else(|| vec![0])
                        } else {
                            vec![0_u32, 1_u32]
                        };
                        for (channel, path) in pick_dbc_files(terminal, &channels)? {
                            app.add_dbc_for_channel(channel, path);
                        }
                    }
                    KeyCode::Char(' ')
                        if app.tab == Tab::Logs && key.modifiers.contains(KeyModifiers::SHIFT) =>
                    {
                        app.start_range_selection()
                    }
                    KeyCode::Char('v') if app.tab == Tab::Logs => app.start_range_selection(),
                    KeyCode::Char(' ') if app.tab == Tab::Logs => app.toggle_selected_log(),
                    KeyCode::Char('A') if app.tab == Tab::Logs => app.select_all_logs(),
                    KeyCode::Char('c') if app.tab == Tab::Logs => app.clear_log_selection(),
                    KeyCode::Char('p') if app.tab == Tab::Logs => app.preview_selected_log(),
                    KeyCode::Char('e') if app.tab == Tab::Logs => app.export_selected_logs(),
                    KeyCode::Char('c') if app.tab == Tab::Live => app.clear_live_frames(),
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn pick_dbc_files(
    _terminal: &mut TuiTerminal,
    channels: &[u32],
) -> anyhow::Result<Vec<(u32, std::path::PathBuf)>> {
    let mut selected = Vec::new();
    for &channel in channels {
        let title = format!("Select DBC for CAN{}", channel + 1);
        let output = Command::new("bash")
            .arg("-lc")
            .env("KVASER_TUI_PICKER_TITLE", &title)
            .arg(picker_script())
            .output()
            .context("running external DBC picker")?;

        if !output.status.success() {
            continue;
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if !path.is_empty() {
            selected.push((channel, std::path::PathBuf::from(path)));
        }
    }
    Ok(selected)
}

fn picker_script() -> &'static str {
    r#"
title="${KVASER_TUI_PICKER_TITLE:-Select DBC}"
if [ -n "$KVASER_TUI_DBC_PICKER" ]; then
  eval "$KVASER_TUI_DBC_PICKER"
  exit $?
fi
if command -v zenity >/dev/null 2>&1; then
  zenity --file-selection --title="$title" --file-filter="DBC files | *.dbc" 2>/dev/null
  exit $?
fi
if command -v kdialog >/dev/null 2>&1; then
  kdialog --title "$title" --getopenfilename "$PWD" "*.dbc|DBC files" 2>/dev/null
  exit $?
fi
echo "No external picker found (zenity/kdialog)." >&2
exit 1
"#
}
