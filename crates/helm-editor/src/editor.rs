//! `Editor`: selections on one buffer, and the commands that edit at every
//! cursor or add selections.

use std::collections::HashMap;
use std::ops::Range;

use helm_text::{
    Buffer, BufferId, BufferSet, Edit, FileFormat, TextBuffer, Transaction, TransactionId,
};

use crate::{EditorError, Motion, Selection, Selections};

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
///
/// # Motions (T-107)
///
/// [`move_cursors`](Self::move_cursors),
/// [`extend_selections`](Self::extend_selections) and
/// [`delete_to`](Self::delete_to) move each selection's head by a
/// [`Motion`], which documents where each one goes. Moving reads only the
/// text near each head, and the lines it crosses for
/// [`Motion::Up`] and [`Motion::Down`]; it never copies the whole text.
///
/// # Goal columns
///
/// Moving up and down through a short line and back keeps the column, as
/// in every editor. Each selection has a goal column, in chars from the
/// start of its head's line. The first [`Motion::Up`] or [`Motion::Down`],
/// through `move_cursors` or `extend_selections`, sets it to the head's
/// column, capped at the line's length without its line break. Later Up
/// and Down moves aim for the goal, not the current column, and keep it.
///
/// Any other `&mut self` method drops every goal, including other motions,
/// editing commands, undo, redo and
/// [`set_selections`](Self::set_selections). If an Up or Down move merges
/// selections, the merged one keeps the goal of one of them; which one is
/// not specified.
#[derive(Clone, Debug)]
pub struct Editor {
    buffer: BufferId,
    format: FileFormat,
    selections: Selections,
    /// The selections around each transaction this editor applied that may
    /// still be undone or redone.
    history: HashMap<TransactionId, Record>,
    /// Transactions in `history` this editor undid and hasn't redone. They
    /// sit in the buffer's redo history, which the next transaction this
    /// editor applies drops, so their records go then too. That keeps
    /// `history` from growing with every undo and retype.
    undone: Vec<TransactionId>,
}

/// The selections just before and just after one transaction.
#[derive(Clone, Debug)]
struct Record {
    id: TransactionId,
    before: Selections,
    after: Selections,
}

