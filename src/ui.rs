use crate::process::{Entry, Scanner};
use anyhow::Result;
use cli_command_kit::Mode;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    widgets::{Paragraph, Row, Table, TableState},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    Up,
    Down,
    Terminate,
    Refresh,
    Quit,
}

pub fn action(event: Event) -> Action {
    let Event::Key(key) = event else {
        return Action::None;
    };
    if !matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
        return Action::None;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return Action::Quit;
    }
    match key.code {
        KeyCode::Char('j') | KeyCode::Down => Action::Down,
        KeyCode::Char('k') | KeyCode::Up => Action::Up,
        KeyCode::Char('x') => Action::Terminate,
        KeyCode::Char('r') => Action::Refresh,
        KeyCode::Char('q') => Action::Quit,
        _ => Action::None,
    }
}

#[derive(Default)]
pub struct Dashboard {
    entries: Vec<Entry>,
    table: TableState,
    status: String,
}
impl Dashboard {
    pub fn replace(&mut self, result: Result<Vec<Entry>>) {
        match result {
            Ok(entries) => {
                let previous = self
                    .table
                    .selected()
                    .and_then(|index| self.entries.get(index))
                    .map(|entry| (entry.pid, entry.port));
                let index = previous
                    .and_then(|previous| {
                        entries
                            .iter()
                            .position(|entry| (entry.pid, entry.port) == previous)
                    })
                    .unwrap_or_else(|| {
                        self.table
                            .selected()
                            .unwrap_or(0)
                            .min(entries.len().saturating_sub(1))
                    });
                self.table.select((!entries.is_empty()).then_some(index));
                self.status = if entries.is_empty() {
                    "No matching listening TCP ports under cwd".into()
                } else {
                    format!("Tracking {} listening process entries", entries.len())
                };
                self.entries = entries;
            }
            Err(error) => self.status = format!("refresh failed: {error:#}"),
        }
    }

    pub fn apply(&mut self, action: Action) {
        match action {
            Action::Up => self
                .table
                .select(self.table.selected().map(|index| index.saturating_sub(1))),
            Action::Down => self.table.select(
                self.table
                    .selected()
                    .map(|index| (index + 1).min(self.entries.len().saturating_sub(1))),
            ),
            Action::Terminate => {
                self.status = match self
                    .table
                    .selected()
                    .and_then(|index| self.entries.get(index))
                {
                    None => "no process selected".into(),
                    Some(entry) => match entry.terminate() {
                        Ok(()) => {
                            format!("Sent SIGTERM to PID {} on port {}", entry.pid, entry.port)
                        }
                        Err(error) => format!("{error:#}"),
                    },
                };
            }
            _ => {}
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, cwd: &Path, mode: Mode) {
        let [header, list, status, controls] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(frame.area());
        frame.render_widget(
            Paragraph::new(format!("cwd {}", cli_command_kit::display_path(cwd))),
            header,
        );
        let rows = self.entries.iter().map(|entry| {
            Row::new(vec![
                entry.pid.to_string(),
                entry.port.to_string(),
                entry.command.clone(),
            ])
        });
        let table = Table::new(
            rows,
            [
                Constraint::Length(7),
                Constraint::Length(6),
                Constraint::Min(0),
            ],
        )
        .header(Row::new(["PID", "PORT", "COMMAND"]))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol(if mode == Mode::Human { "> " } else { "* " });
        frame.render_stateful_widget(table, list, &mut self.table);
        frame.render_widget(Paragraph::new(self.status.clone()), status);
        frame.render_widget(
            Paragraph::new("j/k: select  x: SIGTERM  r: refresh  q: quit"),
            controls,
        );
    }
}

pub fn run(cwd: &Path, mode: Mode) -> Result<()> {
    let scanner = Scanner::new(cwd.into());
    let mut dashboard = Dashboard::default();
    dashboard.replace(scanner.scan());
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = ratatui::try_restore();
            return Err(error.into());
        }
    };
    let result = (|| {
        let mut refresh_at = Instant::now() + Duration::from_millis(750);
        let mut follow_up = None;
        loop {
            terminal.draw(|frame| dashboard.draw(frame, cwd, mode))?;
            let wake = follow_up.map_or(refresh_at, |follow_up: Instant| refresh_at.min(follow_up));
            if crossterm::event::poll(wake.saturating_duration_since(Instant::now()))? {
                let action = action(crossterm::event::read()?);
                if action == Action::Quit {
                    return Ok(());
                }
                if action == Action::Refresh {
                    dashboard.replace(scanner.scan());
                } else {
                    dashboard.apply(action);
                }
                if action == Action::Terminate {
                    follow_up = Some(Instant::now() + Duration::from_millis(200));
                }
            }
            let now = Instant::now();
            if now >= refresh_at || follow_up.is_some_and(|deadline| now >= deadline) {
                dashboard.replace(scanner.scan());
                refresh_at = Instant::now() + Duration::from_millis(750);
                follow_up = None;
            }
        }
    })();
    let restored = ratatui::try_restore();
    result.and_then(|()| restored.map_err(Into::into))
}
