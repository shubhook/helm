//! `BufferSet`: buffers that share one undo history, so a transaction can
//! span several of them.

// The bodies below are stubs until T-102 is implemented. `expect` rather than
// `allow`, so the build fails once the stubs are gone and this line stays.
#![expect(
    unused_variables,
    dead_code,
    reason = "T-102 interface stubs; the implementation deletes this"
)]

use crate::{Buffer, BufferId, HistoryError, Transaction, TransactionError, TransactionId};

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
    // Private: the implementation may replace these fields freely.
    buffers: Vec<Buffer>,
}

impl BufferSet {
    /// An empty set.
    pub fn new() -> Self {
        todo!()
    }

    /// Adds `buffer` to the set with an empty undo and redo history, and
    /// returns its id. Its text when added is the text that undoing every
    /// transaction returns it to.
    pub fn add(&mut self, buffer: Buffer) -> BufferId {
        todo!()
    }

    /// The buffer with id `id`, or `None` if the set has no such buffer.
    pub fn get(&self, id: BufferId) -> Option<&Buffer> {
        todo!()
    }

    /// Checks every edit of `transaction`, then applies them all, records
    /// the transaction in the undo history of every buffer it touches, and
    /// returns its id.
    ///
    /// Errors with the first problem found, as the [type docs](Self)
    /// describe, and then changes nothing.
    pub fn apply(&mut self, transaction: Transaction) -> Result<TransactionId, TransactionError> {
        todo!()
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
        todo!()
    }

    /// Re-applies the transaction on top of `buffer`'s redo history in
    /// every buffer it touched, and returns its id. Returns `Ok(None)` if
    /// `buffer`'s redo history is empty.
    ///
    /// Errors the same way as [`undo`](Self::undo), with the redo histories
    /// in place of the undo histories. On an error, nothing changes.
    pub fn redo(&mut self, buffer: BufferId) -> Result<Option<TransactionId>, HistoryError> {
        todo!()
    }
}

impl Default for BufferSet {
    fn default() -> Self {
        Self::new()
    }
}
