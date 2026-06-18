//! Top-level App state and event loop.

use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::Frame;

use crate::tui::Tui;
use crate::ui::add::AddState;
use crate::ui::list::ListState;
use crate::ui::migrate::MigrateState;
use crate::ui::test::TestState;
use crate::ui::{add, home, list, migrate, test};

/// Which screen we're currently on.
pub enum Screen {
    Home(home::HomeState),
    Add(AddState),
    List(ListState),
    /// List but in "remove" mode — pressing Enter removes the selected account.
    Remove(ListState),
    Test(TestState),
    Migrate(MigrateState),
    Quit,
}

pub struct App {
    pub screen: Screen,
    /// A short message shown at the bottom of the home screen
    /// (e.g. "Account 'work' added successfully").
    pub flash: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Home(home::HomeState::new()),
            flash: None,
        }
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;

            if matches!(self.screen, Screen::Quit) {
                return Ok(());
            }

            // Poll so we could add tickers later; for now we just block briefly.
            if event::poll(Duration::from_millis(250))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code)?;
                    }
                }
            }
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        match &mut self.screen {
            Screen::Home(state) => home::draw(f, state, self.flash.as_deref()),
            Screen::Add(state) => add::draw(f, state),
            Screen::List(state) => list::draw(f, state, false),
            Screen::Remove(state) => list::draw(f, state, true),
            Screen::Test(state) => test::draw(f, state),
            Screen::Migrate(state) => migrate::draw(f, state),
            Screen::Quit => {}
        }
    }

    fn handle_key(&mut self, code: KeyCode) -> Result<()> {
        match &mut self.screen {
            Screen::Home(state) => {
                if let Some(next) = home::handle_key(state, code) {
                    self.flash = None;
                    self.screen = next;
                }
            }
            Screen::Add(state) => {
                if let Some(outcome) = add::handle_key(state, code)? {
                    self.flash = outcome.message;
                    self.screen = Screen::Home(home::HomeState::new());
                }
            }
            Screen::List(state) => {
                if list::handle_key_view(state, code) {
                    self.screen = Screen::Home(home::HomeState::new());
                }
            }
            Screen::Remove(state) => {
                match list::handle_key_remove(state, code)? {
                    list::RemoveOutcome::Stay => {}
                    list::RemoveOutcome::Done(msg) => {
                        self.flash = Some(msg);
                        self.screen = Screen::Home(home::HomeState::new());
                    }
                    list::RemoveOutcome::Back => {
                        self.screen = Screen::Home(home::HomeState::new());
                    }
                }
            }
            Screen::Test(state) => {
                if test::handle_key(state, code)? {
                    self.screen = Screen::Home(home::HomeState::new());
                }
            }
            Screen::Migrate(state) => {
                if migrate::handle_key(state, code) {
                    self.screen = Screen::Home(home::HomeState::new());
                }
            }
            Screen::Quit => {}
        }
        Ok(())
    }
}