/// Which way a command looks from each selection's head.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Before the head, or the line above.
    Backward,
    /// After the head, or the line below.
    Forward,
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
            undone: Vec::new(),
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
        self.replace_each(set, text)
    }

    /// Inserts a line break at every selection, the same way
    /// [`insert`](Self::insert) does. The line break is
    /// [`FileFormat::newline`] of this editor's format. No indentation is
    /// added.
    pub fn newline(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError> {
        self.replace_each(set, self.format.newline().as_str())
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
        self.delete_each(set, Direction::Backward)
    }

    /// Delete at every selection. Like
    /// [`delete_backward`](Self::delete_backward), except a cursor deletes
    /// the character after it, or both bytes of a CRLF if one starts at the
    /// cursor, and at the end of the text deletes nothing.
    pub fn delete_forward(
        &mut self,
        set: &mut BufferSet,
    ) -> Result<Option<TransactionId>, EditorError> {
        self.delete_each(set, Direction::Forward)
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
        let buffer = self.buffer_in(set)?;
        let old = self.selections.clone();
        let clamped = clamp(buffer, &self.selections);
        let text = buffer.to_string();
        let primary = clamped.primary();

        self.selections = if primary.is_empty() {
            match word_at(&text, primary.head) {
                Some(word) => {
                    let mut list = clamped.as_slice().to_vec();
                    list[clamped.primary_index()] = word;
                    normal(list, clamped.primary_index())
                }
                None => clamped,
            }
        } else {
            let needle = &text[primary.range()];
            let end = primary.end();
            let after = occurrences(&text, needle).skip_while(|&at| at < end);
            let before = occurrences(&text, needle).take_while(|&at| at < end);
            let next = after
                .chain(before)
                .map(|at| at..at + needle.len())
                .find(|range| !overlaps_any(&clamped, range));
            let mut selections = clamped;
            if let Some(range) = next {
                selections.push(Selection::new(range.start, range.end));
            }
            selections
        };
        Ok(self.selections != old)
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
        let buffer = self.buffer_in(set)?;
        let old = self.selections.clone();
        let clamped = clamp(buffer, &self.selections);
        let text = buffer.to_string();
        let primary = clamped.primary();

        let searched = if primary.is_empty() {
            word_at(&text, primary.head).map(|word| word.range())
        } else {
            Some(primary.range())
        };
        self.selections = match searched {
            Some(searched) => {
                let needle = &text[searched.clone()];
                let matches: Vec<Selection> = text
                    .match_indices(needle)
                    .map(|(at, _)| Selection::new(at, at + needle.len()))
                    .collect();
                // The searched range is itself an occurrence, so some match
                // overlaps it.
                let primary = matches
                    .iter()
                    .position(|m| m.start() < searched.end && searched.start < m.end())
                    .unwrap_or(0);
                normal(matches, primary)
            }
            None => clamped,
        };
        Ok(self.selections != old)
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
        self.add_cursors(set, Direction::Backward)
    }

    /// Adds a cursor on the line below every selection's head (ED-3, "add
    /// below"), the same way [`add_cursor_above`](Self::add_cursor_above)
    /// does. A selection on the last line adds nothing.
    pub fn add_cursor_below(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        self.add_cursors(set, Direction::Forward)
    }

    /// Undoes the transaction on top of this editor's buffer's undo history
    /// with [`BufferSet::undo`], and returns its id, or `Ok(None)` if there
    /// is nothing to undo. If this editor applied that transaction, the
    /// selections go back to what they were just before it.
    ///
    /// Errors with [`EditorError::Blocked`] when `BufferSet::undo` is
    /// blocked, and then nothing changes.
    pub fn undo(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError> {
        let id = set.undo(self.buffer)?;
        if let Some(record) = id.and_then(|id| self.history.get(&id)) {
            self.selections = record.before.clone();
            self.undone.push(record.id);
        }
        Ok(id)
    }

    /// Redoes the transaction on top of this editor's buffer's redo history
    /// with [`BufferSet::redo`], and returns its id, or `Ok(None)` if there
    /// is nothing to redo. If this editor applied that transaction, the
    /// selections go back to what they were just after it.
    ///
    /// Errors the same way as [`undo`](Self::undo).
    pub fn redo(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError> {
        let id = set.redo(self.buffer)?;
        if let Some(record) = id.and_then(|id| self.history.get(&id)) {
            self.selections = record.after.clone();
            self.undone.retain(|&undone| undone != record.id);
        }
        Ok(id)
    }

    /// Moves every selection by `motion` (T-107): each becomes a cursor
    /// where `motion` takes its head. Returns whether the selections
    /// changed.
    ///
    /// [`Motion::Left`] and [`Motion::Right`] are the exception: a
    /// selection that isn't empty becomes a cursor at its start or its end,
    /// without moving further. [`Motion::Up`] and [`Motion::Down`] use and
    /// keep goal columns, as the type docs describe.
    ///
    /// The selections are clamped first and put back in normal form after,
    /// so cursors that land on one offset merge. The primary is the
    /// selection the old primary became.
    #[expect(
        unused_variables,
        reason = "T-107 interface stub; the implementation deletes this"
    )]
    pub fn move_cursors(&mut self, set: &BufferSet, motion: Motion) -> Result<bool, EditorError> {
        todo!()
    }

    /// Extends every selection by `motion` (T-107): each head moves where
    /// `motion` takes it, and each anchor stays. Returns whether the
    /// selections changed.
    ///
    /// Unlike [`move_cursors`](Self::move_cursors), [`Motion::Left`] and
    /// [`Motion::Right`] move the head of a selection that isn't empty like
    /// any other. A head that moves back onto its anchor leaves a cursor.
    /// Clamping, normal form, the primary and goal columns work as in
    /// `move_cursors`.
    #[expect(
        unused_variables,
        reason = "T-107 interface stub; the implementation deletes this"
    )]
    pub fn extend_selections(
        &mut self,
        set: &BufferSet,
        motion: Motion,
    ) -> Result<bool, EditorError> {
        todo!()
    }

    /// Deletes from every cursor to where `motion` takes it (T-107), in one
    /// transaction, such as Option+Backspace for [`Motion::WordLeft`].
    /// A selection that isn't empty deletes its range instead, as
    /// [`delete_backward`](Self::delete_backward) does.
    ///
    /// Each cursor's range runs between its head and where `motion` takes
    /// it, whichever comes first. Otherwise it works as `delete_backward`:
    /// overlapping ranges are joined, each selection that deleted something
    /// becomes a cursor where its range started, the others stay where they
    /// were, moved by the edits before them, and with nothing to delete it
    /// applies no transaction and returns `Ok(None)`. Undo and redo put
    /// the selections back as for any editing command.
    #[expect(
        unused_variables,
        reason = "T-107 interface stub; the implementation deletes this"
    )]
    pub fn delete_to(
        &mut self,
        set: &mut BufferSet,
        motion: Motion,
    ) -> Result<Option<TransactionId>, EditorError> {
        todo!()
    }

    /// Replaces the selections with one selection of the whole text,
    /// pointing forward from offset 0, as the primary (T-107). Returns
    /// whether the selections changed. In an empty text that is a cursor
    /// at 0.
    #[expect(
        unused_variables,
        reason = "T-107 interface stub; the implementation deletes this"
    )]
    pub fn select_all(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        todo!()
    }

    /// VS Code's Escape (T-107). With more than one selection, keeps only
    /// the primary, as it is. With one selection, makes it a cursor at its
    /// head. Returns whether the selections changed. The selections are
    /// clamped first.
    #[expect(
        unused_variables,
        reason = "T-107 interface stub; the implementation deletes this"
    )]
    pub fn collapse_selections(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        todo!()
    }
}

