//! VS Code's default keymap on macOS (ED-4), for every command
//! [`CommandRegistry::with_all_commands`] registers.
//!
//! [`CommandRegistry::with_all_commands`]: crate::CommandRegistry::with_all_commands

use crate::Keymap;

/// The keymap's name.
pub const NAME: &str = "vscode";

/// The keymap's one mode. Its typing is
/// [`Typing::Insert`](crate::Typing::Insert), so a key no binding matches
/// types its text.
pub const MODE: &str = "default";

/// VS Code's default keymap on macOS: one mode, [`MODE`], and these
/// bindings, each [`Action::Run`](crate::Action::Run) with no mode, no
/// context and no mode switch. Keys are in
/// [`KeySequence`](crate::KeySequence) notation.
///
/// | Keys | Command |
/// | --- | --- |
/// | `enter` | `editor.newline` |
/// | `backspace` | `editor.delete_backward` |
/// | `delete` | `editor.delete_forward` |
/// | `cmd-z` | `editor.undo` |
/// | `shift-cmd-z` | `editor.redo` |
/// | `cmd-d` | `editor.add_next_match` |
/// | `shift-cmd-l` | `editor.select_all_matches` |
/// | `alt-cmd-up` | `editor.add_cursor_above` |
/// | `alt-cmd-down` | `editor.add_cursor_below` |
/// | `left`, `right`, `up`, `down` | `cursor.left`, `cursor.right`, `cursor.up`, `cursor.down` |
/// | `alt-left`, `alt-right` | `cursor.word_left`, `cursor.word_right` |
/// | `home`, `cmd-left` | `cursor.line_start` |
/// | `end`, `cmd-right` | `cursor.line_end` |
/// | `cmd-up`, `cmd-down` | `cursor.text_start`, `cursor.text_end` |
/// | `shift-` with each of the twelve keys in the five rows above | the `cursor.select_` command of the same motion |
/// | `cmd-a` | `cursor.select_all` |
/// | `escape` | `cursor.collapse` |
/// | `alt-backspace`, `alt-delete` | `cursor.delete_word_left`, `cursor.delete_word_right` |
/// | `cmd-backspace`, `cmd-delete` | `cursor.delete_line_start`, `cursor.delete_line_end` |
///
/// That is 39 bindings. None of them is a chord, and none uses a char
/// that Shift makes, so the notation's folding of Shift into chars changes
/// nothing here.
pub fn keymap() -> Keymap {
    todo!()
}
