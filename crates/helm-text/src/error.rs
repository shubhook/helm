//! The error every fallible `helm-text` call returns.

use std::fmt;

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
