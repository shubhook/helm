//! Text storage: a rope behind a Helm trait, with transactions, undo, snapshots and UTF-16 offsets.
//!
//! [`TextBuffer`] is the read-only view every other crate codes against.
//! [`Buffer`] implements it on top of a rope and adds plain inserts and
//! deletes. The rope crate is a private detail: no type from it appears in
//! this crate's public interface, so swapping it touches only `helm-text`.
//!
//! # Offsets
//!
//! Every offset is a `usize` counted from the start of the text, in one of
//! three units:
//!
//! - bytes of UTF-8, the unit edits and slices use;
//! - chars, meaning Unicode scalar values;
//! - UTF-16 code units, the unit LSP positions use. A character outside the
//!   Basic Multilingual Plane, such as 😀, is two code units (a surrogate
//!   pair).
//!
//! A byte offset is valid when it is at most the length and falls on a char
//! boundary. A UTF-16 offset is valid when it is at most the length and does
//! not fall between the two halves of a surrogate pair. Every conversion
//! checks its input and returns a [`TextError`] instead of panicking.
//!
//! # Lines
//!
//! A line break is LF (`\n`), CRLF (`\r\n`) or a lone CR (`\r`), and nothing
//! else. CRLF is one line break. Unicode breaks such as U+2028, U+0085 (NEL),
//! VT and FF are ordinary characters. This matches what LSP counts.
//!
//! Line `n` runs from the start of the text, or the end of the `n`th line
//! break, up to and including its own line break. A text with `k` line breaks
//! has `k + 1` lines, so the empty text has one empty line and a text ending
//! in a line break has an empty last line.
//!
//! A column counts from the start of its line, in bytes for [`Point`] and in
//! UTF-16 code units for [`Utf16Point`]. A column is valid when the offset it
//! names is still inside its line: at most the line's length on the last
//! line, and before the line's last byte otherwise. That makes the position
//! between the CR and LF of a CRLF a valid column (it is a valid byte
//! offset, and editing there must work), while the position after the line
//! break belongs to the next line as column 0.
//!
//! # Transactions and undo
//!
//! A [`BufferSet`] owns buffers and changes them only through a
//! [`Transaction`]: a list of [`Edit`]s, each replacing a byte range of one
//! buffer, that applies, undoes and redoes as one unit. One transaction can
//! touch several buffers, so an LSP rename or an agent's change across many
//! files undoes in one step. Edit ranges refer to the text before the
//! transaction. Each buffer has its own undo and redo history, and a
//! transaction that touched several buffers is undone in all of them or in
//! none. [`BufferSet`] documents the full rules.
//!
//! # Line endings and BOM
//!
//! [`FileFormat::decode`] turns a file's bytes into its text and a
//! [`FileFormat`]: whether the file starts with a UTF-8 BOM, and how many LF,
//! CRLF and lone CR line breaks it has. The text keeps every line break as
//! it is in the file, so [`FileFormat::encode`] writes back the same bytes,
//! even for a file with mixed line breaks. [`FileFormat::newline`] is the
//! line break for a new line the user types, LF for a new file. Bytes that
//! aren't UTF-8, including UTF-16, return a [`DecodeError`]. Reading and
//! writing files is `helm-project`'s job.

mod buffer;
mod buffer_set;
mod encoding;
mod error;
mod snapshot;
mod transaction;

pub use buffer::Buffer;
pub use buffer_set::BufferSet;
pub use encoding::{
    ByteOrder, DecodeError, Decoded, FileFormat, LineEnding, LineEndingStyle, LineEndings,
    UTF16_SAMPLE_LEN,
};
pub use error::{HistoryError, TextError, TransactionError};
pub use snapshot::{Snapshot, Version};
pub use transaction::{BufferId, Edit, Transaction, TransactionId};

/// A position as a line and a column in bytes from the start of that line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    /// Zero-based line index.
    pub line: usize,
    /// Bytes from the start of the line.
    pub column: usize,
}

impl Point {
    /// Builds a point from a line index and a byte column.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A position as a line and a column in UTF-16 code units from the start of
/// that line. This is an LSP `Position` in Helm's types.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Utf16Point {
    /// Zero-based line index.
    pub line: usize,
    /// UTF-16 code units from the start of the line.
    pub column: usize,
}

