# User story — add-wizard-input-editing

> **Created:** 2026-08-27
> **Title:** Add-wizard text inputs with full cursor editing
> **Status:** active
> **Epic:**
> **Affected areas:** `src/ui/add.rs`, `src/app.rs`

## Persona

- A developer adding a GitHub account to gitez who notices a typo mid-email and expects to fix it in place, like in any standard editor, without retyping the whole field.

## Goal

- **G:** The four Add-wizard inputs (Alias, Name, Email, Folder) behave like standard single-line editors
  - Done when: the cursor can be placed anywhere in already-typed text, edits apply at the cursor, word jumps and selection work, and the wizard's navigation keys (Enter, Esc) and all other screens' bindings behave exactly as before

## Scenario

- The user types `usernamw@job.com` on the Email step, presses Ctrl+Left twice and Left once to reach the typo, Backspace, then `e` — the field now reads `username@job.com` with nothing retyped. Enter then advances to the Folder step exactly as before.

## Acceptance criteria

- ✅ Cursor movement within typed text works on all 4 inputs: Left/Right, Home/End, Ctrl/Ctrl+Shift+arrow and Alt+b/Alt+f word jump (plan G1)
- ✅ Edits apply at the cursor; Shift+arrows select and the next edit replaces the selection (plan G2)
- ✅ Wizard Enter/Esc flow and every other screen's bindings are unchanged; no new binding collides with window navigation (plan G3)
- ✅ Ctrl/Alt modifier chords never insert characters into the inputs (plan G4)

## Change log

- 2026-08-27 — wizard-input-editing-20260827: created story; swaps the wizard's append-only String inputs for single-line tui-textarea inputs with cursor, word-jump, and selection editing while preserving all navigation behavior.

## Resolved decisions

- 2026-08-27 — any text-editing binding that collides with window-navigation commands must be dropped; collision analysis found zero real collisions, and the kitty-protocol terminal-flags change was dropped as global risk without need.
