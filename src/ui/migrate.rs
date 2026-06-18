//! Guide: switching existing repos from HTTPS to SSH.

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::Modifier,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::ui;

const LINES: &[(&str, LineKind)] = &[
    ("Why switch?", LineKind::Heading),
    ("", LineKind::Plain),
    (
        "HTTPS auth uses a credential helper (token/password). SSH uses your key — \
         no tokens to rotate, works with gitez multi-account setup automatically.",
        LineKind::Plain,
    ),
    ("", LineKind::Plain),
    ("Step 1 — confirm your gitez account is set up", LineKind::Heading),
    ("", LineKind::Plain),
    (
        "Run gitez and add an account if you haven't. Note the alias you chose (e.g. \"work\").",
        LineKind::Plain,
    ),
    ("", LineKind::Plain),
    ("Step 2 — find repos using HTTPS", LineKind::Heading),
    ("", LineKind::Plain),
    ("Inside any repo run:", LineKind::Plain),
    ("", LineKind::Plain),
    ("  git remote -v", LineKind::Code),
    ("", LineKind::Plain),
    (
        "HTTPS remotes look like:  https://github.com/user/repo.git",
        LineKind::Dim,
    ),
    (
        "SSH remotes look like:    git@github.com:user/repo.git",
        LineKind::Dim,
    ),
    ("", LineKind::Plain),
    ("Step 3 — switch the remote URL", LineKind::Heading),
    ("", LineKind::Plain),
    ("Run this inside each repo (replace values in angle brackets):", LineKind::Plain),
    ("", LineKind::Plain),
    (
        "  git remote set-url origin git@github.com-<alias>:<user>/<repo>.git",
        LineKind::Code,
    ),
    ("", LineKind::Plain),
    ("  <alias>      your gitez alias, e.g. work", LineKind::Dim),
    ("  <user>       GitHub username or org", LineKind::Dim),
    ("  <repo>       repository name", LineKind::Dim),
    ("", LineKind::Plain),
    ("Example:", LineKind::Plain),
    ("", LineKind::Plain),
    (
        "  git remote set-url origin git@github.com-work:acme/backend.git",
        LineKind::Code,
    ),
    ("", LineKind::Plain),
    ("Step 4 — verify", LineKind::Heading),
    ("", LineKind::Plain),
    ("  git remote -v", LineKind::Code),
    ("", LineKind::Plain),
    (
        "Should now show git@github.com-<alias>:... for both fetch and push.",
        LineKind::Plain,
    ),
    ("", LineKind::Plain),
    ("Step 5 — test auth", LineKind::Heading),
    ("", LineKind::Plain),
    (
        "Use \"Test connection\" from the gitez main menu to confirm SSH auth works.",
        LineKind::Plain,
    ),
    ("", LineKind::Plain),
    ("Step 6 — optional: clear the credential helper", LineKind::Heading),
    ("", LineKind::Plain),
    (
        "Once all repos are on SSH you can stop the credential helper prompting \
         for HTTPS tokens. To disable it for GitHub only:",
        LineKind::Plain,
    ),
    ("", LineKind::Plain),
    (
        "  git config --global --unset credential.https://github.com.helper",
        LineKind::Code,
    ),
    ("", LineKind::Plain),
    (
        "Or leave it — it won't interfere with SSH repos.",
        LineKind::Dim,
    ),
    ("", LineKind::Plain),
    ("Note on repos outside your gitez folder", LineKind::Heading),
    ("", LineKind::Plain),
    (
        "gitez applies identity via includeIf based on the folder you configured. \
         Repos cloned outside that folder won't pick up the account automatically — \
         move them inside the folder, or set the remote URL and run:",
        LineKind::Plain,
    ),
    ("", LineKind::Plain),
    (
        "  git config user.email you@example.com",
        LineKind::Code,
    ),
    (
        "  git config user.name  Your Name",
        LineKind::Code,
    ),
    ("", LineKind::Plain),
    ("inside those repos to set identity manually.", LineKind::Plain),
];

#[derive(Clone, Copy)]
enum LineKind {
    Heading,
    Plain,
    Dim,
    Code,
}

pub struct MigrateState {
    pub scroll: u16,
    max_scroll: u16,
}

impl MigrateState {
    pub fn new() -> Self {
        Self { scroll: 0, max_scroll: LINES.len().saturating_sub(1) as u16 }
    }
}

pub fn draw(f: &mut Frame, state: &MigrateState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(3)])
        .split(area);

    draw_body(f, chunks[0], state);
    draw_footer(f, chunks[1]);
}

fn draw_body(f: &mut Frame, area: Rect, state: &MigrateState) {
    let lines: Vec<Line> = LINES
        .iter()
        .map(|(text, kind)| match kind {
            LineKind::Heading => Line::from(Span::styled(
                *text,
                ui::accent().add_modifier(Modifier::BOLD),
            )),
            LineKind::Plain => Line::from(Span::raw(*text)),
            LineKind::Dim => Line::from(Span::styled(*text, ui::dim())),
            LineKind::Code => Line::from(Span::styled(
                *text,
                ui::accent(),
            )),
        })
        .collect();

    let p = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" migrate HTTPS → SSH "),
        )
        .wrap(Wrap { trim: false })
        .scroll((state.scroll, 0));
    f.render_widget(p, area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let p = ratatui::widgets::Paragraph::new(Span::styled(
        " ↑/↓ or j/k to scroll   Esc to go back ",
        ui::dim(),
    ))
    .block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

/// Returns true when the user wants to leave this screen.
pub fn handle_key(state: &mut MigrateState, code: KeyCode) -> bool {
    match code {
        KeyCode::Esc | KeyCode::Char('q') => return true,
        KeyCode::Down | KeyCode::Char('j') => {
            if state.scroll < state.max_scroll {
                state.scroll += 1;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.scroll = state.scroll.saturating_sub(1);
        }
        _ => {}
    }
    false
}
