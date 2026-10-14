//! Edits, transactions and the ids that name buffers and transactions.

use std::ops::Range;

/// Names one buffer in a [`BufferSet`](crate::BufferSet).
///
/// [`BufferSet::add`](crate::BufferSet::add) hands these out, in increasing
/// order. An id names a buffer only in the set that made it. Using it with
/// another set is a logic error: the call may name a different buffer or
/// none, but it never panics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferId(pub(crate) u64);

/// Names one applied transaction in a [`BufferSet`](crate::BufferSet).
///
/// [`BufferSet::apply`](crate::BufferSet::apply) hands these out, in
/// increasing order, and [`BufferSet::undo`](crate::BufferSet::undo) and
/// [`BufferSet::redo`](crate::BufferSet::redo) return the id of the
/// transaction they reverted or re-applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionId(pub(crate) u64);

/// One edit: replace the byte range `range` of buffer `buffer` with `text`.
///
/// An insert is an edit with an empty range; a delete is an edit with empty
/// text. `range` is in bytes of the buffer's text before the transaction
/// that holds this edit, not after the edits listed before it. See
/// [`BufferSet`](crate::BufferSet) for the full rules.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Edit {
    /// The buffer to edit.
    pub buffer: BufferId,
    /// The byte range to replace.
    pub range: Range<usize>,
    /// The text that replaces it.
    pub text: String,
}

impl Edit {
    /// Inserts `text` at byte offset `byte`: the range `byte..byte`.
    pub fn insert(buffer: BufferId, byte: usize, text: impl Into<String>) -> Self {
        Self::replace(buffer, byte..byte, text)
    }

    /// Deletes the byte range `range`: replaces it with empty text.
    pub fn delete(buffer: BufferId, range: Range<usize>) -> Self {
        Self::replace(buffer, range, String::new())
    }

    /// Replaces the byte range `range` with `text`.
    pub fn replace(buffer: BufferId, range: Range<usize>, text: impl Into<String>) -> Self {
        Self {
            buffer,
            range,
            text: text.into(),
        }
    }
}

/// A list of edits that apply, undo and redo as one unit, across one buffer
/// or several.
///
/// Edits keep the order they were added in. That order matters only for
/// edits at the same offset of the same buffer; see
/// [`BufferSet`](crate::BufferSet).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transaction {
    edits: Vec<Edit>,
}

impl Transaction {
    /// A transaction with no edits.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `edit` after the edits already in the transaction.
    pub fn push(&mut self, edit: Edit) {
        self.edits.push(edit);
    }

    /// The edits, in the order they were added. Errors from
    /// [`BufferSet::apply`](crate::BufferSet::apply) name an edit by its
    /// index in this slice.
    pub fn edits(&self) -> &[Edit] {
        &self.edits
    }

    /// The edits, in the order they were added, by value.
    pub(crate) fn into_edits(self) -> Vec<Edit> {
        self.edits
    }
}

impl From<Vec<Edit>> for Transaction {
    fn from(edits: Vec<Edit>) -> Self {
        Self { edits }
    }
}

impl FromIterator<Edit> for Transaction {
    fn from_iter<I: IntoIterator<Item = Edit>>(iter: I) -> Self {
        Self {
            edits: iter.into_iter().collect(),
        }
    }
}
