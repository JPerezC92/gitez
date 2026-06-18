mod account;
mod app;
mod git;
mod ssh;
mod tui;
mod ui;

use color_eyre::Result;

use crate::app::App;

fn main() -> Result<()> {
    color_eyre::install()?;
    let mut terminal = tui::init()?;
    let result = App::new().run(&mut terminal);
    tui::restore()?;
    result
}
