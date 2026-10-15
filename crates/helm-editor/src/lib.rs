//! Editor logic: cursors, multi-cursor, selections, commands and the keymap with VS Code and Vim modes.
//!
//! # Selections
//!
//! A [`Selection`] is an anchor and a head, both byte offsets into a
//! buffer's text. The head is where the cursor is drawn and the end that
//! moves when a selection grows; the anchor stays put. A cursor is a
//! selection whose anchor equals its head. A selection can point either
//! way: `head < anchor` is a selection made leftwards.
//!
//! [`Selections`] holds one or more selections for one editor, always kept
//! in a normal form:
//!
//! - Sorted by start. No two selections overlap.
//! - Two selections merge when they overlap, or when they touch and at
//!   least one of them is empty. So two cursors at one offset become one
//!   cursor, and a cursor at either end of a selection joins it. Two
//!   non-empty selections that only touch, such as `0..1` and `1..2`, stay
//!   apart, so selecting every `a` in `"aaa"` gives three selections.
//! - One selection is the primary: the one the next "add" command starts
//!   from, and the one a later scroll-into-view follows.
//!
//! # Editing at every cursor
//!
//! An [`Editor`] edits one buffer of a [`BufferSet`](helm_text::BufferSet).
//! Each editing command builds one edit per selection and applies them all
//! as one [`Transaction`](helm_text::Transaction), so a keystroke at a
//! thousand cursors is one undo step. Every edit's range refers to the text
//! before the command, as `BufferSet::apply` requires. [`Editor`] documents
//! where each selection lands afterwards.
//!
//! A new line is [`FileFormat::newline`](helm_text::FileFormat::newline) of
//! the editor's format, so a CRLF file gets `"\r\n"`. The editor never
//! writes a hard-coded `"\n"`.
//!
//! # Undo and redo
//!
//! [`Editor::undo`] and [`Editor::redo`] go through the buffer set, then put
//! back the selections the editor had just before or just after the command
//! that made the transaction. Commands that only move or add selections are
//! not transactions and are not undone.

// T-105 stubs. Delete this attribute once every `todo!()` is filled in.
#![expect(
    unused_variables,
    dead_code,
    reason = "T-105 interface stubs; the implementation deletes this"
)]

mod editor;
mod error;
mod selection;

pub use editor::Editor;
pub use error::EditorError;
pub use selection::{Selection, Selections};
