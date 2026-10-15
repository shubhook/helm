//! `Editor`: selections on one buffer, and the commands that edit at every
//! cursor or add selections.

use std::collections::HashMap;

use helm_text::{BufferId, BufferSet, FileFormat, TransactionId};

use crate::{EditorError, Selection, Selections};

/// The selections on one buffer of a [`BufferSet`], and the commands that
/// edit at every selection or add new ones.
///
/// The editor holds the buffer's id, not the buffer. Every command takes
/// the set that owns it, and errors with [`EditorError::UnknownBuffer`] if
/// the buffer isn't there. Passing a different set than the one that made
/// the id is a logic error: the command may edit the wrong buffer, but it
/// never panics.
///
/// # Clamping
///
/// Every command that reads the selections first clamps each offset to the
/// buffer's current text: an offset past the end moves to the end, and an
/// offset inside a character moves back to that character's first byte. The
/// position between the CR and LF of a CRLF is a valid offset and stays.
/// The result is put back in normal form, then the command runs. Stale
/// selections, from [`set_selections`](Self::set_selections) or from an
/// edit made outside this editor, therefore never cause an error or a
/// panic. Clamping reads only the offsets it checks; it never copies the
/// text.
///
/// Whenever a command puts selections back in normal form, it gives
/// [`Selections`] the existing selections first, in order, then any new
/// ones, so a merge keeps the direction `Selections` documents.
///
/// # Editing commands
///
/// [`insert`](Self::insert), [`newline`](Self::newline),
/// [`delete_backward`](Self::delete_backward) and
/// [`delete_forward`](Self::delete_forward) build one edit per selection,
/// apply them as one transaction, and return its id. Ranges refer to the
/// text before the command. After the transaction, every selection is a
/// cursor, its offset moved by the bytes added and removed before it, and
/// the selections are put back in normal form. The primary is the cursor
/// the old primary became.
///
/// A command with nothing to do (deleting backward with every cursor at the
/// start of the text, say) applies no transaction, returns `Ok(None)`, and
/// leaves the clamped selections as they are.
///
/// # Undo and redo
///
/// The editor remembers, for each transaction its own commands applied, the
/// selections just before the command (after clamping) and just after it.
/// [`undo`](Self::undo) puts back the "before" selections of the
/// transaction it undid, and [`redo`](Self::redo) the "after" ones,
/// including which one is primary. A transaction the editor didn't apply,
/// such as an agent's edit, leaves the selections as they are; the next
/// command clamps them.
///
/// # Matches and words
///
/// [`add_next_match`](Self::add_next_match) and
/// [`select_all_matches`](Self::select_all_matches) search for the primary
/// selection's text: an exact, case-sensitive match of its bytes. If the
/// primary is a cursor, they first select the word around it. A word is a
/// run of word characters (`char::is_alphanumeric`, or `_`). The word
/// around a cursor extends left from it while the character before is a
/// word character, and right while the character after is one. If both
/// neighbours aren't word characters, there is no word.
#[derive(Clone, Debug)]
pub struct Editor {
    buffer: BufferId,
    format: FileFormat,
    selections: Selections,
    /// Selections before and after each transaction this editor applied.
    /// A placeholder: the implementation may replace it.
    history: HashMap<TransactionId, (Selections, Selections)>,
}

impl Editor {
    /// An editor on buffer `buffer` with one cursor at offset 0. `format`
    /// is the file's format; its [`FileFormat::newline`] is the line break
    /// [`newline`](Self::newline) inserts.
    pub fn new(buffer: BufferId, format: FileFormat) -> Self {
        Self {
            buffer,
            format,
            selections: Selections::single(Selection::cursor(0)),
            history: HashMap::new(),
        }
    }

    /// The buffer this editor edits.
    pub fn buffer(&self) -> BufferId {
        self.buffer
    }

    /// The file format this editor was made with.
    pub fn format(&self) -> FileFormat {
        self.format
    }

    /// The current selections.
    pub fn selections(&self) -> &Selections {
        &self.selections
    }

    /// Replaces the selections. They are not checked against the buffer
    /// here; the next command clamps them.
    pub fn set_selections(&mut self, selections: Selections) {
        self.selections = selections;
    }

    /// Types `text` at every selection: each selection's range is replaced
    /// by `text`, and each becomes a cursor right after its copy of `text`.
    /// `text` goes in exactly as given; line breaks in it are not converted.
    ///
    /// Returns `Ok(None)` when `text` is empty and every selection is a
    /// cursor.
    pub fn insert(
        &mut self,
        set: &mut BufferSet,
        text: &str,
    ) -> Result<Option<TransactionId>, EditorError> {
        todo!("T-105: replace every selection with `text` in one transaction")
    }

