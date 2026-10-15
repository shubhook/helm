//! `BufferSet`: buffers that share one undo history, so a transaction can
//! span several of them.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::mem;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    AmendError, Buffer, BufferId, Edit, HistoryError, Snapshot, TextBuffer, Transaction,
    TransactionError, TransactionId, Version,
};

/// Buffers that edit, undo and redo through transactions, including
/// transactions that touch several of them at once.
///
/// The set owns its buffers. [`get`](Self::get) lends them out read-only,
/// and the only way to change one is [`apply`](Self::apply),
/// [`amend`](Self::amend), [`undo`](Self::undo) or [`redo`](Self::redo). So
/// no buffer can change behind the history's back, and the rules below hold
/// by construction.
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

/// Checked edits per buffer index, as `(start, end, index in the
/// transaction)`. Sorted, that is the order the edits are placed in.
type Placed = BTreeMap<usize, BTreeSet<(usize, usize, usize)>>;

/// One buffer and its histories.
#[derive(Debug)]
struct Entry {
    buffer: Buffer,
    /// Transaction id numbers, the most recent on top (last).
    undo: Vec<u64>,
    /// Transaction id numbers, the most recently undone on top (last).
    redo: Vec<u64>,
    /// The buffer's version. Only the set writes it.
    version: u64,
    /// A copy of `version` that the buffer's snapshots read to tell whether
    /// they are stale. The set stores each new version into it.
    latest: Arc<AtomicU64>,
}

impl Entry {
    /// Adds one to the version, after a transaction changed the buffer.
    fn bump(&mut self) {
        self.version += 1;
        // Relaxed is enough: the counter publishes no data, since a snapshot
        // carries its own text. Coherence on this one atomic, plus whatever
        // happens-before edge the reader's channel or join gives, makes
        // `Snapshot::is_stale` see every version stored before its call.
        self.latest.store(self.version, Ordering::Relaxed);
    }
}

/// What one transaction did, kept to undo and redo it.
#[derive(Debug)]
struct Record {
    /// One change per buffer the transaction touched, lowest buffer first.
    changes: Vec<Change>,
    /// Whether the transaction is undone, so it sits in redo histories.
    undone: bool,
}

impl Record {
    /// The change to the buffer at `index`, if the transaction touched it.
    fn change(&self, index: usize) -> Option<&Change> {
        self.changes
            .binary_search_by_key(&index, |change| change.buffer)
            .ok()
            .map(|at| &self.changes[at])
    }

    /// [`change`](Self::change), mutably.
    fn change_mut(&mut self, index: usize) -> Option<&mut Change> {
        self.changes
            .binary_search_by_key(&index, |change| change.buffer)
            .ok()
            .map(|at| &mut self.changes[at])
    }
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

impl Change {
    /// The change to the buffer at `index` made of `splices`, each paired
    /// with the text it replaces. The splices are sorted, don't overlap and
    /// refer to the text before the change.
    fn new(index: usize, splices: impl IntoIterator<Item = (Splice, String)>) -> Self {
        let splices = splices.into_iter();
        let mut forward = Vec::with_capacity(splices.size_hint().0);
        let mut inverse = Vec::with_capacity(splices.size_hint().0);
        // Bytes added and removed by the splices placed so far, which move
        // where this splice's text lands in the new text.
        let (mut added, mut removed) = (0, 0);
        for (splice, old) in splices {
            let new_start = splice.range.start + added - removed;
            inverse.push(Splice {
                range: new_start..new_start + splice.text.len(),
                text: old,
            });
            added += splice.text.len();
            removed += splice.range.len();
            forward.push(splice);
        }
        Self {
            buffer: index,
            forward,
            inverse,
        }
    }

