//! `Editor`: selections on one buffer, and the commands that edit at every
//! cursor or add selections.

use std::collections::HashMap;
use std::ops::Range;

use helm_text::{
    Buffer, BufferId, BufferSet, Edit, FileFormat, TextBuffer, Transaction, TransactionId,
};

use crate::motion::{Reads, target, try_target};
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
    /// Each selection's goal column, in chars, by index into `selections`,
    /// or empty when there are none. Only Up and Down set and keep them.
    goals: Vec<usize>,
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
            goals: Vec::new(),
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
        self.goals.clear();
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
        self.delete_each(set, |buffer, selection| {
            delete_range(buffer, selection, Direction::Backward)
        })
    }

    /// Delete at every selection. Like
    /// [`delete_backward`](Self::delete_backward), except a cursor deletes
    /// the character after it, or both bytes of a CRLF if one starts at the
    /// cursor, and at the end of the text deletes nothing.
    pub fn delete_forward(
        &mut self,
        set: &mut BufferSet,
    ) -> Result<Option<TransactionId>, EditorError> {
        self.delete_each(set, |buffer, selection| {
            delete_range(buffer, selection, Direction::Forward)
        })
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
        self.goals.clear();
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
        self.goals.clear();
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
        self.goals.clear();
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
        self.goals.clear();
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
    pub fn move_cursors(&mut self, set: &BufferSet, motion: Motion) -> Result<bool, EditorError> {
        self.move_heads(set, motion, false)
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
    pub fn extend_selections(
        &mut self,
        set: &BufferSet,
        motion: Motion,
    ) -> Result<bool, EditorError> {
        self.move_heads(set, motion, true)
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
    pub fn delete_to(
        &mut self,
        set: &mut BufferSet,
        motion: Motion,
    ) -> Result<Option<TransactionId>, EditorError> {
        let mut reads = Reads::default();
        self.delete_each(set, |buffer, selection| {
            if !selection.is_empty() {
                return Some(selection.range());
            }
            let head = selection.head;
            let to = target(buffer, head, motion, None, &mut reads);
            (to != head).then(|| head.min(to)..head.max(to))
        })
    }

    /// Replaces the selections with one selection of the whole text,
    /// pointing forward from offset 0, as the primary (T-107). Returns
    /// whether the selections changed. In an empty text that is a cursor
    /// at 0.
    pub fn select_all(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        let buffer = self.buffer_in(set)?;
        self.goals.clear();
        let old = std::mem::replace(
            &mut self.selections,
            Selections::single(Selection::new(0, buffer.len_bytes())),
        );
        Ok(self.selections != old)
    }

    /// VS Code's Escape (T-107). With more than one selection, keeps only
    /// the primary, as it is. With one selection, makes it a cursor at its
    /// head. Returns whether the selections changed. The selections are
    /// clamped first.
    pub fn collapse_selections(&mut self, set: &BufferSet) -> Result<bool, EditorError> {
        let buffer = self.buffer_in(set)?;
        self.goals.clear();
        let clamped = clamp(buffer, &self.selections);
        let primary = clamped.primary();
        let collapsed = if clamped.as_slice().len() > 1 {
            primary
        } else {
            Selection::cursor(primary.head)
        };
        let old = std::mem::replace(&mut self.selections, Selections::single(collapsed));
        Ok(self.selections != old)
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
        self.goals.clear();
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

    /// Deletes the range `range_of` gives each selection, in one
    /// transaction. `None` deletes nothing at that selection.
    fn delete_each(
        &mut self,
        set: &mut BufferSet,
        mut range_of: impl FnMut(&Buffer, Selection) -> Option<Range<usize>>,
    ) -> Result<Option<TransactionId>, EditorError> {
        let buffer = self.buffer_in(set)?;
        self.goals.clear();
        let before = clamp(buffer, &self.selections);
        self.selections = before.clone();

        let ranges: Vec<Option<Range<usize>>> = before
            .as_slice()
            .iter()
            .map(|&selection| range_of(buffer, selection))
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

    /// Moves every selection's head by `motion`: to a cursor there, or with
    /// the anchor kept when `extend` is set. Up and Down use and keep the
    /// goal columns; every other motion drops them.
    fn move_heads(
        &mut self,
        set: &BufferSet,
        motion: Motion,
        extend: bool,
    ) -> Result<bool, EditorError> {
        let buffer = self.buffer_in(set)?;
        let vertical = matches!(motion, Motion::Up | Motion::Down);
        let goals = std::mem::take(&mut self.goals);

        // Usually every offset is valid, so clamping would change nothing.
        // Each motion checks the head as it reads next to it, which costs
        // nothing extra, and only when one isn't valid do the selections get
        // clamped first, as the type docs describe.
        let (moved, primary) = match moved(buffer, &self.selections, motion, extend, &goals) {
            Some(moved) => (moved, self.selections.primary_index()),
            None => {
                let list = clamp_each(buffer, &self.selections);
                let clamped = normal(list.clone(), self.selections.primary_index());
                // Each selection keeps its goal through clamping, and one
                // that clamping merged keeps the goal of one merged into it.
                let goals = if goals.len() == list.len() {
                    let members = list.iter().map(Selection::start).zip(goals).collect();
                    owned_goals(members, &clamped)
                } else {
                    Vec::new()
                };
                let moved = moved(buffer, &clamped, motion, extend, &goals)
                    .expect("clamped offsets are valid");
                (moved, clamped.primary_index())
            }
        };

        let mut members = Vec::new();
        if vertical {
            members = moved
                .iter()
                .map(|&(selection, goal)| (selection.start(), goal))
                .collect();
        }
        let selections = normal(
            moved.into_iter().map(|(selection, _)| selection).collect(),
            primary,
        );
        if vertical {
            self.goals = owned_goals(members, &selections);
        }
        let changed = selections != self.selections;
        self.selections = selections;
        Ok(changed)
    }

    /// Adds a cursor on the line above or below every selection's head.
    fn add_cursors(&mut self, set: &BufferSet, direction: Direction) -> Result<bool, EditorError> {
        let buffer = self.buffer_in(set)?;
        self.goals.clear();
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
    normal(clamp_each(buffer, selections), selections.primary_index())
}

/// Each of `selections` with its offsets clamped to `buffer`, in the same
/// order and before normal form. Clamping never moves one offset past
/// another, so the starts stay sorted.
fn clamp_each(buffer: &Buffer, selections: &Selections) -> Vec<Selection> {
    selections
        .as_slice()
        .iter()
        .map(|selection| {
            let head = clamp_offset(buffer, selection.head);
            if selection.is_empty() {
                Selection::cursor(head)
            } else {
                Selection::new(clamp_offset(buffer, selection.anchor), head)
            }
        })
        .collect()
}

/// The goal of each selection in `result`, which is `members` (each
/// selection's start and goal) put in normal form.
///
/// Each selection in `result` starts where one of the selections merged
/// into it starts, and no selection that stayed apart starts there too: a
/// non-empty one that only touches starts at its end, and an empty one
/// there would have merged. So the first member with that start is one of
/// its own, and its goal is the one it keeps. A selection nothing merged
/// into keeps its own goal.
fn owned_goals(mut members: Vec<(usize, usize)>, result: &Selections) -> Vec<usize> {
    if !members.is_sorted_by_key(|&(start, _)| start) {
        members.sort_by_key(|&(start, _)| start);
    }
    result
        .as_slice()
        .iter()
        .map(|selection| {
            let i = members.partition_point(|&(start, _)| start < selection.start());
            members.get(i).map_or(0, |&(_, goal)| goal)
        })
        .collect()
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

/// Every selection in `selections` moved by `motion` as
/// [`Editor::move_cursors`] (or, with `extend`, as
/// [`Editor::extend_selections`]) does, each with its goal column, before
/// normal form. `goals` lines up with `selections`, or is empty. Returns
/// `None` if an offset that matters isn't valid, so the caller clamps
/// first.
fn moved(
    buffer: &Buffer,
    selections: &Selections,
    motion: Motion,
    extend: bool,
    goals: &[usize],
) -> Option<Vec<(Selection, usize)>> {
    let mut moved = Vec::with_capacity(selections.as_slice().len());
    let mut reads = Reads::default();
    for (i, &selection) in selections.as_slice().iter().enumerate() {
        let collapse = !extend && !selection.is_empty();
        let (to, goal) = match motion {
            // VS Code's Left and Right collapse a selection to its side. A
            // side between the CR and LF of a CRLF moves out of it, the way
            // the motion itself would.
            Motion::Left if collapse => (reads.collapse_left(buffer, selection.start())?, 0),
            Motion::Right if collapse => (reads.collapse_right(buffer, selection.end())?, 0),
            _ => try_target(
                buffer,
                selection.head,
                motion,
                goals.get(i).copied(),
                &mut reads,
            )?,
        };
        // The other end must be valid too, even when the move drops it:
        // clamping a stale one could merge selections before the move.
        // It is usually inside the text just read, so this costs nothing.
        let other = match motion {
            Motion::Left if collapse => selection.end(),
            Motion::Right if collapse => selection.start(),
            _ => selection.anchor,
        };
        if !selection.is_empty() && !reads.is_valid(buffer, other) {
            return None;
        }
        let selection = if extend {
            Selection::new(selection.anchor, to)
        } else {
            Selection::cursor(to)
        };
        moved.push((selection, goal));
    }
    Some(moved)
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
pub(crate) fn is_word(c: char) -> bool {
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

#[cfg(test)]
mod tests {
    use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

    use super::*;

    fn editor(text: &str, selections: Vec<Selection>, primary: usize) -> (BufferSet, Editor) {
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text));
        let format = FileFormat {
            bom: false,
            line_endings: LineEndings::count(text),
        };
        let mut editor = Editor::new(id, format);
        editor.set_selections(Selections::new(selections, primary).expect("valid"));
        (set, editor)
    }

    #[test]
    fn collapsing_never_lands_inside_a_crlf() {
        let (set, mut e) = editor("a\r\nb", vec![Selection::new(2, 4)], 0);
        e.move_cursors(&set, Motion::Left).expect("in the set");
        assert_eq!(e.selections().as_slice(), [Selection::cursor(1)]);
        let (set, mut e) = editor("a\r\nb", vec![Selection::new(0, 2)], 0);
        e.move_cursors(&set, Motion::Right).expect("in the set");
        assert_eq!(e.selections().as_slice(), [Selection::cursor(3)]);
        let (set, mut e) = editor("a\r\nb", vec![Selection::new(4, 2)], 0);
        e.move_cursors(&set, Motion::Left).expect("in the set");
        assert_eq!(e.selections().as_slice(), [Selection::cursor(1)]);
    }

    #[test]
    fn stale_anchors_are_clamped_before_moving() {
        // Offset 2 is inside é; 100 is past the end (7).
        for (selection, motion, expected) in [
            (Selection::new(2, 4), Motion::Left, Selection::cursor(1)),
            (Selection::new(4, 2), Motion::Left, Selection::cursor(1)),
            (Selection::new(100, 0), Motion::Right, Selection::cursor(7)),
            (
                Selection::new(2, 5),
                Motion::WordRight,
                Selection::cursor(7),
            ),
        ] {
            let (set, mut e) = editor("aéb\nxy", vec![selection], 0);
            e.move_cursors(&set, motion).expect("in the set");
            assert_eq!(
                e.selections().as_slice(),
                [expected],
                "{selection:?} {motion:?}"
            );
        }
        // Extending keeps the clamped anchor.
        let (set, mut e) = editor("aéb\nxy", vec![Selection::new(2, 4)], 0);
        e.extend_selections(&set, Motion::Right)
            .expect("in the set");
        assert_eq!(e.selections().as_slice(), [Selection::new(1, 5)]);
    }

    /// Each selection moved on its own editor, as the goal rules define.
    fn alone(text: &str, selections: &[Selection], steps: &[Motion]) -> Vec<Selection> {
        selections
            .iter()
            .map(|&selection| {
                let (set, mut e) = editor(text, vec![selection], 0);
                for &motion in steps {
                    e.extend_selections(&set, motion).expect("in the set");
                }
                e.selections().primary()
            })
            .collect()
    }

    #[test]
    fn touching_selections_keep_their_own_goals() {
        // Shift+Down leaves 0..9 and 9..19 touching but apart; Shift+Up
        // must take each back by its own goal (columns 2 and 5).
        let text = "abcdef\n".repeat(4);
        let steps = [Motion::Down, Motion::Up];
        for selections in [
            vec![Selection::new(0, 2), Selection::new(9, 12)],
            vec![Selection::new(2, 0), Selection::new(9, 12)],
            vec![Selection::new(0, 2), Selection::new(12, 9)],
        ] {
            for primary in 0..2 {
                let (set, mut e) = editor(&text, selections.clone(), primary);
                for i in 0..steps.len() {
                    e.extend_selections(&set, steps[i]).expect("in the set");
                    let expected = alone(&text, &selections, &steps[..=i]);
                    assert_eq!(e.selections().as_slice(), expected, "{selections:?} {i}");
                    assert_eq!(e.selections().primary_index(), primary);
                }
            }
        }
    }

    #[test]
    fn clamping_keeps_each_selections_goal() {
        // Lines start at 0, 7 ("ab"), 10, 17, 24 and 31.
        let text = "abcdef\nab\nabcdef\nabcdef\nabcdef\nabcdef";
        let (mut set, mut e) = editor(
            text,
            vec![
                Selection::cursor(5),
                Selection::cursor(18),
                Selection::cursor(25),
            ],
            2,
        );
        e.move_cursors(&set, Motion::Down).expect("in the set");
        assert_eq!(
            e.selections().as_slice(),
            [
                Selection::cursor(9),
                Selection::cursor(25),
                Selection::cursor(32)
            ]
        );
        // An edit outside the editor cuts the text at 17, so the last two
        // cursors clamp to 17 and merge there.
        let mut transaction = Transaction::new();
        transaction.push(Edit::delete(e.buffer(), 17..text.len()));
        set.apply(transaction).expect("valid edit");
        e.move_cursors(&set, Motion::Down).expect("in the set");
        // The first cursor keeps its goal, column 5, through the short
        // line; the merged one is on the last line and stays at the end.
        assert_eq!(
            e.selections().as_slice(),
            [Selection::cursor(15), Selection::cursor(17)]
        );
        assert_eq!(e.selections().primary_index(), 1);
        e.move_cursors(&set, Motion::Up).expect("in the set");
        // Back up with column 5 for the first, and column 1 (the goal both
        // merged cursors had) for the second.
        assert_eq!(
            e.selections().as_slice(),
            [Selection::cursor(9), Selection::cursor(11)]
        );
    }

    #[test]
    fn merged_selections_keep_a_goal_of_their_own() {
        // Lines start at 0, 7 ("ab"), 10, 17 and 24 (empty).
        let text = "abcdef\nab\nabcdef\nabcdef\n";
        // Cursors at columns 4 and 5 both land at the end of "ab" and
        // merge there. Whichever goal the merged cursor keeps, 4 or 5, the
        // next Down takes it to that column, never to the third cursor's.
        for primary in 0..3 {
            let selections = vec![
                Selection::cursor(4),
                Selection::cursor(5),
                Selection::cursor(18),
            ];
            let (set, mut e) = editor(text, selections, primary);
            e.move_cursors(&set, Motion::Down).expect("in the set");
            assert_eq!(
                e.selections().as_slice(),
                [Selection::cursor(9), Selection::cursor(24)]
            );
            assert_eq!(e.selections().primary_index(), primary.saturating_sub(1));
            e.move_cursors(&set, Motion::Up).expect("in the set");
            let merged = e.selections().as_slice()[0].head;
            assert!(merged == 4 || merged == 5, "{merged}");
            // The third cursor went to the empty last line and keeps its
            // own goal, column 1.
            assert_eq!(e.selections().as_slice()[1], Selection::cursor(18));
        }
    }
}