impl Editor {
    /// This editor's buffer in `set`.
    fn buffer_in<'a>(&self, set: &'a BufferSet) -> Result<&'a Buffer, EditorError> {
        set.get(self.buffer).ok_or(EditorError::UnknownBuffer {
            buffer: self.buffer,
        })
    }

    /// Replaces every selection with `text` in one transaction.
    fn replace_each(
        &mut self,
        set: &mut BufferSet,
        text: &str,
    ) -> Result<Option<TransactionId>, EditorError> {
        let before = clamp(self.buffer_in(set)?, &self.selections);
        self.selections = before.clone();
        if text.is_empty() && before.as_slice().iter().all(Selection::is_empty) {
            return Ok(None);
        }

        let mut transaction = Transaction::new();
        let mut cursors = Vec::with_capacity(before.as_slice().len());
        // Bytes added and removed by the edits before the current one.
        let (mut added, mut removed) = (0, 0);
        for selection in before.as_slice() {
            let range = selection.range();
            if !(range.is_empty() && text.is_empty()) {
                transaction.push(Edit::replace(self.buffer, range.clone(), text));
            }
            added += text.len();
            cursors.push(Selection::cursor(range.start - removed + added));
            removed += range.len();
        }
        let after = normal(cursors, before.primary_index());
        Ok(Some(self.apply(set, transaction, before, after)))
    }

    /// Deletes at every selection in one transaction, before or after each
    /// cursor.
    fn delete_each(
        &mut self,
        set: &mut BufferSet,
        direction: Direction,
    ) -> Result<Option<TransactionId>, EditorError> {
        let buffer = self.buffer_in(set)?;
        let before = clamp(buffer, &self.selections);
        self.selections = before.clone();

        let ranges: Vec<Option<Range<usize>>> = before
            .as_slice()
            .iter()
            .map(|&selection| delete_range(buffer, selection, direction))
            .collect();
        let mut sorted: Vec<Range<usize>> = ranges.iter().flatten().cloned().collect();
        if sorted.is_empty() {
            return Ok(None);
        }
        sorted.sort_by_key(|range| (range.start, range.end));
        let mut joined: Vec<Range<usize>> = Vec::with_capacity(sorted.len());
        for range in sorted {
            match joined.last_mut() {
                Some(last) if range.start < last.end => last.end = last.end.max(range.end),
                _ => joined.push(range),
            }
        }
        // `removed[k]` is the bytes the first `k` joined ranges remove.
        let mut removed = Vec::with_capacity(joined.len() + 1);
        removed.push(0);
        for range in &joined {
            removed.push(removed[removed.len() - 1] + range.len());
        }

        // Joined ranges are sorted and apart, so their ends increase too.
        let cursors = before
            .as_slice()
            .iter()
            .zip(&ranges)
            .map(|(selection, range)| {
                let at = match range {
                    Some(range) => joined[joined.partition_point(|j| j.end <= range.start)].start,
                    None => selection.head,
                };
                Selection::cursor(at - removed[joined.partition_point(|j| j.end <= at)])
            })
            .collect();
        let after = normal(cursors, before.primary_index());
        let transaction = joined
            .into_iter()
            .map(|range| Edit::delete(self.buffer, range))
            .collect();
        Ok(Some(self.apply(set, transaction, before, after)))
    }

    /// Applies `transaction`, which this editor built from `before`, and
    /// moves to `after`.
    fn apply(
        &mut self,
        set: &mut BufferSet,
        transaction: Transaction,
        before: Selections,
        after: Selections,
    ) -> TransactionId {
        let id = set
            .apply(transaction)
            .expect("edits come from clamped selections in normal form, so they are valid");
        for undone in self.undone.drain(..) {
            self.history.remove(&undone);
        }
        self.selections = after.clone();
        self.history.insert(id, Record { id, before, after });
        id
    }

    /// Adds a cursor on the line above or below every selection's head.
    fn add_cursors(&mut self, set: &BufferSet, direction: Direction) -> Result<bool, EditorError> {
        let buffer = self.buffer_in(set)?;
        let old = self.selections.clone();
        let clamped = clamp(buffer, &self.selections);

        let mut list = clamped.as_slice().to_vec();
        let mut primary = clamped.primary_index();
        for (i, selection) in clamped.as_slice().iter().enumerate() {
            if let Some(at) = next_line_offset(buffer, selection.head, direction) {
                if i == clamped.primary_index() {
                    primary = list.len();
                }
                list.push(Selection::cursor(at));
            }
        }
        self.selections = normal(list, primary);
        Ok(self.selections != old)
    }
}

