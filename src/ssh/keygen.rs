//! Generate an ed25519 SSH keypair and write it to disk.

use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Result};
use ssh_key::{Algorithm, LineEnding, PrivateKey};

/// Generate an ed25519 keypair and write it to `private_path` and
/// `<private_path>.pub`. The comment is embedded in the public key
/// (conventionally the user's email).
pub fn generate_ed25519(private_path: &Path, comment: &str) -> Result<()> {
    if private_path.exists() {
        return Err(eyre!(
            "Key already exists at {} — choose a different alias or remove the old key first",
            private_path.display()
        ));
    }

    if let Some(parent) = private_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut rng = rand::rngs::OsRng;
    let mut private_key = PrivateKey::random(&mut rng, Algorithm::Ed25519)
        .map_err(|e| eyre!("failed to generate key: {e}"))?;
    private_key.set_comment(comment);

    // Write the private key in OpenSSH PEM format.
    let pem = private_key
        .to_openssh(LineEnding::LF)
        .map_err(|e| eyre!("failed to serialize private key: {e}"))?;
    fs::write(private_path, pem.as_bytes())?;

    // Restrict permissions: owner-only read/write.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(private_path)?.permissions();
        perms.set_mode(0o600);
        fs::set_permissions(private_path, perms)?;
    }
    #[cfg(windows)]
    restrict_key_permissions_windows(private_path)?;

    // Public key alongside the private one.
    let pub_path = pub_path_for(private_path);
    let public_openssh = private_key
        .public_key()
        .to_openssh()
        .map_err(|e| eyre!("failed to serialize public key: {e}"))?;
    fs::write(&pub_path, format!("{public_openssh}\n"))?;

    Ok(())
}

/// Remove inherited ACEs and grant full control only to the current user.
/// This makes Git Bash's OpenSSH accept the key (it rejects world-readable keys).
#[cfg(windows)]
fn restrict_key_permissions_windows(path: &Path) -> Result<()> {
    use std::process::Command;
    let p = path.to_string_lossy();
    // Remove inherited permissions, then grant only the current user.
    let out = Command::new("icacls")
        .args([p.as_ref(), "/inheritance:r", "/grant:r"])
        .arg(format!(
            "{}:F",
            std::env::var("USERNAME").unwrap_or_else(|_| "User".into())
        ))
        .output()
        .map_err(|e| eyre!("failed to run icacls: {e}"))?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        return Err(eyre!("icacls failed: {msg}"));
    }
    Ok(())
}

fn pub_path_for(private_path: &Path) -> PathBuf {
    let mut p = private_path.to_path_buf();
    let new = format!("{}.pub", p.file_name().unwrap().to_string_lossy());
    p.set_file_name(new);
    p
}

/// Read a public key file as a string (newline-trimmed).
pub fn read_public_key(pub_path: &Path) -> Result<String> {
    let s = fs::read_to_string(pub_path)?;
    Ok(s.trim().to_string())
}
