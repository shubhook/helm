//! The ids of the cursor commands, which
//! [`CommandRegistry::with_all_commands`] registers next to the
//! [`editor_commands`](crate::editor_commands). Keymaps (T-107, T-108)
//! bind these.
//!
//! Each move and select command runs [`Editor::move_cursors`] or
//! [`Editor::extend_selections`] with the [`Motion`] its name says, and
//! each delete command runs [`Editor::delete_to`]. They live apart from
//! `editor_commands` because T-106's tests pin that module's list.
//!
//! [`CommandRegistry::with_all_commands`]: crate::CommandRegistry::with_all_commands
//! [`Editor::move_cursors`]: crate::Editor::move_cursors
//! [`Editor::extend_selections`]: crate::Editor::extend_selections
//! [`Editor::delete_to`]: crate::Editor::delete_to
//! [`Motion`]: crate::Motion

#![expect(
    dead_code,
    reason = "T-107 interface stubs; the implementation deletes this"
)]

use crate::CommandRegistry;

/// `move_cursors(Motion::Left)`.
pub const LEFT: &str = "cursor.left";
/// `move_cursors(Motion::Right)`.
pub const RIGHT: &str = "cursor.right";
/// `move_cursors(Motion::Up)`.
pub const UP: &str = "cursor.up";
/// `move_cursors(Motion::Down)`.
pub const DOWN: &str = "cursor.down";
/// `move_cursors(Motion::WordLeft)`.
pub const WORD_LEFT: &str = "cursor.word_left";
/// `move_cursors(Motion::WordRight)`.
pub const WORD_RIGHT: &str = "cursor.word_right";
/// `move_cursors(Motion::LineStart)`.
pub const LINE_START: &str = "cursor.line_start";
/// `move_cursors(Motion::LineEnd)`.
pub const LINE_END: &str = "cursor.line_end";
/// `move_cursors(Motion::TextStart)`.
pub const TEXT_START: &str = "cursor.text_start";
/// `move_cursors(Motion::TextEnd)`.
pub const TEXT_END: &str = "cursor.text_end";

/// `extend_selections(Motion::Left)`.
pub const SELECT_LEFT: &str = "cursor.select_left";
/// `extend_selections(Motion::Right)`.
pub const SELECT_RIGHT: &str = "cursor.select_right";
/// `extend_selections(Motion::Up)`.
pub const SELECT_UP: &str = "cursor.select_up";
/// `extend_selections(Motion::Down)`.
pub const SELECT_DOWN: &str = "cursor.select_down";
/// `extend_selections(Motion::WordLeft)`.
pub const SELECT_WORD_LEFT: &str = "cursor.select_word_left";
/// `extend_selections(Motion::WordRight)`.
pub const SELECT_WORD_RIGHT: &str = "cursor.select_word_right";
/// `extend_selections(Motion::LineStart)`.
pub const SELECT_LINE_START: &str = "cursor.select_line_start";
/// `extend_selections(Motion::LineEnd)`.
pub const SELECT_LINE_END: &str = "cursor.select_line_end";
/// `extend_selections(Motion::TextStart)`.
pub const SELECT_TEXT_START: &str = "cursor.select_text_start";
/// `extend_selections(Motion::TextEnd)`.
pub const SELECT_TEXT_END: &str = "cursor.select_text_end";

/// [`Editor::select_all`](crate::Editor::select_all).
pub const SELECT_ALL: &str = "cursor.select_all";
/// [`Editor::collapse_selections`](crate::Editor::collapse_selections).
pub const COLLAPSE: &str = "cursor.collapse";

/// `delete_to(Motion::WordLeft)`.
pub const DELETE_WORD_LEFT: &str = "cursor.delete_word_left";
/// `delete_to(Motion::WordRight)`.
pub const DELETE_WORD_RIGHT: &str = "cursor.delete_word_right";
/// `delete_to(Motion::LineStart)`.
pub const DELETE_LINE_START: &str = "cursor.delete_line_start";
/// `delete_to(Motion::LineEnd)`.
pub const DELETE_LINE_END: &str = "cursor.delete_line_end";

/// Every id above, sorted.
pub const ALL: [&str; 26] = [
    COLLAPSE,
    DELETE_LINE_END,
    DELETE_LINE_START,
    DELETE_WORD_LEFT,
    DELETE_WORD_RIGHT,
    DOWN,
    LEFT,
    LINE_END,
    LINE_START,
    RIGHT,
    SELECT_ALL,
    SELECT_DOWN,
    SELECT_LEFT,
    SELECT_LINE_END,
    SELECT_LINE_START,
    SELECT_RIGHT,
    SELECT_TEXT_END,
    SELECT_TEXT_START,
    SELECT_UP,
    SELECT_WORD_LEFT,
    SELECT_WORD_RIGHT,
    TEXT_END,
    TEXT_START,
    UP,
    WORD_LEFT,
    WORD_RIGHT,
];

/// Registers one command for each id in [`ALL`], each with a non-empty
/// title.
pub(crate) fn register_all(registry: &mut CommandRegistry) {
    let _ = registry;
    todo!()
}
