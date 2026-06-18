//! Manage `~/.gitconfig` conditional includes and per-account
//! `~/.gitconfig-<alias>` files.
//!
//! In the global gitconfig we add a managed block like:
//!
//! ```text
//! # >>> gitez: work
//! [includeIf "gitdir:~/work/"]
//!     path = ~/.gitconfig-work
//! # <<< gitez: work
//! ```
//!
//! The included file contains the account's name + email and points
//! Git at the right SSH host alias for fetch/push:
//!
//! ```text
//! [user]
//!     name = Your Name
//!     email = you@example.com
//! [core]
//!     sshCommand = ssh -i ~/.ssh/id_ed25519_work -F ~/.ssh/config
//! [url "git@github.com-work:"]
//!     insteadOf = git@github.com:
//! ```
//!
//! The `insteadOf` rewrite means even repos cloned with the plain
//! `git@github.com:...` URL get routed through the correct SSH host
//! when they live under the account's folder.

use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Result};

use crate::account::Account;
use crate::ssh::config::display_path;

const BEGIN_PREFIX: &str = "# >>> gitez:";
const END_PREFIX: &str = "# <<< gitez:";

pub fn gitconfig_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| eyre!("Could not locate home directory"))?;
    Ok(home.join(".gitconfig"))
}

pub fn per_account_path(alias: &str) -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| eyre!("Could not locate home directory"))?;
    Ok(home.join(format!(".gitconfig-{alias}")))
}

fn read_gitconfig() -> Result<String> {
    let path = gitconfig_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(fs::read_to_string(path)?)
}

fn write_gitconfig(contents: &str) -> Result<()> {
    let path = gitconfig_path()?;
    fs::write(path, contents)?;
    Ok(())
}

/// Write the per-account gitconfig file and add the includeIf block to
/// the main gitconfig.
pub fn upsert_account(account: &Account) -> Result<()> {
    write_per_account_file(account)?;

    for folder in &account.folders {
        fs::create_dir_all(folder)?;
    }

    let mut main = read_gitconfig()?;
    main = remove_block(&main, &account.alias);

    let include_block = render_include_block(account);
    if !main.is_empty() && !main.ends_with('\n') {
        main.push('\n');
    }
    if !main.is_empty() {
        main.push('\n');
    }
    main.push_str(&include_block);
    write_gitconfig(&main)?;
    Ok(())
}

/// Remove the per-account file and the includeIf block.
pub fn remove_account(alias: &str) -> Result<()> {
    let main = read_gitconfig()?;
    let updated = remove_block(&main, alias);
    if updated != main {
        write_gitconfig(&updated)?;
    }
    let per_path = per_account_path(alias)?;
    if per_path.exists() {
        fs::remove_file(per_path)?;
    }
    Ok(())
}

fn write_per_account_file(account: &Account) -> Result<()> {
    let path = per_account_path(&account.alias)?;
    let key_display = display_path(&account.key_path);
    let contents = format!(
        "[user]\n    \
         name = {name}\n    \
         email = {email}\n\
         [core]\n    \
         sshCommand = ssh -i {key} -F ~/.ssh/config\n\
         [url \"git@{host}:\"]\n    \
         insteadOf = git@github.com:\n",
        name = account.name,
        email = account.email,
        key = key_display,
        host = account.host_alias(),
    );
    fs::write(path, contents)?;
    Ok(())
}

fn render_include_block(account: &Account) -> String {
    let include_target = format!("~/.gitconfig-{}", account.alias);
    let mut block = format!("{BEGIN_PREFIX} {}\n", account.alias);
    for folder in &account.folders {
        let folder_display = display_folder(folder);
        block.push_str(&format!(
            "[includeIf \"gitdir:{folder_display}\"]\n    path = {include_target}\n",
        ));
    }
    block.push_str(&format!("{END_PREFIX} {}\n", account.alias));
    block
}

/// Format a folder path for inclusion in a gitdir matcher: forward
/// slashes, trailing slash, `~` if under home.
fn display_folder(folder: &Path) -> String {
    let mut s = display_path(folder);
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn remove_block(contents: &str, alias: &str) -> String {
    let begin = format!("{BEGIN_PREFIX} {alias}");
    let end = format!("{END_PREFIX} {alias}");
    let mut out = String::new();
    let mut skipping = false;
    for line in contents.lines() {
        if !skipping && line.trim_start().starts_with(&begin) {
            skipping = true;
            continue;
        }
        if skipping && line.trim_start().starts_with(&end) {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}
