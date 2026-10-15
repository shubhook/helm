//! The errors `Editor` commands return.

use std::fmt;

use helm_text::{BufferId, HistoryError, TransactionId};

/// Why an [`Editor`](crate::Editor) command did nothing. On an error, the
/// buffer, its history and the editor's selections are unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorError {
    /// The editor's buffer isn't in the buffer set it was given.
    UnknownBuffer {
        /// The editor's buffer.
        buffer: BufferId,
    },
    /// Undo or redo is blocked by a later transaction in another buffer,
    /// as [`HistoryError::Blocked`] describes.
    Blocked {
        /// The transaction that couldn't be undone or redone.
        transaction: TransactionId,
        /// The buffer whose history has a later transaction on top.
        buffer: BufferId,
    },
}

impl From<HistoryError> for EditorError {
    fn from(error: HistoryError) -> Self {
        match error {
            HistoryError::UnknownBuffer { buffer } => Self::UnknownBuffer { buffer },
            HistoryError::Blocked {
                transaction,
                buffer,
            } => Self::Blocked {
                transaction,
                buffer,
            },
        }
    }
}

impl fmt::Display for EditorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::UnknownBuffer { buffer } => {
                write!(f, "buffer {buffer:?} is not in the set")
            }
            Self::Blocked {
                transaction,
                buffer,
            } => write!(
                f,
                "transaction {transaction:?} is blocked by a later edit to buffer {buffer:?}"
            ),
        }
    }
}

impl std::error::Error for EditorError {}
