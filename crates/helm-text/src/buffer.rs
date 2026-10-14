//! `Buffer`: the rope-backed text buffer.

use std::fmt;
use std::ops::Range;

use ropey::Rope;

use crate::{Point, TextBuffer, TextError, Utf16Point};

/// An editable text buffer backed by a rope.
///
/// Cloning is cheap: clones share the rope's nodes until one of them is
/// edited. Transactions and undo live in [`BufferSet`](crate::BufferSet),
/// which owns its buffers; this type has only the plain edits.
#[derive(Clone, Debug, Default)]
pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    /// An empty buffer.
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// Inserts `text` at byte offset `byte`.
    ///
    /// Errors with [`TextError::ByteOutOfBounds`] or
    /// [`TextError::NotCharBoundary`] for an invalid offset, and leaves the
    /// buffer unchanged.
    pub fn insert(&mut self, byte: usize, text: &str) -> Result<(), TextError> {
        self.check_byte(byte)?;
        let char = self.rope.byte_to_char(byte);
        self.rope.insert(char, text);
        Ok(())
    }

    /// Deletes the byte range `range`.
    ///
    /// Errors the same way as [`TextBuffer::slice`] for an invalid range,
    /// and leaves the buffer unchanged.
    pub fn delete(&mut self, range: Range<usize>) -> Result<(), TextError> {
        self.check_range(&range)?;
        let start = self.rope.byte_to_char(range.start);
        let end = self.rope.byte_to_char(range.end);
        self.rope.remove(start..end);
        Ok(())
    }

    /// Checks that `byte` is at most the length and on a char boundary.
    fn check_byte(&self, byte: usize) -> Result<(), TextError> {
        let len = self.rope.len_bytes();
        if byte > len {
            return Err(TextError::ByteOutOfBounds { offset: byte, len });
        }
        self.check_char_boundary(byte)
    }

    /// Checks that in-bounds byte offset `byte` falls on a char boundary.
    fn check_char_boundary(&self, byte: usize) -> Result<(), TextError> {
        // A one-past-the-end offset gets the last chunk, where it is the
        // chunk's length and so a boundary.
        let (chunk, chunk_start, _, _) = self.rope.chunk_at_byte(byte);
        if chunk.is_char_boundary(byte - chunk_start) {
            Ok(())
        } else {
            Err(TextError::NotCharBoundary { offset: byte })
        }
    }

    /// Checks a byte range in the order [`TextBuffer::slice`] documents.
    fn check_range(&self, range: &Range<usize>) -> Result<(), TextError> {
        if range.start > range.end {
            return Err(TextError::InvalidRange {
                start: range.start,
                end: range.end,
            });
        }
        let len = self.rope.len_bytes();
        if range.end > len {
            return Err(TextError::ByteOutOfBounds {
                offset: range.end,
                len,
            });
        }
        self.check_char_boundary(range.start)?;
        self.check_char_boundary(range.end)
    }

    /// Checks that `line` exists.
    fn check_line(&self, line: usize) -> Result<(), TextError> {
        let len = self.rope.len_lines();
        if line < len {
            Ok(())
        } else {
            Err(TextError::LineOutOfBounds { line, len })
        }
    }

    /// The line containing valid byte offset `byte`.
    fn line_of(&self, byte: usize) -> usize {
        // ropey never counts a CR as a break on its own when an LF follows
        // it, so the position between them stays on the CR's line.
        self.rope.byte_to_line(byte)
    }

    /// The largest column of `line`, in units that `start_of` measures: the
    /// line's length on the last line, and one less than that (the position
    /// before its last byte, which is a one-unit LF or CR) on any other.
    fn max_column(&self, line: usize, start_of: impl Fn(usize) -> usize, len: usize) -> usize {
        let start = start_of(line);
        if line + 1 == self.rope.len_lines() {
            len - start
        } else {
            start_of(line + 1) - start - 1
        }
    }
}

impl From<&str> for Buffer {
    fn from(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }
}

