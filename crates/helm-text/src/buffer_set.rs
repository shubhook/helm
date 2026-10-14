//! `BufferSet`: buffers that share one undo history, so a transaction can
//! span several of them.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::mem;
use std::ops::Range;

use crate::{
    Buffer, BufferId, HistoryError, TextBuffer, Transaction, TransactionError, TransactionId,
};

/// Buffers that edit, undo and redo through transactions, including
/// transactions that touch several of them at once.
///
/// The set owns its buffers. [`get`](Self::get) lends them out read-only,
/// and the only way to change one is [`apply`](Self::apply),
/// [`undo`](Self::undo) or [`redo`](Self::redo). So no buffer can change
/// behind the history's back, and the rules below hold by construction.
///
/// # Applying a transaction
///
/// Every edit's range is in bytes of its buffer's text *before the
/// transaction*, not after the edits listed before it. This is how an LSP
/// `WorkspaceEdit` and a diff both describe their changes. For each buffer,
/// the result is that buffer's old text with every edit's range replaced by
/// its text, where:
///
/// - Edits on the same buffer must not overlap. Two ranges overlap when
///   `a.start < b.end && b.start < a.end`. So an insert strictly inside
///   another edit's range overlaps it, while an insert at either end of a
///   range, and two inserts at the same offset, do not.
/// - Edits are placed in order of their range's start. At the same start,
///   inserts come first, in the order they appear in the transaction, then
///   the one edit with a non-empty range, if any. So an insert at the start
///   of a replaced range lands before the replacement, and an insert at its
///   end lands after it.
///
/// `apply` checks every edit before it changes anything. It checks the edits
/// in order and returns the first problem it finds, naming the edit by its
/// index in [`Transaction::edits`]:
///
/// 1. A transaction with no edits is [`TransactionError::Empty`].
/// 2. For each edit in order: its buffer must be in the set
///    ([`TransactionError::UnknownBuffer`]); then its range must be valid in
///    that buffer's old text, checked the way [`TextBuffer::slice`]
///    documents ([`TransactionError::InvalidRange`]); then it must not
///    overlap an earlier edit on the same buffer
///    ([`TransactionError::Overlap`], naming the earliest such edit).
///
/// If any check fails, no buffer and no history changes. A transaction
/// whose edits leave the text as it was, such as an empty insert, is still
/// applied and recorded like any other.
///
/// # Undo and redo
///
/// Each buffer has an undo history and a redo history, both stacks of
/// transactions. A transaction appears in the histories of every buffer it
/// touched, and is undone or redone in all of them at once, never in some.
///
/// - [`apply`](Self::apply) pushes the transaction onto the undo history of
///   every buffer it touches and drops their redo histories (see below).
/// - [`undo(buffer)`](Self::undo) takes the transaction on top of `buffer`'s
///   undo history and reverts it in every buffer it touched, restoring each
///   one's text exactly. It then moves from the undo history to the redo
///   history of each of those buffers.
/// - [`redo(buffer)`](Self::redo) does the reverse with `buffer`'s redo
///   history, restoring each buffer's text to exactly what it was after the
///   transaction.
///
/// **A later edit blocks an undo.** A transaction can be undone only when it
/// is on top of the undo history of *every* buffer it touched. If a later
/// transaction touched one of those buffers and is still applied, undo
/// returns [`HistoryError::Blocked`] and changes nothing. The error names the
/// transaction and the touched buffer, lowest id first, whose undo history
/// has a later transaction on top. Undoing that buffer first unblocks it.
/// Redo follows the same rule with the redo histories.
///
/// The transaction applied last is always on top of every history it is in,
/// so undoing it never blocks, and repeatedly undoing whatever is not
/// blocked always empties every undo history.
///
/// **A new transaction drops redo.** Applying a transaction drops every
/// transaction in the redo histories of the buffers it touches. Dropping a
/// transaction removes it from the redo history of every buffer it touched,
/// including buffers the new transaction doesn't touch, together with every
/// transaction below it in those histories: those were applied after it, so
/// they can't be redone without it. That repeats until nothing more drops.
/// Every other transaction stays in the redo histories it was in, so typing
/// in one buffer doesn't drop redo in an unrelated one.
///
/// A failed `apply`, an undo or redo with nothing to do, and a blocked undo
/// or redo leave every history as it was.
///
/// [`TextBuffer::slice`]: crate::TextBuffer::slice
#[derive(Debug)]
pub struct BufferSet {
    /// The buffers with their histories. A `BufferId`'s number is its index.
    entries: Vec<Entry>,
    /// Every transaction that is still in some undo or redo history, by its
    /// id's number.
    transactions: HashMap<u64, Record>,
    /// The number the next applied transaction's id gets.
    next_transaction: u64,
}

