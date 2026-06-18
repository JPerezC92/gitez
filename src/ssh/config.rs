//! Read and write `~/.ssh/config` entries that gitez manages.
//!
//! gitez wraps each managed host block in marker comments so it can
//! safely remove or update them without touching anything the user
//! added by hand:
//!
//! ```text
//! # >>> gitez: work
//! Host github.com-work
//!     HostName github.com
//!     User git
//!     IdentityFile ~/.ssh/id_ed25519_work
//!     IdentitiesOnly yes
//! # <<< gitez: work
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Result};

use crate::account::Account;

const BEGIN_PREFIX: &str = "# >>> gitez:";
const END_PREFIX: &str = "# <<< gitez:";

pub fn ssh_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| eyre!("Could not locate home directory"))?;
    Ok(home.join(".ssh").join("config"))
}

pub fn ssh_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| eyre!("Could not locate home directory"))?;
    Ok(home.join(".ssh"))
}

/// Read the SSH config, returning an empty string if it doesn't exist yet.
fn read_existing() -> Result<String> {
    let path = ssh_config_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    Ok(fs::read_to_string(path)?)
}

fn write_config(contents: &str) -> Result<()> {
    let path = ssh_config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, contents)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(&path, perms)?;
    }
    Ok(())
}

/// Append (or replace) a managed block for the given account.
pub fn upsert_account(account: &Account) -> Result<()> {
    let mut existing = read_existing()?;
    // Remove any previous block with this alias first.
    existing = remove_block(&existing, &account.alias);

    let block = render_block(account);

    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    if !existing.is_empty() {
        existing.push('\n');
    }
    existing.push_str(&block);

    write_config(&existing)?;
    Ok(())
}

/// Remove the managed block for `alias`. No-op if not present.
pub fn remove_account(alias: &str) -> Result<()> {
    let existing = read_existing()?;
    let updated = remove_block(&existing, alias);
    if updated != existing {
        write_config(&updated)?;
    }
    Ok(())
}

/// List all aliases gitez currently manages.
pub fn list_managed_aliases() -> Result<Vec<String>> {
    let existing = read_existing()?;
    let mut aliases = Vec::new();
    for line in existing.lines() {
        if let Some(rest) = line.strip_prefix(BEGIN_PREFIX) {
            aliases.push(rest.trim().to_string());
        }
    }
    Ok(aliases)
}

/// Read the IdentityFile path for a managed alias, if present.
pub fn key_path_for(alias: &str) -> Result<Option<PathBuf>> {
    let existing = read_existing()?;
    let begin = format!("{BEGIN_PREFIX} {alias}");
    let end = format!("{END_PREFIX} {alias}");
    let mut in_block = false;
    for line in existing.lines() {
        if line.trim_start().starts_with(&begin) {
            in_block = true;
            continue;
        }
        if line.trim_start().starts_with(&end) {
            in_block = false;
            continue;
        }
        if in_block {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("IdentityFile") {
                let path = rest.trim().trim_matches('"').to_string();
                return Ok(Some(expand_tilde(&path)));
            }
        }
    }
    Ok(None)
}

fn expand_tilde(p: &str) -> PathBuf {
    if let Some(stripped) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    }
    PathBuf::from(p)
}

fn render_block(account: &Account) -> String {
    let key_display = display_path(&account.key_path);
    format!(
        "{BEGIN_PREFIX} {alias}\n\
         Host {host}\n    \
         HostName github.com\n    \
         User git\n    \
         IdentityFile {key}\n    \
         IdentitiesOnly yes\n\
         {END_PREFIX} {alias}\n",
        alias = account.alias,
        host = account.host_alias(),
        key = key_display,
    )
}

/// Render a path with `~` if it lives under the user's home dir.
pub fn display_path(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = path.strip_prefix(&home) {
            // Always use forward slashes inside ssh_config.
            let rel = rel.to_string_lossy().replace('\\', "/");
            return format!("~/{rel}");
        }
    }
    path.to_string_lossy().replace('\\', "/")
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
    // Collapse trailing blank lines.
    while out.ends_with("\n\n") {
        out.pop();
    }
    out
}