    /// Folds `edits` into the change, so it turns the text before it into
    /// the text after it with `edits` applied.
    ///
    /// `edits` are sorted the way `apply` places them, don't overlap, and
    /// refer to the text after the change, which `buffer` holds. The walk
    /// goes through the change's splices, by where their text sits in
    /// `buffer`, and `edits` together. A run of them that overlap or touch
    /// becomes one splice, so a run of typed characters stays one splice.
    /// Splices no edit reaches are kept as they are. The text the new
    /// splices replace comes from the old splices and, where the change left
    /// the text alone, from `buffer` inside the edits' ranges, so the work grows
    /// with the change and the edits, not with the buffer.
    fn compose(&mut self, buffer: &Buffer, edits: &[Splice]) {
        // Each old splice with its inverse, whose range is where the
        // splice's text sits in `buffer`.
        let mut olds = mem::take(&mut self.forward)
            .into_iter()
            .zip(mem::take(&mut self.inverse))
            .peekable();
        let mut edits = edits.iter().peekable();
        let mut composed = Vec::new();
        // Bytes added and removed by the old splices passed so far, which
        // map an offset in `buffer` back to the text before the change.
        let (mut added, mut removed) = (0, 0);

        loop {
            let start = match (olds.peek(), edits.peek()) {
                (None, None) => break,
                (Some((_, inverse)), Some(edit)) => inverse.range.start.min(edit.range.start),
                (Some((_, inverse)), None) => inverse.range.start,
                (None, Some(edit)) => edit.range.start,
            };
            let before_start = start + removed - added;

            // The run of old splices and edits that starts here: each one
            // starts at or before the end of the ones before it.
            let mut end = start;
            let mut run_olds = Vec::new();
            let mut run_edits = Vec::new();
            loop {
                if let Some(old) = olds.next_if(|(_, inverse)| inverse.range.start <= end) {
                    end = end.max(old.1.range.end);
                    added += old.0.text.len();
                    removed += old.0.range.len();
                    run_olds.push(old);
                } else if let Some(edit) = edits.next_if(|edit| edit.range.start <= end) {
                    end = end.max(edit.range.end);
                    run_edits.push(edit);
                } else {
                    break;
                }
            }

            if run_edits.is_empty() {
                composed.extend(
                    run_olds
                        .into_iter()
                        .map(|(forward, inverse)| (forward, inverse.text)),
                );
                continue;
            }

            // The run's text before the change: what the old splices
            // replaced, and between them, text the change left alone, which
            // lies inside an edit's range.
            let mut old_text = String::new();
            let mut at = start;
            for (_, inverse) in run_olds {
                read(buffer, at..inverse.range.start, &mut old_text);
                old_text.push_str(&inverse.text);
                at = inverse.range.end;
            }
            read(buffer, at..end, &mut old_text);

            // The run's text after the edits: their texts, and between
            // them, text in `buffer`, which lies inside an old splice's.
            let mut new_text = String::new();
            let mut at = start;
            for edit in run_edits {
                read(buffer, at..edit.range.start, &mut new_text);
                new_text.push_str(&edit.text);
                at = edit.range.end;
            }
            read(buffer, at..end, &mut new_text);

            let splice = Splice {
                range: before_start..end + removed - added,
                text: new_text,
            };
            composed.push((splice, old_text));
        }
        *self = Self::new(self.buffer, composed);
    }
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
            version: 0,
            latest: Arc::new(AtomicU64::new(0)),
        });
        id
    }

    /// The buffer with id `id`, or `None` if the set has no such buffer.
    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        self.index(id).map(|index| &self.entries[index].buffer)
    }

    /// The current version of buffer `id`, or `None` if the set has no such
    /// buffer.
    pub fn version(&self, id: BufferId) -> Option<Version> {
        self.index(id)
            .map(|index| Version(self.entries[index].version))
    }

    /// A snapshot of buffer `id`'s current text and version, or `None` if
    /// the set has no such buffer.
    ///
    /// Costs O(1) whatever the buffer's size: no copy of the text, no lock,
    /// and no change to the buffer, its version or its history.
    pub fn snapshot(&self, id: BufferId) -> Option<Snapshot> {
        self.index(id).map(|index| {
            let entry = &self.entries[index];
            Snapshot::new(
                entry.buffer.clone(),
                id,
                Version(entry.version),
                Arc::clone(&entry.latest),
            )
        })
    }

    /// Checks every edit of `transaction`, then applies them all, records
    /// the transaction in the undo history of every buffer it touches, and
    /// returns its id.
    ///
    /// Errors with the first problem found, as the [type docs](Self)
    /// describe, and then changes nothing.
    pub fn apply(&mut self, transaction: Transaction) -> Result<TransactionId, TransactionError> {
        let edits = transaction.into_edits();
        let (mut olds, placed) = self.check(&edits, |_, _, _| Ok(()), |error| error)?;

        let touched: Vec<usize> = placed.keys().copied().collect();
        self.drop_redo(&touched);

        let mut texts: Vec<String> = edits.into_iter().map(|edit| edit.text).collect();
        let id = self.next_transaction;
        self.next_transaction += 1;
        let mut changes = Vec::with_capacity(placed.len());
        for (index, mine) in placed {
            let change = Change::new(
                index,
                mine.into_iter().map(|(start, end, i)| {
                    let splice = Splice {
                        range: start..end,
                        text: mem::take(&mut texts[i]),
                    };
                    (splice, mem::take(&mut olds[i]))
                }),
            );
            let entry = &mut self.entries[index];
            splice_all(&mut entry.buffer, &change.forward);
            entry.bump();
            entry.undo.push(id);
            changes.push(change);
        }
        self.transactions.insert(
            id,
            Record {
                changes,
                undone: false,
            },
        );
        Ok(TransactionId(id))
    }

    /// Folds `transaction`'s edits into the applied transaction `id`, so one
    /// undo reverts both and one redo brings both back.
    ///
    /// This is how a run of keystrokes becomes one undo step (ED-2): the
    /// editor applies the first keystroke as a transaction and amends it
    /// with each keystroke after that. Every keystroke changes the buffer at
    /// once, so language servers and agents, which read the buffer, see it.
    ///
    /// # When amending is allowed
    ///
    /// `id` must be applied, not undone, and must be on top of the undo
    /// history of every buffer `transaction` touches. `transaction` may only
    /// touch buffers `id` touched, though it needn't touch all of them. A
    /// later transaction on top of a buffer that `transaction` doesn't touch
    /// doesn't stop the amend. It still blocks undoing `id`, as the
    /// [type docs](Self) describe.
    ///
    /// # Offsets
    ///
    /// Every edit's range is in bytes of its buffer's text after `id`, which
    /// is the buffer's current text, since `id` is on top of its undo
    /// history. Within `transaction`, ranges refer to that text, not to the
    /// text after the edits listed before them, and edits are placed and
    /// must not overlap exactly as for [`apply`](Self::apply).
    ///
    /// # Checks
    ///
    /// `amend` checks everything before it changes anything, in this order,
    /// and returns the first problem it finds:
    ///
    /// 1. `id` must be in some history of the set
    ///    ([`AmendError::UnknownTransaction`]). An id from another set, or
    ///    of a transaction a later `apply` or `amend` dropped from the redo
    ///    histories, fails here.
    /// 2. `id` must not be undone ([`AmendError::Undone`]).
    /// 3. `transaction` must have edits
    ///    ([`AmendError::Transaction`] with [`TransactionError::Empty`]).
    /// 4. For each edit in order: its buffer must be in the set
    ///    ([`TransactionError::UnknownBuffer`]); then `id` must have touched
    ///    it ([`AmendError::Untouched`]); then `id` must be on top of its
    ///    undo history ([`AmendError::NotOnTop`]); then its range must be
    ///    valid in the buffer's current text, checked the way
    ///    [`TextBuffer::slice`] documents
    ///    ([`TransactionError::InvalidRange`]); then it must not overlap an
    ///    earlier edit on the same buffer ([`TransactionError::Overlap`],
    ///    naming the earliest such edit). The [`TransactionError`]s come
    ///    wrapped in [`AmendError::Transaction`], with the same edit indexes
    ///    `apply` would give.
    ///
    /// If any check fails, no buffer, history or version changes.
    ///
    /// # What an amend does
    ///
    /// - Each buffer `transaction` touches gets its edits, exactly as if
    ///   `apply` had applied them.
    /// - `id` keeps its id and its place in every history.
    ///   [`undo`](Self::undo) and [`redo`](Self::redo) return `id` as
    ///   before, and `amend` uses up no new id.
    /// - Undoing `id` from any buffer it touched restores every one of those
    ///   buffers, byte for byte, to its text from before `id` was first
    ///   applied. Redoing it restores each one to its text after the last
    ///   amend. Buffers `id` touched that no amend touched get back the text
    ///   `id` alone gave them.
    /// - The redo histories of the buffers `transaction` touches are
    ///   dropped, with the same knock-on drops as for `apply`.
    /// - The version of each buffer `transaction` touches goes up by exactly
    ///   one, however many edits it had there, so snapshots taken before go
    ///   stale. Other buffers keep their versions, including ones `id`
    ///   touched that this amend didn't. A later undo or redo of `id` adds
    ///   one to every buffer `id` touched, as for any transaction.
    ///
    /// An amend whose edits leave the text as it was, such as an empty
    /// insert, still counts: it drops redo and bumps versions.
    ///
    /// # Cost
    ///
    /// An amend costs about what applying `transaction` costs, plus work in
    /// proportion to the changes `id` already holds for the buffers it
    /// touches. It doesn't grow with the buffer's length, apart from the
    /// text the edits replace, and it doesn't keep a list of amends to
    /// replay: after any number of amends, undoing `id` is one change per
    /// buffer, so a run of typed characters undoes like one insert.
    ///
    /// [`TextBuffer::slice`]: crate::TextBuffer::slice
    pub fn amend(&mut self, id: TransactionId, transaction: Transaction) -> Result<(), AmendError> {
        let Some(record) = self.transactions.get(&id.0) else {
            return Err(AmendError::UnknownTransaction { transaction: id });
        };
        if record.undone {
            return Err(AmendError::Undone { transaction: id });
        }

        let edits = transaction.into_edits();
        let (_, placed) = self.check(
            &edits,
            |i, index, buffer| {
                if record.change(index).is_none() {
                    Err(AmendError::Untouched { edit: i, buffer })
                } else if self.entries[index].undo.last() != Some(&id.0) {
                    Err(AmendError::NotOnTop { edit: i, buffer })
                } else {
                    Ok(())
                }
            },
            AmendError::Transaction,
        )?;

        let touched: Vec<usize> = placed.keys().copied().collect();
        self.drop_redo(&touched);

        let mut texts: Vec<String> = edits.into_iter().map(|edit| edit.text).collect();
        let Self {
            entries,
            transactions,
            ..
        } = self;
        let Some(record) = transactions.get_mut(&id.0) else {
            unreachable!("`id` is applied, so dropping redo kept it");
        };
        for (index, mine) in placed {
            let splices: Vec<Splice> = mine
                .into_iter()
                .map(|(start, end, i)| Splice {
                    range: start..end,
                    text: mem::take(&mut texts[i]),
                })
                .collect();
            let Some(change) = record.change_mut(index) else {
                unreachable!("every edit's buffer passed the `Untouched` check");
            };
            let entry = &mut entries[index];
            change.compose(&entry.buffer, &splices);
            splice_all(&mut entry.buffer, &splices);
            entry.bump();
        }
        Ok(())
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

    /// Runs `apply`'s checks on `edits`, with `extra` run on each edit
    /// right after its buffer is found, and returns the text each edit
    /// replaces and, per buffer, the edits as `(start, end, index)`. Sorted,
    /// that is the order the edits are placed in. `wrap` turns a failed
    /// `apply` check into the caller's error.
    fn check<E>(
        &self,
        edits: &[Edit],
        mut extra: impl FnMut(usize, usize, BufferId) -> Result<(), E>,
        wrap: impl Fn(TransactionError) -> E,
    ) -> Result<(Vec<String>, Placed), E> {
        if edits.is_empty() {
            return Err(wrap(TransactionError::Empty));
        }
        let mut olds = Vec::with_capacity(edits.len());
        let mut placed = Placed::new();
        for (i, edit) in edits.iter().enumerate() {
            let index = self.index(edit.buffer).ok_or_else(|| {
                wrap(TransactionError::UnknownBuffer {
                    edit: i,
                    buffer: edit.buffer,
                })
            })?;
            extra(i, index, edit.buffer)?;
            let old = self.entries[index]
                .buffer
                .slice(edit.range.clone())
                .map_err(|error| wrap(TransactionError::InvalidRange { edit: i, error }))?;
            let mine = placed.entry(index).or_default();
            if let Some(other) = earliest_overlap(mine, &edit.range) {
                return Err(wrap(TransactionError::Overlap { edit: i, other }));
            }
            mine.insert((edit.range.start, edit.range.end, i));
            olds.push(old);
        }
        Ok((olds, placed))
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
        let Some(record) = self.transactions.get_mut(&top) else {
            unreachable!("every id in a history has a record");
        };

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
            entry.bump();
        }
        record.undone = matches!(direction, Direction::Undo);
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

/// Appends the text in `range` of `buffer` to `into`. `range` comes from
/// splices and edits already checked against `buffer`, so it is valid.
fn read(buffer: &Buffer, range: Range<usize>, into: &mut String) {
    if !range.is_empty() {
        into.push_str(&buffer.slice(range).expect("a checked range"));
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