impl Utf16Point {
    /// Builds a point from a line index and a UTF-16 column.
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// Read-only access to a text buffer: lengths, reading text out, and
/// conversions between byte, char, line and UTF-16 offsets.
///
/// See the [crate docs](crate) for what counts as a valid offset, a line and
/// a column. Every method that takes an offset returns an error for an
/// invalid one and never panics. Unless its docs say otherwise, a method
/// that takes a byte offset errors with [`TextError::ByteOutOfBounds`] if
/// the offset is past the end and [`TextError::NotCharBoundary`] if it
/// splits a character.
pub trait TextBuffer {
    /// Length of the text in bytes of UTF-8.
    fn len_bytes(&self) -> usize;

    /// Length of the text in chars (Unicode scalar values).
    fn len_chars(&self) -> usize;

    /// Length of the text in UTF-16 code units.
    fn len_utf16(&self) -> usize;

    /// Number of lines: one more than the number of line breaks. Never zero.
    fn len_lines(&self) -> usize;

    /// The text of line `line`, including its line break if it has one.
    ///
    /// Errors with [`TextError::LineOutOfBounds`] if `line >= len_lines()`.
    fn line(&self, line: usize) -> Result<String, TextError>;

    /// The text in the byte range `range`.
    ///
    /// Errors with [`TextError::InvalidRange`] if `range.start > range.end`,
    /// then [`TextError::ByteOutOfBounds`] if `range.end` is past the end,
    /// then [`TextError::NotCharBoundary`] if either end splits a character.
    fn slice(&self, range: std::ops::Range<usize>) -> Result<String, TextError>;

    /// The char offset of byte offset `byte`.
    fn byte_to_char(&self, byte: usize) -> Result<usize, TextError>;

    /// The byte offset of char offset `char`.
    ///
    /// Errors with [`TextError::CharOutOfBounds`] if `char > len_chars()`.
    fn char_to_byte(&self, char: usize) -> Result<usize, TextError>;

    /// The UTF-16 offset of byte offset `byte`.
    fn byte_to_utf16(&self, byte: usize) -> Result<usize, TextError>;

    /// The byte offset of UTF-16 offset `utf16`.
    ///
    /// Errors with [`TextError::Utf16OutOfBounds`] if `utf16 > len_utf16()`,
    /// or [`TextError::InsideSurrogatePair`] if it falls between the two
    /// halves of a surrogate pair.
    fn utf16_to_byte(&self, utf16: usize) -> Result<usize, TextError>;

    /// The index of the line that contains byte offset `byte`. A byte offset
    /// at the end of the text belongs to the last line.
    fn byte_to_line(&self, byte: usize) -> Result<usize, TextError>;

    /// The byte offset where line `line` starts.
    ///
    /// Errors with [`TextError::LineOutOfBounds`] if `line >= len_lines()`.
    fn line_to_byte(&self, line: usize) -> Result<usize, TextError>;

    /// The line and byte column of byte offset `byte`.
    fn byte_to_point(&self, byte: usize) -> Result<Point, TextError>;

    /// The byte offset of a line and byte column.
    ///
    /// Errors with [`TextError::LineOutOfBounds`] for a line past the last,
    /// [`TextError::ColumnOutOfBounds`] for a column outside its line, or
    /// [`TextError::NotCharBoundary`] for a column that splits a character.
    fn point_to_byte(&self, point: Point) -> Result<usize, TextError>;

    /// The line and UTF-16 column of byte offset `byte`.
    fn byte_to_utf16_point(&self, byte: usize) -> Result<Utf16Point, TextError>;

    /// The byte offset of a line and UTF-16 column. This is how an LSP
    /// position becomes a byte offset.
    ///
    /// Errors with [`TextError::LineOutOfBounds`] for a line past the last,
    /// [`TextError::ColumnOutOfBounds`] for a column outside its line, or
    /// [`TextError::InsideSurrogatePair`] for a column between the two
    /// halves of a surrogate pair.
    fn utf16_point_to_byte(&self, point: Utf16Point) -> Result<usize, TextError>;
}