/// Writes the whole text, so `to_string()` returns it.
impl fmt::Display for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.rope.chunks().try_for_each(|chunk| f.write_str(chunk))
    }
}

impl TextBuffer for Buffer {
    fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    fn len_utf16(&self) -> usize {
        self.rope.len_utf16_cu()
    }

    fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    fn line(&self, line: usize) -> Result<String, TextError> {
        self.check_line(line)?;
        Ok(self.rope.line(line).to_string())
    }

    fn slice(&self, range: Range<usize>) -> Result<String, TextError> {
        self.check_range(&range)?;
        Ok(self.rope.byte_slice(range).to_string())
    }

    fn byte_to_char(&self, byte: usize) -> Result<usize, TextError> {
        self.check_byte(byte)?;
        Ok(self.rope.byte_to_char(byte))
    }

    fn char_to_byte(&self, char: usize) -> Result<usize, TextError> {
        let len = self.rope.len_chars();
        if char > len {
            return Err(TextError::CharOutOfBounds { offset: char, len });
        }
        Ok(self.rope.char_to_byte(char))
    }

    fn byte_to_utf16(&self, byte: usize) -> Result<usize, TextError> {
        let char = self.byte_to_char(byte)?;
        Ok(self.rope.char_to_utf16_cu(char))
    }

    fn utf16_to_byte(&self, utf16: usize) -> Result<usize, TextError> {
        let len = self.rope.len_utf16_cu();
        if utf16 > len {
            return Err(TextError::Utf16OutOfBounds { offset: utf16, len });
        }
        // ropey maps an offset inside a surrogate pair to the char holding
        // it, so a round trip that lands elsewhere means it split a pair.
        let char = self.rope.utf16_cu_to_char(utf16);
        if self.rope.char_to_utf16_cu(char) != utf16 {
            return Err(TextError::InsideSurrogatePair { offset: utf16 });
        }
        Ok(self.rope.char_to_byte(char))
    }

    fn byte_to_line(&self, byte: usize) -> Result<usize, TextError> {
        self.check_byte(byte)?;
        Ok(self.line_of(byte))
    }

    fn line_to_byte(&self, line: usize) -> Result<usize, TextError> {
        self.check_line(line)?;
        Ok(self.rope.line_to_byte(line))
    }

    fn byte_to_point(&self, byte: usize) -> Result<Point, TextError> {
        self.check_byte(byte)?;
        let line = self.line_of(byte);
        Ok(Point::new(line, byte - self.rope.line_to_byte(line)))
    }

    fn point_to_byte(&self, point: Point) -> Result<usize, TextError> {
        let Point { line, column } = point;
        self.check_line(line)?;
        let max = self.max_column(line, |l| self.rope.line_to_byte(l), self.rope.len_bytes());
        if column > max {
            return Err(TextError::ColumnOutOfBounds { line, column });
        }
        // `column <= max` keeps the sum inside the text, so it can't overflow.
        let byte = self.rope.line_to_byte(line) + column;
        self.check_char_boundary(byte)?;
        Ok(byte)
    }

    fn byte_to_utf16_point(&self, byte: usize) -> Result<Utf16Point, TextError> {
        let utf16 = self.byte_to_utf16(byte)?;
        let line = self.line_of(byte);
        let start = self.rope.char_to_utf16_cu(self.rope.line_to_char(line));
        Ok(Utf16Point::new(line, utf16 - start))
    }

    fn utf16_point_to_byte(&self, point: Utf16Point) -> Result<usize, TextError> {
        let Utf16Point { line, column } = point;
        self.check_line(line)?;
        let start_of = |l| self.rope.char_to_utf16_cu(self.rope.line_to_char(l));
        let max = self.max_column(line, start_of, self.rope.len_utf16_cu());
        if column > max {
            return Err(TextError::ColumnOutOfBounds { line, column });
        }
        // `column <= max` keeps the sum inside the text, so it can't overflow.
        self.utf16_to_byte(start_of(line) + column)
    }
}
