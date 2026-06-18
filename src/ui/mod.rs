pub mod add;
pub mod home;
pub mod list;
pub mod migrate;
pub mod test;

use ratatui::style::{Color, Style};

pub fn accent() -> Style {
    Style::default().fg(Color::Cyan)
}

pub fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn ok() -> Style {
    Style::default().fg(Color::Green)
}

pub fn err() -> Style {
    Style::default().fg(Color::Red)
}
