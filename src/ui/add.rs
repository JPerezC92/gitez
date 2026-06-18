//! Add-account wizard.
//!
//! Steps:
//!   Alias   — short name like "work" or "personal"
//!   Name    — git user.name
//!   Email   — git user.email
//!   Folder  — folder where this account's repos live
//!   Review  — confirm and execute
//!   Done    — show the public key + browser opened

use std::path::PathBuf;

use color_eyre::eyre::{eyre, Result};
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::account::Account;
use crate::git::config as gitcfg;
use crate::ssh::{config as sshcfg, keygen};
use crate::ui;

#[derive(PartialEq, Eq, Clone, Copy)]
enum Step {
    Alias,
    Name,
    Email,
    Folder,
    Review,
    Done,
}

pub struct AddState {
    step: Step,
    alias: String,
    name: String,
    email: String,
    /// Current folder input being typed.
    folder: String,
    /// All folders accumulated so far.
    folders: Vec<String>,
    /// Once the wizard runs successfully this holds the public key.
    public_key: Option<String>,
    /// Set to true after user presses 'c' to copy the public key.
    copied: bool,
    /// Error from validation or the apply step, shown on the Review screen.
    error: Option<String>,
}

impl AddState {
    pub fn new() -> Self {
        let default_folder = dirs::home_dir()
            .map(|h| h.join("dev").to_string_lossy().to_string())
            .unwrap_or_default();
        Self {
            step: Step::Alias,
            alias: String::new(),
            name: String::new(),
            email: String::new(),
            folder: default_folder,
            folders: Vec::new(),
            public_key: None,
            copied: false,
            error: None,
        }
    }
}

pub struct AddOutcome {
    pub message: Option<String>,
}

pub fn draw(f: &mut Frame, state: &AddState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // stepper
            Constraint::Length(4), // description
            Constraint::Min(6),    // body
            Constraint::Length(3), // footer
        ])
        .split(area);

    draw_stepper(f, chunks[0], state);
    draw_description(f, chunks[1], state);
    draw_body(f, chunks[2], state);
    draw_footer(f, chunks[3], state);
}

const STEP_LABELS: &[&str] = &["Alias", "Name", "Email", "Folders", "Review", "Done"];

fn step_index(step: Step) -> usize {
    match step {
        Step::Alias => 0,
        Step::Name => 1,
        Step::Email => 2,
        Step::Folder => 3,
        Step::Review => 4,
        Step::Done => 5,
    }
}

fn draw_stepper(f: &mut Frame, area: Rect, state: &AddState) {
    let current = step_index(state.step);
    let mut spans: Vec<Span> = Vec::new();
    for (i, label) in STEP_LABELS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ── ", ui::dim()));
        }
        let (icon, style) = if i < current {
            ("✓ ", ui::ok())
        } else if i == current {
            ("● ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        } else {
            ("○ ", ui::dim())
        };
        spans.push(Span::styled(format!("{icon}{label}"), style));
    }
    let p = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title(" add account "));
    f.render_widget(p, area);
}

fn draw_description(f: &mut Frame, area: Rect, state: &AddState) {
    let text = match state.step {
        Step::Alias => "A short name to identify this GitHub account (e.g. 'work', 'personal').\nBecomes part of your SSH host alias: github.com-<alias>.",
        Step::Name => "Your full name as it will appear on every commit made with this account.\nThis sets git user.name in a dedicated config file.",
        Step::Email => "The email address linked to this GitHub account.\nUsed in commit metadata and embedded in the SSH public key.",
        Step::Folder => "One or more directories where this account's repos will live.\nGit auto-applies this identity to any repo cloned inside these folders.",
        Step::Review => "Review all settings before applying.\ngitez will generate an SSH keypair and update ~/.ssh/config and ~/.gitconfig.",
        Step::Done => "SSH key generated and GitHub config applied.\nAdd the public key to GitHub — the settings page should have opened automatically.",
    };
    let p = Paragraph::new(text)
        .style(ui::dim())
        .wrap(Wrap { trim: false })
        .block(Block::default().borders(Borders::TOP | Borders::LEFT | Borders::RIGHT));
    f.render_widget(p, area);
}

