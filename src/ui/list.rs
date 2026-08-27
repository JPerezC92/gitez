//! List + Remove screens. Same UI; the parent decides which mode it's in.

use color_eyre::eyre::Result;
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState as RListState, Paragraph, Wrap},
    Frame,
};

use crate::git::config as gitcfg;
use crate::ssh::config as sshcfg;
use crate::ssh::keygen;
use crate::ui;

struct KeyDetail {
    alias: String,
    public_key: String,
    copied: bool,
}

pub struct ListState {
    pub aliases: Vec<String>,
    pub cursor: usize,
    pub confirm: bool,
    pub error: Option<String>,
    list_state: RListState,
    detail: Option<KeyDetail>,
}

impl ListState {
    pub fn new() -> Self {
        let aliases = sshcfg::list_managed_aliases().unwrap_or_default();
        let mut list_state = RListState::default();
        if !aliases.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            aliases,
            cursor: 0,
            confirm: false,
            error: None,
            list_state,
            detail: None,
        }
    }
}

pub fn draw(f: &mut Frame, state: &mut ListState, remove_mode: bool) {
    if let Some(detail) = &state.detail {
        draw_key_detail(f, detail);
        return;
    }

    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);

    let title = if remove_mode { " remove account " } else { " accounts " };
    let header = Paragraph::new(Line::from(Span::styled(
        format!(" {} configured accounts", state.aliases.len()),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )))
    .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(header, chunks[0]);

    if state.aliases.is_empty() {
        let p = Paragraph::new(Line::from(Span::styled(
            "  No accounts configured yet. Use 'Add account' on the home screen.",
            ui::dim(),
        )))
        .block(Block::default().borders(Borders::ALL));
        f.render_widget(p, chunks[1]);
    } else {
        let items: Vec<ListItem> = state
            .aliases
            .iter()
            .map(|alias| {
                let host = format!("github.com-{alias}");
                let line = Line::from(vec![
                    Span::styled(format!(" {alias}  "), Style::default().add_modifier(Modifier::BOLD)),
                    Span::styled(host, ui::dim()),
                ]);
                ListItem::new(line)
            })
            .collect();
        state.list_state.select(Some(state.cursor.min(state.aliases.len().saturating_sub(1))));
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("> ");
        f.render_stateful_widget(list, chunks[1], &mut state.list_state);
    }

    let footer_text = if let Some(err) = &state.error {
        Line::from(Span::styled(format!(" {err} "), ui::err()))
    } else if remove_mode && state.confirm {
        Line::from(Span::styled(
            " Press Enter again to confirm removal, Esc to cancel ",
            ui::err().add_modifier(Modifier::BOLD),
        ))
    } else if remove_mode {
        Line::from(Span::styled(
            " ↑/↓ to move   Enter: remove selected   Esc: back ",
            ui::dim(),
        ))
    } else {
        Line::from(Span::styled(
            " ↑/↓ to move   Enter: view public key   Esc: back ",
            ui::dim(),
        ))
    };
    let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn draw_key_detail(f: &mut Frame, detail: &KeyDetail) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4), Constraint::Length(3)])
        .split(area);

    let header = Paragraph::new(Line::from(Span::styled(
        format!(" Public key for '{}'", detail.alias),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )))
    .block(Block::default().borders(Borders::ALL).title(" public key "));
    f.render_widget(header, chunks[0]);

    draw_key_body(f, chunks[1], detail);

    let copy_hint = if detail.copied {
        Span::styled(" ✓ Copied!   Esc: back ", ui::ok())
    } else {
        Span::styled(" c: copy key   Esc: back ", ui::dim())
    };
    let footer = Paragraph::new(Line::from(copy_hint))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn draw_key_body(f: &mut Frame, area: Rect, detail: &KeyDetail) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![
        Line::from("Paste this into GitHub → Settings → SSH keys:"),
        Line::from(""),
    ];
    for chunk in detail.public_key.as_bytes().chunks(76) {
        lines.push(Line::from(Span::styled(
            String::from_utf8_lossy(chunk).into_owned(),
            ui::accent(),
        )));
    }
    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}

/// View-only mode. Returns true to go back to home.
pub fn handle_key_view(state: &mut ListState, code: KeyCode) -> bool {
    // Detail overlay open.
    if state.detail.is_some() {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => state.detail = None,
            KeyCode::Char('c') => {
                if let Some(d) = &mut state.detail {
                    if let Ok(mut ctx) = arboard::Clipboard::new() {
                        let _ = ctx.set_text(d.public_key.clone());
                        d.copied = true;
                    }
                }
            }
            _ => {}
        }
        return false;
    }

    match code {
        KeyCode::Esc | KeyCode::Char('q') => return true,
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
        KeyCode::Enter if !state.aliases.is_empty() => {
            let alias = &state.aliases[state.cursor];
            state.detail = load_key_detail(alias);
        }
        _ => {}
    }
    false
}

fn load_key_detail(alias: &str) -> Option<KeyDetail> {
    let key_path = sshcfg::key_path_for(alias).ok()??;
    let mut pub_path = key_path.clone();
    let pub_name = format!("{}.pub", pub_path.file_name()?.to_string_lossy());
    pub_path.set_file_name(pub_name);
    let public_key = keygen::read_public_key(&pub_path).ok()?;
    Some(KeyDetail { alias: alias.to_string(), public_key, copied: false })
}

pub enum RemoveOutcome {
    Stay,
    Back,
    Done(String),
}

pub fn handle_key_remove(state: &mut ListState, code: KeyCode) -> Result<RemoveOutcome> {
    if state.aliases.is_empty() {
        if matches!(code, KeyCode::Esc | KeyCode::Char('q')) {
            return Ok(RemoveOutcome::Back);
        }
        return Ok(RemoveOutcome::Stay);
    }

    match code {
        KeyCode::Esc => {
            if state.confirm {
                state.confirm = false;
                Ok(RemoveOutcome::Stay)
            } else {
                Ok(RemoveOutcome::Back)
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if state.cursor + 1 < state.aliases.len() {
                state.cursor += 1;
            }
            state.confirm = false;
            Ok(RemoveOutcome::Stay)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.cursor > 0 {
                state.cursor -= 1;
            }
            state.confirm = false;
            Ok(RemoveOutcome::Stay)
        }
        KeyCode::Enter => {
            if !state.confirm {
                state.confirm = true;
                Ok(RemoveOutcome::Stay)
            } else {
                let alias = state.aliases[state.cursor].clone();
                match perform_remove(&alias) {
                    Ok(()) => Ok(RemoveOutcome::Done(format!("Account '{alias}' removed."))),
                    Err(e) => {
                        state.error = Some(e.to_string());
                        state.confirm = false;
                        Ok(RemoveOutcome::Stay)
                    }
                }
            }
        }
        _ => Ok(RemoveOutcome::Stay),
    }
}

fn perform_remove(alias: &str) -> Result<()> {
    if let Some(key_path) = sshcfg::key_path_for(alias)? {
        let _ = std::fs::remove_file(&key_path);
        let mut pub_path = key_path.clone();
        let new_name = format!("{}.pub", pub_path.file_name().unwrap().to_string_lossy());
        pub_path.set_file_name(new_name);
        let _ = std::fs::remove_file(&pub_path);
    }
    sshcfg::remove_account(alias)?;
    gitcfg::remove_account(alias)?;
    Ok(())
}
