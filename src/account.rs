//! The Account type — one configured GitHub identity.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Account {
    /// Short alias used in SSH config Host (e.g. "work" → github.com-work).
    pub alias: String,
    /// Git author name.
    pub name: String,
    /// Git author email.
    pub email: String,
    /// Folders on disk where repos for this account live (one `includeIf`
    /// block per folder is written to `~/.gitconfig`).
    pub folders: Vec<PathBuf>,
    /// Path to the private key (e.g. ~/.ssh/id_ed25519_work).
    pub key_path: PathBuf,
}

impl Account {
    pub fn pub_key_path(&self) -> PathBuf {
        let mut p = self.key_path.clone();
        let new = format!("{}.pub", p.file_name().unwrap().to_string_lossy());
        p.set_file_name(new);
        p
    }

    pub fn host_alias(&self) -> String {
        format!("github.com-{}", self.alias)
    }
}
