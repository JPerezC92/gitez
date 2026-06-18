//! Test connection screen — runs `ssh -T git@github.com-<alias>`
//! and shows the output.

use std::process::Command;

use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState as RListState, Paragraph, Wrap},
    Frame,
};

use crate::ssh::config as sshcfg;
use crate::ui;

enum Phase {
    Picking,
    Result {
        alias: String,
        success: bool,
        output: String,
    },
}

pub struct TestState {
    aliases: Vec<String>,
    cursor: usize,
    list_state: RListState,
    phase: Phase,
}

impl TestState {
    pub fn new() -> Self {
        let aliases = sshcfg::list_managed_aliases().unwrap_or_default();
        let mut list_state = RListState::default();
        if !aliases.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            aliases,
            cursor: 0,
            list_state,
            phase: Phase::Picking,
        }
    }
}

pub fn draw(f: &mut Frame, state: &mut TestState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let header = Paragraph::new(Line::from(Span::styled(
        " Test SSH connection to GitHub",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" test connection "),
    );
    f.render_widget(header, chunks[0]);

    // Decide which body to draw without keeping a borrow on state.phase alive
    // while we also need other fields of state.
    let is_picking = matches!(state.phase, Phase::Picking);
    if is_picking {
        draw_picking(
            f,
            chunks[1],
            &state.aliases,
            state.cursor,
            &mut state.list_state,
        );
    } else if let Phase::Result {
        alias,
        success,
        output,
    } = &state.phase
    {
        draw_result(f, chunks[1], alias, *success, output);
    }

    let footer_text = if is_picking {
        " ↑/↓ to move   Enter: test   Esc: back "
    } else {
        " Enter or Esc: back "
    };
    let footer = Paragraph::new(Span::styled(footer_text, ui::dim()))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn draw_picking(
    f: &mut Frame,
    area: Rect,
    aliases: &[String],
    cursor: usize,
    list_state: &mut RListState,
) {
    if aliases.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            "  No accounts to test. Add one first from the home screen.",
            ui::dim(),
        )))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = aliases
        .iter()
        .map(|alias| {
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {alias}  "),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("github.com-{alias}"), ui::dim()),
            ]))
        })
        .collect();
    list_state.select(Some(cursor.min(aliases.len().saturating_sub(1))));
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, area, list_state);
}

fn draw_result(f: &mut Frame, area: Rect, alias: &str, success: bool, output: &str) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let status_line = if success {
        Line::from(Span::styled(
            format!("✓ '{alias}' authenticated to GitHub successfully."),
            ui::ok().add_modifier(Modifier::BOLD),
        ))
    } else {
        Line::from(Span::styled(
            format!("✗ '{alias}' could not authenticate. Details below."),
            ui::err().add_modifier(Modifier::BOLD),
        ))
    };

    let mut lines = vec![status_line, Line::from("")];
    for raw in output.lines() {
        lines.push(Line::from(raw.to_string()));
    }
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}

/// Returns true to return to home.
pub fn handle_key(state: &mut TestState, code: KeyCode) -> Result<bool> {
    let is_picking = matches!(state.phase, Phase::Picking);
    if is_picking {
        if state.aliases.is_empty() {
            if matches!(code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                return Ok(true);
            }
            return Ok(false);
        }
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(true),
            KeyCode::Down | KeyCode::Char('j') => {
                if state.cursor + 1 < state.aliases.len() {
                    state.cursor += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if state.cursor > 0 {
                    state.cursor -= 1;
                }
            }
            KeyCode::Enter => {
                let alias = state.aliases[state.cursor].clone();
                let (success, output) = run_test(&alias);
                state.phase = Phase::Result {
                    alias,
                    success,
                    output,
                };
            }
            _ => {}
        }
    } else if matches!(code, KeyCode::Enter | KeyCode::Esc | KeyCode::Char('q')) {
        return Ok(true);
    }
    Ok(false)
}

/// Runs `ssh -T git@github.com-<alias>`. GitHub returns exit code 1
/// even on success (no shell granted), so we detect success by looking
/// for "successfully authenticated" in the output.
fn run_test(alias: &str) -> (bool, String) {
    let host = format!("git@github.com-{alias}");
    let output = Command::new("ssh")
        .arg("-T")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg(&host)
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let combined = format!("{stdout}{stderr}");
            let success = combined.contains("successfully authenticated");
            (success, combined)
        }
        Err(e) => (false, format!("failed to run ssh: {e}")),
    }
}