/// One buffer and its histories.
#[derive(Debug)]
struct Entry {
    buffer: Buffer,
    /// Transaction id numbers, the most recent on top (last).
    undo: Vec<u64>,
    /// Transaction id numbers, the most recently undone on top (last).
    redo: Vec<u64>,
}

/// What one transaction did, kept to undo and redo it.
#[derive(Debug)]
struct Record {
    /// One change per buffer the transaction touched, lowest buffer first.
    changes: Vec<Change>,
}

/// What one transaction did to one buffer.
#[derive(Debug)]
struct Change {
    /// The buffer's index in `entries`.
    buffer: usize,
    /// Turns the text before the transaction into the text after it.
    forward: Vec<Splice>,
    /// Turns the text after the transaction back into the text before it.
    inverse: Vec<Splice>,
}

/// Replace `range` with `text`. A change's splices are in order of their
/// start, don't overlap, and all refer to the same text, so applying them
/// from last to first never shifts one that is still to come.
#[derive(Debug)]
struct Splice {
    range: Range<usize>,
    text: String,
}

impl BufferSet {
    /// An empty set.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            transactions: HashMap::new(),
            next_transaction: 0,
        }
    }

    /// Adds `buffer` to the set with an empty undo and redo history, and
    /// returns its id. Its text when added is the text that undoing every
    /// transaction returns it to.
    pub fn add(&mut self, buffer: Buffer) -> BufferId {
        let id = BufferId(self.entries.len() as u64);
        self.entries.push(Entry {
            buffer,
            undo: Vec::new(),
            redo: Vec::new(),
        });
        id
    }

    /// The buffer with id `id`, or `None` if the set has no such buffer.
    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.index(id).map(|index| &self.entries[index].buffer)
    }

    /// Checks every edit of `transaction`, then applies them all, records
    /// the transaction in the undo history of every buffer it touches, and
    /// returns its id.
    ///
    /// Errors with the first problem found, as the [type docs](Self)
    /// describe, and then changes nothing.
    pub fn apply(&mut self, transaction: Transaction) -> Result<TransactionId, TransactionError> {
        let edits = transaction.into_edits();
        if edits.is_empty() {
            return Err(TransactionError::Empty);
        }

        // Check every edit, collecting the text each one replaces and, per
        // buffer, the edits as `(start, end, index)`. Sorted, that is the
        // order the edits are placed in.
        let mut olds = Vec::with_capacity(edits.len());
        let mut placed: BTreeMap<usize, BTreeSet<(usize, usize, usize)>> = BTreeMap::new();
        for (i, edit) in edits.iter().enumerate() {
            let index = self
                .index(edit.buffer)
                .ok_or(TransactionError::UnknownBuffer {
                    edit: i,
                    buffer: edit.buffer,
                })?;
            let old = self.entries[index]
                .buffer
                .slice(edit.range.clone())
                .map_err(|error| TransactionError::InvalidRange { edit: i, error })?;
            let mine = placed.entry(index).or_default();
            if let Some(other) = earliest_overlap(mine, &edit.range) {
                return Err(TransactionError::Overlap { edit: i, other });
            }
            mine.insert((edit.range.start, edit.range.end, i));
            olds.push(old);
        }

        let touched: Vec<usize> = placed.keys().copied().collect();
        self.drop_redo(&touched);

        let mut texts: Vec<String> = edits.into_iter().map(|edit| edit.text).collect();
        let id = self.next_transaction;
        self.next_transaction += 1;
        let mut changes = Vec::with_capacity(placed.len());
        for (index, mine) in placed {
            let mut forward = Vec::with_capacity(mine.len());
            let mut inverse = Vec::with_capacity(mine.len());
            // Bytes added and removed by the edits placed so far, which move
            // where this edit's text lands in the new text.
            let (mut added, mut removed) = (0, 0);
            for (start, end, i) in mine {
                let text = mem::take(&mut texts[i]);
                let new_start = start + added - removed;
                inverse.push(Splice {
                    range: new_start..new_start + text.len(),
                    text: mem::take(&mut olds[i]),
                });
                added += text.len();
                removed += end - start;
                forward.push(Splice {
                    range: start..end,
                    text,
                });
            }
            let entry = &mut self.entries[index];
            splice_all(&mut entry.buffer, &forward);
            entry.undo.push(id);
            changes.push(Change {
                buffer: index,
                forward,
                inverse,
            });
        }
        self.transactions.insert(id, Record { changes });
        Ok(TransactionId(id))
    }

    /// Reverts the transaction on top of `buffer`'s undo history in every
    /// buffer it touched, and returns its id. Returns `Ok(None)` if
    /// `buffer`'s undo history is empty.
    ///
    /// Errors with [`HistoryError::UnknownBuffer`] if the set has no such
    /// buffer, or [`HistoryError::Blocked`] if a later transaction is on top
    /// of the undo history of another buffer the transaction touched. On an
    /// error, nothing changes.
    pub fn undo(&mut self, buffer: BufferId) -> Result<Option<TransactionId>, HistoryError> {
        self.step(buffer, Direction::Undo)
    }

    /// Re-applies the transaction on top of `buffer`'s redo history in
    /// every buffer it touched, and returns its id. Returns `Ok(None)` if
    /// `buffer`'s redo history is empty.
    ///
    /// Errors the same way as [`undo`](Self::undo), with the redo histories
    /// in place of the undo histories. On an error, nothing changes.
    pub fn redo(&mut self, buffer: BufferId) -> Result<Option<TransactionId>, HistoryError> {
        self.step(buffer, Direction::Redo)
    }

    /// The index in `entries` of buffer `id`, if the set has it.
    fn index(&self, id: BufferId) -> Option<usize> {
        usize::try_from(id.0)
            .ok()
            .filter(|&index| index < self.entries.len())
    }

    /// Undoes or redoes the transaction on top of `buffer`'s history.
    fn step(
        &mut self,
        buffer: BufferId,
        direction: Direction,
    ) -> Result<Option<TransactionId>, HistoryError> {
        let index = self
            .index(buffer)
            .ok_or(HistoryError::UnknownBuffer { buffer })?;
        let Some(&top) = direction.from(&self.entries[index]).last() else {
            return Ok(None);
        };
        let record = &self.transactions[&top];

        // Changes are sorted by buffer, so the first blocked one is the
        // lowest id.
        if let Some(change) = record
            .changes
            .iter()
            .find(|change| direction.from(&self.entries[change.buffer]).last() != Some(&top))
        {
            return Err(HistoryError::Blocked {
                transaction: TransactionId(top),
                buffer: BufferId(change.buffer as u64),
            });
        }

        for change in &record.changes {
            let entry = &mut self.entries[change.buffer];
            let (splices, from, to) = match direction {
                Direction::Undo => (&change.inverse, &mut entry.undo, &mut entry.redo),
                Direction::Redo => (&change.forward, &mut entry.redo, &mut entry.undo),
            };
            splice_all(&mut entry.buffer, splices);
            from.pop();
            to.push(top);
        }
        Ok(Some(TransactionId(top)))
    }

    /// Drops the redo histories of the buffers at `touched`, then every
    /// transaction below a dropped one in any redo history, until nothing
    /// more drops. Dropped transactions leave every history and the set.
    fn drop_redo(&mut self, touched: &[usize]) {
        let mut dropped = HashSet::new();
        let mut queue = Vec::new();
        for &index in touched {
            for &id in &self.entries[index].redo {
                if dropped.insert(id) {
                    queue.push(id);
                }
            }
        }
        while let Some(id) = queue.pop() {
            for change in &self.transactions[&id].changes {
                let redo = &self.entries[change.buffer].redo;
                if let Some(at) = redo.iter().position(|&other| other == id) {
                    for &below in &redo[..at] {
                        if dropped.insert(below) {
                            queue.push(below);
                        }
                    }
                }
            }
        }

        let mut affected = BTreeSet::new();
        for id in &dropped {
            if let Some(record) = self.transactions.remove(id) {
                affected.extend(record.changes.iter().map(|change| change.buffer));
            }
        }
        for index in affected {
            self.entries[index].redo.retain(|id| !dropped.contains(id));
        }
    }
}