    /// Inserts a line break at every selection, the same way
    /// [`insert`](Self::insert) does. The line break is
    /// [`FileFormat::newline`] of this editor's format. No indentation is
    /// added.
    pub fn newline(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError> {
        todo!("T-105: insert the format's newline at every selection")
    }

    /// Backspace at every selection. A non-empty selection deletes its
    /// range. A cursor deletes the character before it, or both bytes of a
    /// CRLF if it sits right after one; at offset 0 it deletes nothing. A
    /// character is one Unicode scalar value, so 😀 goes in one step.
    ///
    /// Deleted ranges that overlap (two cursors on either side of the LF of
    /// a CRLF) are joined into one before the transaction is built. Each
    /// selection that deleted something becomes a cursor where its range
    /// started; the rest stay where they were, moved by the edits before
    /// them.
    pub fn delete_backward(
        &mut self,
        set: &mut BufferSet,
    ) -> Result<Option<TransactionId>, EditorError> {
        todo!("T-105: delete before every cursor in one transaction")
    }

    /// Delete at every selection. Like
    /// [`delete_backward`](Self::delete_backward), except a cursor deletes
    /// the character after it, or both bytes of a CRLF if one starts at the
    /// cursor, and at the end of the text deletes nothing.
    pub fn delete_forward(
        &mut self,
        set: &mut BufferSet,
    ) -> Result<Option<TransactionId>, EditorError> {
        todo!("T-105: delete after every cursor in one transaction")
    }

    /// Adds a selection on the next match of the primary's text (ED-3, "add
    /// next match"). Returns whether the selections changed.
    ///
    /// If the primary is a cursor, it becomes the word around it, and
    /// nothing else happens. With no word there, nothing changes.
    ///
    /// Otherwise it looks at every place the text occurs, including
    /// occurrences that overlap each other, first those that start at or
    /// after the primary's end, in order, then, wrapping around, those that
    /// start before it, from the start of the text. The first occurrence
    /// that doesn't overlap an existing selection is added, pointing
    /// forward, and becomes the primary. If every occurrence overlaps a
    /// selection, nothing changes.
    pub fn add_next_match(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        todo!("T-105: add the next match of the primary's text")
    }

    /// Replaces the selections with every match of the primary's text
    /// (ED-3, "select all matches"). Returns whether the selections
    /// changed.
    ///
    /// If the primary is a cursor, its word is used, as in
    /// [`add_next_match`](Self::add_next_match). With no word there,
    /// nothing changes. Matches are found from the start of the text, each
    /// search continuing after the previous match, so they never overlap.
    /// Each points forward. The primary is the first match that overlaps the
    /// range searched for: the old primary, or the word around it.
    pub fn select_all_matches(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        todo!("T-105: select every match of the primary's text")
    }

    /// Adds a cursor on the line above every selection's head (ED-3, "add
    /// above"). Returns whether the selections changed.
    ///
    /// The column is the head's column in chars, capped at its line's
    /// length without the line break. The new cursor sits at that many
    /// chars into the line above, or at the end of that line's text, before
    /// its line break, if the line is shorter. A selection on the first line
    /// adds nothing. The cursor added for the old primary becomes the
    /// primary.
    pub fn add_cursor_above(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        todo!("T-105: add a cursor on the line above every head")
    }

    /// Adds a cursor on the line below every selection's head (ED-3, "add
    /// below"), the same way [`add_cursor_above`](Self::add_cursor_above)
    /// does. A selection on the last line adds nothing.
    pub fn add_cursor_below(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        todo!("T-105: add a cursor on the line below every head")
    }

    /// Undoes the transaction on top of this editor's buffer's undo history
    /// with [`BufferSet::undo`], and returns its id, or `Ok(None)` if there
    /// is nothing to undo. If this editor applied that transaction, the
    /// selections go back to what they were just before it.
    ///
    /// Errors with [`EditorError::Blocked`] when `BufferSet::undo` is
    /// blocked, and then nothing changes.
    pub fn undo(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError> {
        todo!("T-105: undo and restore the selections from before")
    }

    /// Redoes the transaction on top of this editor's buffer's redo history
    /// with [`BufferSet::redo`], and returns its id, or `Ok(None)` if there
    /// is nothing to redo. If this editor applied that transaction, the
    /// selections go back to what they were just after it.
    ///
    /// Errors the same way as [`undo`](Self::undo).
    pub fn redo(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError> {
        todo!("T-105: redo and restore the selections from after")
    }
}