fn draw_body(f: &mut Frame, area: Rect, state: &AddState) {
    match state.step {
        Step::Alias => draw_input(
            f,
            area,
            "Short name for this account (e.g. work, personal)",
            &state.alias,
            "letters, numbers, dashes only",
        ),
        Step::Name => draw_input(
            f,
            area,
            "Git user.name for this account",
            &state.name,
            "shown on every commit",
        ),
        Step::Email => draw_input(
            f,
            area,
            "Git user.email for this account",
            &state.email,
            "use the email tied to this GitHub account",
        ),
        Step::Folder => draw_folder_step(f, area, state),
        Step::Review => draw_review(f, area, state),
        Step::Done => draw_done(f, area, state),
    }
}

fn draw_input(f: &mut Frame, area: Rect, label: &str, value: &str, hint: &str) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    let label_p =
        Paragraph::new(Span::styled(label, Style::default().add_modifier(Modifier::BOLD)));
    f.render_widget(label_p, chunks[0]);

    let hint_p = Paragraph::new(Span::styled(hint, ui::dim()));
    f.render_widget(hint_p, chunks[1]);

    let display = format!("> {value}_");
    let input_p = Paragraph::new(Line::from(Span::styled(display, ui::accent())));
    f.render_widget(input_p, chunks[3]);
}

fn draw_folder_step(f: &mut Frame, area: Rect, state: &AddState) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(1),
        ])
        .split(inner);

    let hint = if state.folders.is_empty() {
        "Enter path, press Enter to add. Add as many as needed, then press Enter on empty line."
    } else {
        "Press Enter to add another folder, or Enter on empty line to continue."
    };

    f.render_widget(
        Paragraph::new(Span::styled(
            "Folders where this account's repos will live",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled(hint, ui::dim())),
        chunks[1],
    );

    let added: Vec<Line> = state
        .folders
        .iter()
        .map(|p| Line::from(Span::styled(format!("  ✓ {p}"), ui::ok())))
        .collect();
    f.render_widget(Paragraph::new(added), chunks[3]);

    let display = format!("> {}_", state.folder);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(display, ui::accent()))),
        chunks[4],
    );
}

