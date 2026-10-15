//! The errors `helm-text` calls return.

use std::fmt;

use crate::{BufferId, TransactionId};

/// Why an offset, position or range was rejected.
///
/// Offsets in these variants are in the unit the variant names, counted from
/// the start of the text. For a [`Point`](crate::Point) or
/// [`Utf16Point`](crate::Utf16Point) that names a valid line and column but
/// splits a character, `offset` is the absolute offset the point resolved to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextError {
    /// A byte offset is past the end of the text.
    ByteOutOfBounds {
        /// The offset that was asked for.
        offset: usize,
        /// The text's length in bytes.
        len: usize,
    },
    /// A char offset is past the end of the text.
    CharOutOfBounds {
        /// The offset that was asked for.
        offset: usize,
        /// The text's length in chars.
        len: usize,
    },
    /// A UTF-16 offset is past the end of the text.
    Utf16OutOfBounds {
        /// The offset that was asked for.
        offset: usize,
        /// The text's length in UTF-16 code units.
        len: usize,
    },
    /// A line index is past the last line.
    LineOutOfBounds {
        /// The line that was asked for.
        line: usize,
        /// The number of lines.
        len: usize,
    },
    /// A column is past the end of its line. See the [crate docs](crate)
    /// for which columns a line has.
    ColumnOutOfBounds {
        /// The point's line.
        line: usize,
        /// The point's column, in the point's unit.
        column: usize,
    },
    /// A byte offset falls inside a multi-byte UTF-8 character.
    NotCharBoundary {
        /// The byte offset.
        offset: usize,
    },
    /// A UTF-16 offset falls between the two halves of a surrogate pair.
    InsideSurrogatePair {
        /// The UTF-16 offset.
        offset: usize,
    },
    /// A byte range starts after it ends.
    InvalidRange {
        /// The range's start.
        start: usize,
        /// The range's end.
        end: usize,
    },
}

impl fmt::Display for TextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ByteOutOfBounds { offset, len } => {
                write!(f, "byte offset {offset} is past the end ({len} bytes)")
            }
            Self::CharOutOfBounds { offset, len } => {
                write!(f, "char offset {offset} is past the end ({len} chars)")
            }
            Self::Utf16OutOfBounds { offset, len } => {
                write!(
                    f,
                    "UTF-16 offset {offset} is past the end ({len} code units)"
                )
            }
            Self::LineOutOfBounds { line, len } => {
                write!(f, "line {line} is past the last line ({len} lines)")
            }
            Self::ColumnOutOfBounds { line, column } => {
                write!(f, "column {column} is past the end of line {line}")
            }
            Self::NotCharBoundary { offset } => {
                write!(f, "byte offset {offset} is inside a character")
            }
            Self::InsideSurrogatePair { offset } => {
                write!(f, "UTF-16 offset {offset} is inside a surrogate pair")
            }
            Self::InvalidRange { start, end } => {
                write!(f, "range {start}..{end} starts after it ends")
            }
        }
    }
}

impl std::error::Error for TextError {}

/// Why [`BufferSet::apply`](crate::BufferSet::apply) rejected a transaction.
///
/// `edit` and `other` are indexes into
/// [`Transaction::edits`](crate::Transaction::edits). A rejected transaction
/// changes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransactionError {
    /// The transaction has no edits.
    Empty,
    /// An edit names a buffer that isn't in the set.
    UnknownBuffer {
        /// The edit's index.
        edit: usize,
        /// The buffer it names.
        buffer: BufferId,
    },
    /// An edit's range isn't valid in its buffer's text before the
    /// transaction.
    InvalidRange {
        /// The edit's index.
        edit: usize,
        /// What [`TextBuffer::slice`](crate::TextBuffer::slice) returns for
        /// the same range of the same text.
        error: TextError,
    },
    /// An edit overlaps an earlier edit on the same buffer.
    Overlap {
        /// The later edit's index.
        edit: usize,
        /// The index of the earliest edit it overlaps.
        other: usize,
    },
}

impl fmt::Display for TransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Empty => write!(f, "the transaction has no edits"),
            Self::UnknownBuffer { edit, buffer } => {
                write!(f, "edit {edit} names {buffer:?}, which isn't in the set")
            }
            Self::InvalidRange { edit, error } => write!(f, "edit {edit}: {error}"),
            Self::Overlap { edit, other } => write!(f, "edit {edit} overlaps edit {other}"),
        }
    }
}

impl std::error::Error for TransactionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidRange { error, .. } => Some(error),
            _ => None,
        }
    }
}

/// Why [`BufferSet::undo`](crate::BufferSet::undo) or
/// [`BufferSet::redo`](crate::BufferSet::redo) did nothing. Either way, no
/// buffer and no history changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HistoryError {
    /// The buffer isn't in the set.
    UnknownBuffer {
        /// The buffer that was asked for.
        buffer: BufferId,
    },
    /// The transaction touched another buffer whose history has a later
    /// transaction on top, so undoing or redoing it would split it.
    Blocked {
        /// The transaction that couldn't be undone or redone.
        transaction: TransactionId,
        /// The touched buffer, lowest id first, whose history has a later
        /// transaction on top. Undo or redo this buffer first.
        buffer: BufferId,
    },
}

impl fmt::Display for HistoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownBuffer { buffer } => write!(f, "{buffer:?} isn't in the set"),
            Self::Blocked {
                transaction,
                buffer,
            } => write!(
                f,
                "{transaction:?} can't be undone or redone in part, and {buffer:?} has a later transaction on top"
            ),
        }
    }
}

impl std::error::Error for HistoryError {}

/// Why [`BufferSet::amend`](crate::BufferSet::amend) rejected an amendment.
///
/// `edit` is an index into [`Transaction::edits`](crate::Transaction::edits)
/// of the transaction passed to `amend`. A rejected amendment changes
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AmendError {
    /// The transaction isn't in any undo or redo history of the set. It was
    /// never applied in this set, or a later transaction dropped it from the
    /// redo histories.
    UnknownTransaction {
        /// The transaction that was asked for.
        transaction: TransactionId,
    },
    /// The transaction was undone and hasn't been redone, so it sits in the
    /// redo histories.
    Undone {
        /// The transaction that was asked for.
        transaction: TransactionId,
    },
    /// An edit names a buffer in the set that the transaction didn't touch.
    Untouched {
        /// The edit's index.
        edit: usize,
        /// The buffer it names.
        buffer: BufferId,
    },
    /// An edit names a buffer whose undo history has a later transaction on
    /// top of the one being amended.
    NotOnTop {
        /// The edit's index.
        edit: usize,
        /// The buffer it names.
        buffer: BufferId,
    },
    /// The edits fail one of the checks
    /// [`BufferSet::apply`](crate::BufferSet::apply) makes, run against the
    /// text after the transaction being amended.
    Transaction(TransactionError),
}

impl fmt::Display for AmendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownTransaction { transaction } => {
                write!(f, "{transaction:?} isn't in any history of the set")
            }
            Self::Undone { transaction } => {
                write!(f, "{transaction:?} was undone, so it can't be amended")
            }
            Self::Untouched { edit, buffer } => write!(
                f,
                "edit {edit} names {buffer:?}, which the transaction didn't touch"
            ),
            Self::NotOnTop { edit, buffer } => write!(
                f,
                "edit {edit} names {buffer:?}, which has a later transaction on top"
            ),
            Self::Transaction(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for AmendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Transaction(error) => Some(error),
            _ => None,
        }
    }
}