/// `selections` in normal form. `selections` is never empty and `primary`
/// always indexes it where this is called.
fn normal(selections: Vec<Selection>, primary: usize) -> Selections {
    Selections::new(selections, primary).expect("non-empty, with the primary in range")
}

/// `selections` with every offset clamped to `buffer`, in normal form.
fn clamp(buffer: &Buffer, selections: &Selections) -> Selections {
    let list = selections
        .as_slice()
        .iter()
        .map(|selection| {
            Selection::new(
                clamp_offset(buffer, selection.anchor),
                clamp_offset(buffer, selection.head),
            )
        })
        .collect();
    normal(list, selections.primary_index())
}

/// `offset` moved to the end of the text if past it, or back to the start
/// of the character it falls inside.
fn clamp_offset(buffer: &Buffer, offset: usize) -> usize {
    let len = buffer.len_bytes();
    if offset >= len {
        return len;
    }
    // A character is at most four bytes and 0 is a boundary, so this takes
    // at most three steps back.
    let mut offset = offset;
    while buffer.byte_to_char(offset).is_err() {
        offset -= 1;
    }
    offset
}

/// Whether `buffer`'s text in `range` is `expected`. False for a range that
/// isn't valid.
fn text_is(buffer: &Buffer, range: Range<usize>, expected: &str) -> bool {
    buffer.slice(range).is_ok_and(|text| text == expected)
}

