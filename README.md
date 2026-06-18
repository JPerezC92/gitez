# gitez

A friendly TUI tool to manage multiple GitHub accounts on one machine.
Generates SSH keys, configures `~/.ssh/config`, and sets up git's
conditional includes so the right identity is used automatically based
on the folder a repo lives in.

## Features

- Add a new GitHub account (generate ed25519 SSH key, configure SSH + git)
- List configured accounts
- Remove an account (cleans SSH config + git includes + keyfiles)
- Test the SSH connection to GitHub for a given account

## Build

```bash
cargo build --release
```

The binary will be at `target/release/gitez` (or `gitez.exe` on Windows).

## Usage

Just run it:

```bash
gitez
```

Use arrow keys (or `j`/`k`) to move, `Enter` to confirm, `Esc` or `q` to go
back / quit.

## How it works

When you add an account with alias `work`:

1. Generates `~/.ssh/id_ed25519_work` (+ `.pub`).
2. Adds a `Host github.com-work` block to `~/.ssh/config`.
3. Creates `~/.gitconfig-work` with your name/email + SSH key override.
4. Adds an `[includeIf "gitdir:..."]` block to `~/.gitconfig` pointing at
   the folder you chose (e.g. `~/work/`).
5. Shows you the public key and opens GitHub's SSH keys page so you can
   paste it.

Clone with:

```bash
git clone git@github.com-work:org/repo.git
```

Any repo under your work folder picks up the right identity automatically.
