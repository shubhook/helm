//! `Buffer`: the rope-backed text buffer.

// The bodies below are stubs until T-101 is implemented. `expect` rather than
// `allow`, so the build fails once the stubs are gone and this line stays.
#![expect(
    unused_variables,
    dead_code,
    reason = "T-101 interface stubs; the implementation deletes this"
)]

use std::fmt;
use std::ops::Range;

use ropey::Rope;

use crate::{Point, TextBuffer, TextError, Utf16Point};

/// An editable text buffer backed by a rope.
///
/// Cloning is cheap: clones share the rope's nodes until one of them is
/// edited. Transactions and undo come in T-102; this type has only the plain
/// edits.
#[derive(Clone, Debug, Default)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    /// An empty buffer.
    pub fn new() -> Self {
        todo!()
    }

    /// Inserts `text` at byte offset `byte`.
    ///
    /// Errors with [`TextError::ByteOutOfBounds`] or
    /// [`TextError::NotCharBoundary`] for an invalid offset, and leaves the
    /// buffer unchanged.
    pub fn insert(&mut self, byte: usize, text: &str) -> Result<(), TextError> {
        todo!()
    }

    /// Deletes the byte range `range`.
    ///
    /// Errors the same way as [`TextBuffer::slice`] for an invalid range,
    /// and leaves the buffer unchanged.
    pub fn delete(&mut self, range: Range<usize>) -> Result<(), TextError> {
        todo!()
    }
}

impl From<&str> for Buffer {
    fn from(text: &str) -> Self {
        todo!()
    }
}

/// Writes the whole text, so `to_string()` returns it.
impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl TextBuffer for Buffer {
    fn len_bytes(&self) -> usize {
        todo!()
    }

    fn len_chars(&self) -> usize {
        todo!()
    }

    fn len_utf16(&self) -> usize {
        todo!()
    }

    fn len_lines(&self) -> usize {
        todo!()
    }

    fn line(&self, line: usize) -> Result<String, TextError> {
        todo!()
    }

    fn slice(&self, range: Range<usize>) -> Result<String, TextError> {
        todo!()
    }

    fn byte_to_char(&self, byte: usize) -> Result<usize, TextError> {
        todo!()
    }

    fn char_to_byte(&self, char: usize) -> Result<usize, TextError> {
        todo!()
    }

    fn byte_to_utf16(&self, byte: usize) -> Result<usize, TextError> {
        todo!()
    }

    fn utf16_to_byte(&self, utf16: usize) -> Result<usize, TextError> {
        todo!()
    }

    fn byte_to_line(&self, byte: usize) -> Result<usize, TextError> {
        todo!()
    }

    fn line_to_byte(&self, line: usize) -> Result<usize, TextError> {
        todo!()
    }

    fn byte_to_point(&self, byte: usize) -> Result<Point, TextError> {
        todo!()
    }

    fn point_to_byte(&self, point: Point) -> Result<usize, TextError> {
        todo!()
    }

    fn byte_to_utf16_point(&self, byte: usize) -> Result<Utf16Point, TextError> {
        todo!()
    }

    fn utf16_point_to_byte(&self, point: Utf16Point) -> Result<usize, TextError> {
        todo!()
    }
}