/// The range `selection` deletes on its own, or `None` if it deletes
/// nothing: its range if it isn't empty, else the character or CRLF before
/// or after the cursor.
fn delete_range(
    buffer: &Buffer,
    selection: Selection,
    direction: Direction,
) -> Option<Range<usize>> {
    if !selection.is_empty() {
        return Some(selection.range());
    }
    let at = selection.head;
    let char = buffer.byte_to_char(at).ok()?;
    match direction {
        Direction::Backward => {
            let start = buffer.char_to_byte(char.checked_sub(1)?).ok()?;
            if start > 0
                && text_is(buffer, start..at, "\n")
                && text_is(buffer, start - 1..start, "\r")
            {
                Some(start - 1..at)
            } else {
                Some(start..at)
            }
        }
        Direction::Forward => {
            let end = buffer.char_to_byte(char + 1).ok()?;
            if text_is(buffer, at..end, "\r") && text_is(buffer, end..end + 1, "\n") {
                Some(at..end + 1)
            } else {
                Some(at..end)
            }
        }
    }
}

/// Where line `line`'s text ends, before its line break.
fn line_text_end(buffer: &Buffer, line: usize) -> Option<usize> {
    if line + 1 >= buffer.len_lines() {
        return Some(buffer.len_bytes());
    }
    let next = buffer.line_to_byte(line + 1).ok()?;
    if next >= 2 && text_is(buffer, next - 2..next, "\r\n") {
        Some(next - 2)
    } else {
        next.checked_sub(1)
    }
}

/// The offset at `head`'s column, in chars, on the line above or below it,
/// or `None` if there is no such line. The column is capped at the end of
/// each line's text.
fn next_line_offset(buffer: &Buffer, head: usize, direction: Direction) -> Option<usize> {
    let line = buffer.byte_to_line(head).ok()?;
    let target = match direction {
        Direction::Backward => line.checked_sub(1)?,
        Direction::Forward => line + 1,
    };
    if target >= buffer.len_lines() {
        return None;
    }
    let line_start = buffer.byte_to_char(buffer.line_to_byte(line).ok()?).ok()?;
    let column = buffer
        .byte_to_char(head.min(line_text_end(buffer, line)?))
        .ok()?
        - line_start;
    let target_start = buffer
        .byte_to_char(buffer.line_to_byte(target).ok()?)
        .ok()?;
    let target_end = buffer.byte_to_char(line_text_end(buffer, target)?).ok()?;
    buffer
        .char_to_byte((target_start + column).min(target_end))
        .ok()
}

/// Whether `c` is a word character: alphanumeric or `_`.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The word around char boundary `at` of `text`, pointing forward, or
/// `None` if neither neighbour is a word character.
fn word_at(text: &str, at: usize) -> Option<Selection> {
    let left: usize = text[..at]
        .chars()
        .rev()
        .take_while(|&c| is_word(c))
        .map(char::len_utf8)
        .sum();
    let right: usize = text[at..]
        .chars()
        .take_while(|&c| is_word(c))
        .map(char::len_utf8)
        .sum();
    (left + right > 0).then(|| Selection::new(at - left, at + right))
}

/// Every offset where non-empty `needle` occurs in `text`, in order,
/// including occurrences that overlap each other.
fn occurrences<'a>(text: &'a str, needle: &'a str) -> impl Iterator<Item = usize> + 'a {
    // Searching on from the next char boundary after each match finds the
    // overlapping ones.
    let step = needle.chars().next().map_or(1, char::len_utf8);
    let mut from = 0;
    std::iter::from_fn(move || {
        let at = from + text.get(from..)?.find(needle)?;
        from = at + step;
        Some(at)
    })
}

/// Whether `range` overlaps a selection in `selections`: shares a byte
/// with it, or has a cursor strictly inside it.
fn overlaps_any(selections: &Selections, range: &Range<usize>) -> bool {
    // In normal form, ends increase with starts, so the first selection
    // ending after `range.start` is the only one that can overlap first.
    let list = selections.as_slice();
    let i = list.partition_point(|s| s.end() <= range.start);
    list.get(i).is_some_and(|s| s.start() < range.end)
}
