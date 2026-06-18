//! Home screen — main menu.

use crossterm::event::KeyCode;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState as RListState, Paragraph, Wrap},
    Frame,
};

use crate::app::Screen;
use crate::ui::{self, add::AddState, list::ListState, migrate::MigrateState, test::TestState};

const ITEMS: &[(&str, &str)] = &[
    ("Add account", "Create a new SSH key and configure a GitHub identity"),
    ("List accounts", "Show all gitez-managed accounts"),
    ("Remove account", "Delete an account's keys and config"),
    ("Test connection", "Verify SSH auth to GitHub for an account"),
    ("Migrate HTTPS → SSH", "Guide to switching existing repos from HTTPS to SSH"),
    ("Quit", "Exit gitez"),
];

pub struct HomeState {
    pub cursor: usize,
    list_state: RListState,
}

impl HomeState {
    pub fn new() -> Self {
        let mut list_state = RListState::default();
        list_state.select(Some(0));
        Self { cursor: 0, list_state }
    }
}

pub fn draw(f: &mut Frame, state: &mut HomeState, flash: Option<&str>) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),  // banner
            Constraint::Min(8),     // menu
            Constraint::Length(3),  // flash / hint
        ])
        .split(area);

    draw_banner(f, chunks[0]);
    draw_menu(f, chunks[1], state);
    draw_footer(f, chunks[2], flash);
}

fn draw_banner(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "  gitez ",
            Style::default().add_modifier(Modifier::BOLD).fg(ratatui::style::Color::Cyan),
        )),
        Line::from(Span::styled(
            "  Multi-account GitHub setup, made easy.",
            ui::dim(),
        )),
    ];
    let p = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" welcome "));
    f.render_widget(p, area);
}

fn draw_menu(f: &mut Frame, area: Rect, state: &mut HomeState) {
    let items: Vec<ListItem> = ITEMS
        .iter()
        .map(|(title, desc)| {
            let line = Line::from(vec![
                Span::styled(format!(" {title}  "), Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(*desc, ui::dim()),
            ]);
            ListItem::new(line)
        })
        .collect();

    state.list_state.select(Some(state.cursor));
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" menu "))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::REVERSED)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");
    f.render_stateful_widget(list, area, &mut state.list_state);
}

fn draw_footer(f: &mut Frame, area: Rect, flash: Option<&str>) {
    let text = if let Some(msg) = flash {
        Line::from(Span::styled(format!(" {msg} "), ui::ok()))
    } else {
        Line::from(Span::styled(
            " ↑/↓ or j/k to move   Enter to select   q or Esc to quit ",
            ui::dim(),
        ))
    };
    let p = Paragraph::new(text)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

/// Returns the new screen if the user picked something, else None.
pub fn handle_key(state: &mut HomeState, code: KeyCode) -> Option<Screen> {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => return Some(Screen::Quit),
        KeyCode::Down | KeyCode::Char('j') => {
            if state.cursor + 1 < ITEMS.len() {
                state.cursor += 1;
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if state.cursor > 0 {
                state.cursor -= 1;
            }
        }
        KeyCode::Enter => {
            return match state.cursor {
                0 => Some(Screen::Add(AddState::new())),
                1 => Some(Screen::List(ListState::new())),
                2 => Some(Screen::Remove(ListState::new())),
                3 => Some(Screen::Test(TestState::new())),
                4 => Some(Screen::Migrate(MigrateState::new())),
                5 => Some(Screen::Quit),
                _ => None,
            };
        }
        _ => {}
    }
    None
}
