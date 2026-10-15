//! Motions: where a cursor goes from its head. [`Editor::move_cursors`],
//! [`Editor::extend_selections`] and [`Editor::delete_to`] take one, and
//! the cursor commands in [`cursor_commands`](crate::cursor_commands) are
//! one motion each.
//!
//! [`Editor::move_cursors`]: crate::Editor::move_cursors
//! [`Editor::extend_selections`]: crate::Editor::extend_selections
//! [`Editor::delete_to`]: crate::Editor::delete_to

/// Where a cursor moves from its head.
///
/// Every motion lands on a char boundary of the buffer's text, never
/// between the CR and LF of a CRLF. A line break is LF, CRLF or a lone CR,
/// as `helm-text` defines it, and CRLF always counts as one character.
/// Columns are in chars, not display columns: tab width and the width of
/// wide characters like 中 are `helm-ui`'s job.
///
/// # Word classes
///
/// [`WordLeft`](Self::WordLeft) and [`WordRight`](Self::WordRight) sort
/// each char into one of three classes, plus line breaks:
///
/// - **Word:** `char::is_alphanumeric`, or `_`. The same rule as
///   [`Editor::add_next_match`](crate::Editor::add_next_match).
/// - **Blank:** `char::is_whitespace` and not part of a line break, such
///   as a space or a tab.
/// - **Punctuation:** every other char, such as `.`, `(` or `😀`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// One character back: two bytes after a CRLF, one char otherwise. At
    /// offset 0 it stays.
    ///
    /// When [`Editor::move_cursors`](crate::Editor::move_cursors) moves a
    /// selection that isn't empty, the selection becomes a cursor at its
    /// start instead, the way VS Code collapses a selection on Left.
    Left,
    /// One character forward: two bytes before a CRLF, one char otherwise.
    /// At the end of the text it stays.
    ///
    /// When [`Editor::move_cursors`](crate::Editor::move_cursors) moves a
    /// selection that isn't empty, the selection becomes a cursor at its
    /// end instead.
    Right,
    /// To the line above, at the selection's goal column (see
    /// [`Editor`](crate::Editor), "Goal columns"), capped at that line's
    /// length without its line break. On the first line it goes to offset
    /// 0.
    Up,
    /// To the line below, the same way as [`Up`](Self::Up). On the last
    /// line it goes to the end of the text.
    Down,
    /// To the start of the word, or run of punctuation, before the head:
    ///
    /// 1. If a line break ends right before the head, move to before it,
    ///    and stop.
    /// 2. Skip back over blanks. If that reaches the start of a line, or
    ///    offset 0, stop there.
    /// 3. Skip back over the run of chars of the same class as the char
    ///    just before, word or punctuation, and stop.
    ///
    /// At offset 0 it stays.
    WordLeft,
    /// To the end of the word, or run of punctuation, after the head:
    ///
    /// 1. If a line break starts at the head, move to after it, and stop.
    /// 2. Skip over blanks. If that reaches a line break, or the end of the
    ///    text, stop there.
    /// 3. Skip over the run of chars of the same class as the char just
    ///    after, word or punctuation, and stop.
    ///
    /// At the end of the text it stays.
    WordRight,
    /// To the first char of the line that isn't a blank, or, if the head
    /// is already there, to the start of the line. A line of only blanks
    /// has its first non-blank position at the end of its text. This is
    /// VS Code's Home.
    LineStart,
    /// To the end of the line's text, before its line break.
    LineEnd,
    /// To offset 0.
    TextStart,
    /// To the end of the text.
    TextEnd,
}
