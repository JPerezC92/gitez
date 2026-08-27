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
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use tui_textarea::TextArea;

use crate::account::Account;
use crate::git::config as gitcfg;
use crate::ssh::{config as sshcfg, keygen};
use crate::ui;

const FOLDER_PLACEHOLDER: &str = "e.g. /home/you/dev";

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
    alias: TextArea<'static>,
    name: TextArea<'static>,
    email: TextArea<'static>,
    /// Current folder input being typed.
    folder: TextArea<'static>,
    /// All folders accumulated so far.
    folders: Vec<String>,
    /// Once the wizard runs successfully this holds the public key.
    public_key: Option<String>,
    /// Set to true after user presses 'c' to copy the public key.
    copied: bool,
    /// Error from validation or the apply step, shown on the Review screen.
    error: Option<String>,
}

/// Build an empty single-line wizard input with the shared styling.
fn single_line_input(placeholder: &str) -> TextArea<'static> {
    styled_input(TextArea::default(), placeholder)
}

/// Apply the shared single-line wizard-input styling to a textarea that may
/// already hold text (e.g. the prefilled default folder).
fn styled_input(mut ta: TextArea<'static>, placeholder: &str) -> TextArea<'static> {
    ta.remove_line_number();
    ta.set_placeholder_text(placeholder);
    ta.set_placeholder_style(ui::dim());
    ta.set_style(ui::accent());
    ta
}

/// Single-line textarea contents as an owned `String`.
fn text(t: &TextArea) -> String {
    t.lines().join("")
}

impl AddState {
    pub fn new() -> Self {
        let default_folder = dirs::home_dir()
            .map(|h| h.join("dev").to_string_lossy().to_string())
            .unwrap_or_default();
        let mut folder = styled_input(TextArea::new(vec![default_folder]), FOLDER_PLACEHOLDER);
        folder.move_cursor(tui_textarea::CursorMove::End);
        Self {
            step: Step::Alias,
            alias: single_line_input("e.g. work"),
            name: single_line_input("e.g. Ada Lovelace"),
            email: single_line_input("e.g. you@example.com"),
            folder,
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

fn draw_input(f: &mut Frame, area: Rect, label: &str, input: &TextArea<'_>, hint: &str) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // label
            Constraint::Length(1), // hint
            Constraint::Min(3),    // input (multi-row so the cursor renders)
        ])
        .split(inner);

    let label_p =
        Paragraph::new(Span::styled(label, Style::default().add_modifier(Modifier::BOLD)));
    f.render_widget(label_p, chunks[0]);

    let hint_p = Paragraph::new(Span::styled(hint, ui::dim()));
    f.render_widget(hint_p, chunks[1]);

    f.render_widget(input, chunks[2]);
}

fn draw_folder_step(f: &mut Frame, area: Rect, state: &AddState) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // label
            Constraint::Length(1), // hint
            Constraint::Min(2),    // added folders
            Constraint::Length(3), // input (multi-row so the cursor renders)
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
    f.render_widget(Paragraph::new(added), chunks[2]);

    f.render_widget(&state.folder, chunks[3]);
}

fn draw_review(f: &mut Frame, area: Rect, state: &AddState) {
    let block = Block::default().borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let alias = text(&state.alias);
    let key_path = format!("~/.ssh/id_ed25519_{alias}");
    let mut lines = vec![
        Line::from(Span::styled(
            "About to do the following:",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(format!("  • Generate ed25519 SSH key at {key_path}")),
        Line::from(format!(
            "  • Add Host github.com-{} to ~/.ssh/config",
            alias
        )),
        Line::from(format!("  • Create ~/.gitconfig-{alias}")),
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
        Span::raw(text(&state.name)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Email: ", ui::dim()),
        Span::raw(text(&state.email)),
    ]));
    lines.push(Line::from(vec![
        Span::styled("Alias: ", ui::dim()),
        Span::raw(alias),
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

pub fn handle_key(state: &mut AddState, key: KeyEvent) -> Result<Option<AddOutcome>> {
    match state.step {
        Step::Alias => match key.code {
            KeyCode::Enter => {
                if !text(&state.alias).trim().is_empty() {
                    state.step = Step::Name;
                }
            }
            KeyCode::Esc => return Ok(Some(AddOutcome { message: None })),
            KeyCode::Tab => {}
            _ => {
                state.alias.input(key);
            }
        },
        Step::Name => match key.code {
            KeyCode::Enter => {
                if !text(&state.name).trim().is_empty() {
                    state.step = Step::Email;
                }
            }
            KeyCode::Esc => state.step = Step::Alias,
            KeyCode::Tab => {}
            _ => {
                state.name.input(key);
            }
        },
        Step::Email => match key.code {
            KeyCode::Enter => {
                if text(&state.email).contains('@') {
                    state.step = Step::Folder;
                }
            }
            KeyCode::Esc => state.step = Step::Name,
            KeyCode::Tab => {}
            _ => {
                state.email.input(key);
            }
        },
        Step::Folder => match key.code {
            KeyCode::Enter => {
                let folder = text(&state.folder);
                if !folder.trim().is_empty() {
                    state.folders.push(folder.trim().to_string());
                    state.folder = single_line_input(FOLDER_PLACEHOLDER);
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
            KeyCode::Tab => {}
            _ => {
                state.folder.input(key);
            }
        },
        Step::Review => match key.code {
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
        Step::Done => match key.code {
            KeyCode::Char('c') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(pk) = &state.public_key {
                    if let Ok(mut ctx) = arboard::Clipboard::new() {
                        let _ = ctx.set_text(pk.clone());
                        state.copied = true;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Esc => {
                let msg = format!("Account '{}' added.", text(&state.alias));
                return Ok(Some(AddOutcome { message: Some(msg) }));
            }
            _ => {}
        },
    }
    Ok(None)
}

fn apply(state: &mut AddState) -> Result<String> {
    validate(state)?;
    let alias = text(&state.alias).trim().to_string();
    let name = text(&state.name).trim().to_string();
    let email = text(&state.email).trim().to_string();
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
    let alias = text(&state.alias);
    let alias = alias.trim();
    if alias.is_empty() {
        return Err(eyre!("Alias cannot be empty"));
    }
    if !alias
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(eyre!("Alias may only contain letters, numbers, '-', '_'"));
    }
    if text(&state.name).trim().is_empty() {
        return Err(eyre!("Name cannot be empty"));
    }
    let email = text(&state.email);
    if email.trim().is_empty() || !email.contains('@') {
        return Err(eyre!("Email looks invalid"));
    }
    if state.folders.is_empty() {
        return Err(eyre!("At least one folder is required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kev(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    fn type_str(state: &mut AddState, s: &str) {
        for c in s.chars() {
            handle_key(state, kev(KeyCode::Char(c), KeyModifiers::NONE)).unwrap();
        }
    }

    fn enter(state: &mut AddState) {
        handle_key(state, kev(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    }

    #[test]
    fn edits_apply_at_cursor() {
        let mut s = AddState::new();
        type_str(&mut s, "ab");
        handle_key(&mut s, kev(KeyCode::Left, KeyModifiers::NONE)).unwrap();
        handle_key(&mut s, kev(KeyCode::Backspace, KeyModifiers::NONE)).unwrap();
        handle_key(&mut s, kev(KeyCode::Char('x'), KeyModifiers::NONE)).unwrap();
        assert_eq!(text(&s.alias), "xb");
    }

    #[test]
    fn ctrl_arrows_jump_words() {
        let mut s = AddState::new();
        type_str(&mut s, "hi there");
        assert_eq!(s.alias.cursor(), (0, 8));
        // Jump within already-typed text: back to the start first.
        handle_key(&mut s, kev(KeyCode::Home, KeyModifiers::NONE)).unwrap();
        assert_eq!(s.alias.cursor(), (0, 0));
        handle_key(&mut s, kev(KeyCode::Right, KeyModifiers::CONTROL)).unwrap();
        assert_eq!(s.alias.cursor(), (0, 3));
        handle_key(
            &mut s,
            kev(
                KeyCode::Right,
                KeyModifiers::CONTROL | KeyModifiers::SHIFT,
            ),
        )
        .unwrap();
        assert_eq!(s.alias.cursor(), (0, 8));
    }

    #[test]
    fn backspace_deletes_selection() {
        let mut s = AddState::new();
        type_str(&mut s, "ab");
        handle_key(&mut s, kev(KeyCode::Home, KeyModifiers::NONE)).unwrap();
        handle_key(&mut s, kev(KeyCode::Right, KeyModifiers::SHIFT)).unwrap();
        handle_key(&mut s, kev(KeyCode::Backspace, KeyModifiers::NONE)).unwrap();
        assert_eq!(text(&s.alias), "b");
    }

    #[test]
    fn modifier_chords_do_not_insert() {
        let mut s = AddState::new();
        type_str(&mut s, "ab");
        handle_key(&mut s, kev(KeyCode::Char('x'), KeyModifiers::CONTROL)).unwrap();
        handle_key(&mut s, kev(KeyCode::Char('x'), KeyModifiers::ALT)).unwrap();
        assert_eq!(text(&s.alias), "ab");
    }

    #[test]
    fn enter_validation_and_esc_back() {
        let mut s = AddState::new();
        enter(&mut s);
        assert!(matches!(s.step, Step::Alias));
        type_str(&mut s, "work");
        enter(&mut s);
        assert!(matches!(s.step, Step::Name));
        handle_key(&mut s, kev(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
        assert!(matches!(s.step, Step::Alias));
    }

    #[test]
    fn folder_step_adds_and_pops() {
        let mut s = AddState::new();
        type_str(&mut s, "work");
        enter(&mut s);
        type_str(&mut s, "Ada Lovelace");
        enter(&mut s);
        type_str(&mut s, "a@b.c");
        enter(&mut s);
        assert!(matches!(s.step, Step::Folder));

        // Clear the prefilled default folder (path is environment-dependent).
        handle_key(&mut s, kev(KeyCode::Home, KeyModifiers::NONE)).unwrap();
        handle_key(&mut s, kev(KeyCode::End, KeyModifiers::SHIFT)).unwrap();
        handle_key(&mut s, kev(KeyCode::Backspace, KeyModifiers::NONE)).unwrap();
        assert!(text(&s.folder).is_empty());

        type_str(&mut s, "/tmp/repos");
        enter(&mut s);
        assert_eq!(s.folders, vec!["/tmp/repos"]);
        assert!(text(&s.folder).is_empty());

        type_str(&mut s, "/tmp/other");
        enter(&mut s);
        assert_eq!(s.folders, vec!["/tmp/repos", "/tmp/other"]);

        handle_key(&mut s, kev(KeyCode::Esc, KeyModifiers::NONE)).unwrap();
        assert_eq!(s.folders, vec!["/tmp/repos"]);
        assert!(matches!(s.step, Step::Folder));
    }

    #[test]
    fn email_without_at_stays_on_email() {
        let mut s = AddState::new();
        type_str(&mut s, "work");
        enter(&mut s);
        type_str(&mut s, "Ada");
        enter(&mut s);
        assert!(matches!(s.step, Step::Email));
        type_str(&mut s, "nope");
        enter(&mut s);
        assert!(matches!(s.step, Step::Email));
    }

    #[test]
    fn done_ctrl_c_does_not_copy() {
        let mut s = AddState::new();
        s.step = Step::Done;
        s.public_key = Some("ssh-ed25519 AAAATEST test".to_string());
        handle_key(&mut s, kev(KeyCode::Char('c'), KeyModifiers::CONTROL)).unwrap();
        assert!(!s.copied);
    }
}