/// Which way [`BufferSet::step`] moves a transaction.
#[derive(Clone, Copy)]
enum Direction {
    Undo,
    Redo,
}

impl Direction {
    /// The history a transaction leaves.
    fn from(self, entry: &Entry) -> &[u64] {
        match self {
            Self::Undo => &entry.undo,
            Self::Redo => &entry.redo,
        }
    }
}

/// Applies `splices`, which are sorted, don't overlap and refer to the
/// buffer's current text, from last to first.
fn splice_all(buffer: &mut Buffer, splices: &[Splice]) {
    for splice in splices.iter().rev() {
        buffer.splice(splice.range.clone(), &splice.text);
    }
}

/// The lowest index of an edit in `placed` that overlaps `range`.
///
/// The edits in `placed` don't overlap each other, so sorted by start their
/// ends never decrease either. The ones that overlap `range` then sit
/// together right before the first edit that starts at or after `range.end`,
/// and walking back from there can stop at the first one that ends at or
/// before `range.start`.
fn earliest_overlap(
    placed: &BTreeSet<(usize, usize, usize)>,
    range: &Range<usize>,
) -> Option<usize> {
    placed
        .range(..(range.end, 0, 0))
        .rev()
        .take_while(|&&(_, end, _)| range.start < end)
        .map(|&(.., index)| index)
        .min()
}

impl Default for BufferSet {
    fn default() -> Self {
        Self::new()
    }
}