fn draw_review(f: &mut Frame, area: Rect, state: &AddState) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let key_path = format!("~/.ssh/id_ed25519_{}", state.alias);
    let mut lines = vec![
        Line::from(Span::styled(
            "About to do the following:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("  • Generate ed25519 SSH key at {key_path}")),
        Line::from(format!(
            "  • Add Host github.com-{} to ~/.ssh/config",
            state.alias
        )),
        Line::from(format!("  • Create ~/.gitconfig-{}", state.alias)),
    ];
    for folder in &state.folders {
        lines.push(Line::from(format!(
            "  • Add includeIf for '{}' to ~/.gitconfig",
            folder
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("Name:  ", ui::dim()),
        Span::raw(state.name.clone()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Email: ", ui::dim()),
        Span::raw(state.email.clone()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Alias: ", ui::dim()),
        Span::raw(state.alias.clone()),
    ]));
    lines.push(Line::from(""));
    if let Some(err) = &state.error {
        lines.push(Line::from(Span::styled(format!("Error: {err}"), ui::err())));
    } else {
        lines.push(Line::from(Span::styled(
            "Press Enter to apply, Esc to go back.",
            ui::dim(),
        )));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}

fn draw_done(f: &mut Frame, area: Rect, state: &AddState) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![
        Line::from(Span::styled(
            "✓ Account configured.",
            ui::ok().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Your public key — paste this into GitHub:"),
        Line::from(""),
    ];
    if let Some(pk) = &state.public_key {
        for chunk in pk.as_bytes().chunks(78) {
            lines.push(Line::from(Span::styled(
                String::from_utf8_lossy(chunk).into_owned(),
                ui::accent(),
            )));
        }
    }
    lines.push(Line::from(""));
    if state.copied {
        lines.push(Line::from(Span::styled("✓ Copied to clipboard!", ui::ok())));
    } else {
        lines.push(Line::from(Span::styled("Press 'c' to copy key to clipboard.", ui::dim())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "GitHub's keys page should have opened in your browser.",
        ui::dim(),
    )));
    lines.push(Line::from(Span::styled(
        "If not, visit: https://github.com/settings/ssh/new",
        ui::dim(),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from("Press Enter to return to the menu."));

    let p = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(p, inner);
}

fn draw_footer(f: &mut Frame, area: Rect, state: &AddState) {
    let hint = match state.step {
        Step::Done => " c: copy key   Enter: back to menu ",
        Step::Review => " Enter: apply   Esc: back ",
        _ => " Enter: next   Esc: back ",
    };
    let p = Paragraph::new(Span::styled(hint, ui::dim()))
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

pub fn handle_key(state: &mut AddState, code: KeyCode) -> Result<Option<AddOutcome>> {
    match state.step {
        Step::Alias => match code {
            KeyCode::Char(c) => state.alias.push(c),
            KeyCode::Backspace => {
                state.alias.pop();
            }
            KeyCode::Enter => {
                if !state.alias.trim().is_empty() {
                    state.step = Step::Name;
                }
            }
            KeyCode::Esc => return Ok(Some(AddOutcome { message: None })),
            _ => {}
        },
        Step::Name => match code {
            KeyCode::Char(c) => state.name.push(c),
            KeyCode::Backspace => {
                state.name.pop();
            }
            KeyCode::Enter => {
                if !state.name.trim().is_empty() {
                    state.step = Step::Email;
                }
            }
            KeyCode::Esc => state.step = Step::Alias,
            _ => {}
        },
        Step::Email => match code {
            KeyCode::Char(c) => state.email.push(c),
            KeyCode::Backspace => {
                state.email.pop();
            }
            KeyCode::Enter => {
                if state.email.contains('@') {
                    state.step = Step::Folder;
                }
            }
            KeyCode::Esc => state.step = Step::Name,
            _ => {}
        },
        Step::Folder => match code {
            KeyCode::Char(c) => state.folder.push(c),
            KeyCode::Backspace => {
                state.folder.pop();
            }
            KeyCode::Enter => {
                if !state.folder.trim().is_empty() {
                    let f = state.folder.trim().to_string();
                    state.folders.push(f);
                    state.folder.clear();
                } else if !state.folders.is_empty() {
                    state.step = Step::Review;
                }
            }
            KeyCode::Esc => {
                if state.folders.is_empty() {
                    state.step = Step::Email;
                } else {
                    state.folders.pop();
                }
            }
            _ => {}
        },
        Step::Review => match code {
            KeyCode::Esc => state.step = Step::Folder,
            KeyCode::Enter => match apply(state) {
                Ok(pk) => {
                    state.public_key = Some(pk);
                    state.error = None;
                    state.step = Step::Done;
                    let _ = open::that("https://github.com/settings/ssh/new");
                }
                Err(e) => state.error = Some(e.to_string()),
            },
            _ => {}
        },
        Step::Done => match code {
            KeyCode::Char('c') => {
                if let Some(pk) = &state.public_key {
                    if let Ok(mut ctx) = arboard::Clipboard::new() {
                        let _ = ctx.set_text(pk.clone());
                        state.copied = true;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Esc => {
                let msg = format!("Account '{}' added.", state.alias);
                return Ok(Some(AddOutcome { message: Some(msg) }));
            }
            _ => {}
        },
    }
    Ok(None)
}

fn apply(state: &mut AddState) -> Result<String> {
    validate(state)?;
    let alias = state.alias.trim().to_string();
    let name = state.name.trim().to_string();
    let email = state.email.trim().to_string();
    let folders: Vec<PathBuf> = state
        .folders
        .iter()
        .map(|f| PathBuf::from(f.trim()))
        .collect();

    let key_path = sshcfg::ssh_dir()?.join(format!("id_ed25519_{alias}"));

    let account = Account {
        alias,
        name,
        email,
        folders,
        key_path: key_path.clone(),
    };

    keygen::generate_ed25519(&key_path, &account.email)?;
    sshcfg::upsert_account(&account)?;
    gitcfg::upsert_account(&account)?;

    let pub_path = account.pub_key_path();
    keygen::read_public_key(&pub_path)
}

fn validate(state: &AddState) -> Result<()> {
    let alias = state.alias.trim();
    if alias.is_empty() {
        return Err(eyre!("Alias cannot be empty"));
    }
    if !alias
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(eyre!("Alias may only contain letters, numbers, '-', '_'"));
    }
    if state.name.trim().is_empty() {
        return Err(eyre!("Name cannot be empty"));
    }
    if state.email.trim().is_empty() || !state.email.contains('@') {
        return Err(eyre!("Email looks invalid"));
    }
    if state.folders.is_empty() {
        return Err(eyre!("At least one folder is required"));
    }
    Ok(())
}
