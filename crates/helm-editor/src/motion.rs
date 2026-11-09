//! Motions: where a cursor goes from its head. [`Editor::move_cursors`],
//! [`Editor::extend_selections`] and [`Editor::delete_to`] take one, and
//! the cursor commands in [`cursor_commands`](crate::cursor_commands) are
//! one motion each.
//!
//! [`Editor::move_cursors`]: crate::Editor::move_cursors
//! [`Editor::extend_selections`]: crate::Editor::extend_selections
//! [`Editor::delete_to`]: crate::Editor::delete_to

use helm_text::{Buffer, TextBuffer};

use crate::editor::is_word;

/// Where a cursor moves from its head.
///
/// Every motion lands on a char boundary of the buffer's text, never
/// between the CR and LF of a CRLF. A line break is LF, CRLF or a lone CR,
/// as `helm-text` defines it, and CRLF always counts as one character.
/// Columns are in chars, not display columns: tab width and the width of
/// wide characters like 中 are `helm-ui`'s job.
///
/// # Word classes
///
/// [`WordLeft`](Self::WordLeft) and [`WordRight`](Self::WordRight) sort
/// each char into one of three classes, plus line breaks:
///
/// - **Word:** `char::is_alphanumeric`, or `_`. The same rule as
///   [`Editor::add_next_match`](crate::Editor::add_next_match).
/// - **Blank:** `char::is_whitespace` and not part of a line break, such
///   as a space or a tab.
/// - **Punctuation:** every other char, such as `.`, `(` or `😀`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Motion {
    /// One character back: two bytes after a CRLF, one char otherwise. At
    /// offset 0 it stays.
    ///
    /// When [`Editor::move_cursors`](crate::Editor::move_cursors) moves a
    /// selection that isn't empty, the selection becomes a cursor at its
    /// start instead, the way VS Code collapses a selection on Left.
    Left,
    /// One character forward: two bytes before a CRLF, one char otherwise.
    /// At the end of the text it stays.
    ///
    /// When [`Editor::move_cursors`](crate::Editor::move_cursors) moves a
    /// selection that isn't empty, the selection becomes a cursor at its
    /// end instead.
    Right,
    /// To the line above, at the selection's goal column (see
    /// [`Editor`](crate::Editor), "Goal columns"), capped at that line's
    /// length without its line break. On the first line it goes to offset
    /// 0.
    Up,
    /// To the line below, the same way as [`Up`](Self::Up). On the last
    /// line it goes to the end of the text.
    Down,
    /// To the start of the word, or run of punctuation, before the head:
    ///
    /// 1. If a line break ends right before the head, move to before it,
    ///    and stop.
    /// 2. Skip back over blanks. If that reaches the start of a line, or
    ///    offset 0, stop there.
    /// 3. Skip back over the run of chars of the same class as the char
    ///    just before, word or punctuation, and stop.
    ///
    /// At offset 0 it stays.
    WordLeft,
    /// To the end of the word, or run of punctuation, after the head:
    ///
    /// 1. If a line break starts at the head, move to after it, and stop.
    /// 2. Skip over blanks. If that reaches a line break, or the end of the
    ///    text, stop there.
    /// 3. Skip over the run of chars of the same class as the char just
    ///    after, word or punctuation, and stop.
    ///
    /// At the end of the text it stays.
    WordRight,
    /// To the first char of the line that isn't a blank, or, if the head
    /// is already there, to the start of the line. A line of only blanks
    /// has its first non-blank position at the end of its text. This is
    /// VS Code's Home.
    LineStart,
    /// To the end of the line's text, before its line break.
    LineEnd,
    /// To offset 0.
    TextStart,
    /// To the end of the text.
    TextEnd,
}

/// Where `motion` takes a head at `head`, a char boundary of `buffer`'s
/// text. [`Motion::Up`] and [`Motion::Down`] aim for column `goal`, or the
/// head's own column without one.
///
/// It reads a few small windows of text around the head, and for Up and
/// Down the head's line and the line it moves to. It never copies the
/// whole text.
pub(crate) fn target(buffer: &Buffer, head: usize, motion: Motion, goal: Option<usize>) -> usize {
    match motion {
        Motion::Left => left(&mut Backward::new(buffer, head)),
        Motion::Right => right(&mut Forward::new(buffer, head)),
        Motion::Up => vertical(buffer, head, false, goal),
        Motion::Down => vertical(buffer, head, true, goal),
        Motion::WordLeft => word_left(buffer, head),
        Motion::WordRight => word_right(buffer, head),
        Motion::LineStart => line_start(buffer, head).unwrap_or(head),
        Motion::LineEnd => line_end(buffer, head).unwrap_or(head),
        Motion::TextStart => 0,
        Motion::TextEnd => buffer.len_bytes(),
    }
}

/// The column of `head`, in chars from the start of its line, capped at
/// the line's length without its line break.
pub(crate) fn column(buffer: &Buffer, head: usize) -> usize {
    line_column(buffer, head).1
}

/// The first window a reader takes, in bytes. Each later one doubles, so a
/// long run of one class takes a few reads, not one per window.
const WINDOW: usize = 32;

/// Reads chars forward from an offset, one small window of text at a time.
struct Forward<'a> {
    buffer: &'a Buffer,
    /// Where `window` starts in the text.
    start: usize,
    window: String,
    /// How far into `window` the reader is.
    at: usize,
    /// The size of the next window.
    size: usize,
}

impl<'a> Forward<'a> {
    fn new(buffer: &'a Buffer, offset: usize) -> Self {
        Self {
            buffer,
            start: offset,
            window: String::new(),
            at: 0,
            size: WINDOW,
        }
    }

    /// The reader's offset in the text.
    fn offset(&self) -> usize {
        self.start + self.at
    }

    /// The next char, without moving past it.
    fn peek(&mut self) -> Option<char> {
        if self.at == self.window.len() {
            let start = self.offset();
            let len = self.buffer.len_bytes();
            if start >= len {
                return None;
            }
            // `start` is a char boundary and a char is at most four bytes,
            // so stepping back finds a boundary past `start`.
            let mut end = (start + self.size).min(len);
            self.window = loop {
                match self.buffer.slice(start..end) {
                    Ok(window) => break window,
                    Err(_) => end -= 1,
                }
            };
            self.start = start;
            self.at = 0;
            self.size *= 2;
        }
        self.window[self.at..].chars().next()
    }

    /// Moves past the next char, if there is one.
    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.at += c.len_utf8();
        }
    }
}

/// Reads chars backward from an offset, one small window of text at a
/// time.
struct Backward<'a> {
    buffer: &'a Buffer,
    /// Where `window` starts in the text.
    start: usize,
    window: String,
    /// How far into `window` the reader is: chars before it are unread.
    at: usize,
    /// The size of the next window.
    size: usize,
}

impl<'a> Backward<'a> {
    fn new(buffer: &'a Buffer, offset: usize) -> Self {
        Self {
            buffer,
            start: offset,
            window: String::new(),
            at: 0,
            size: WINDOW,
        }
    }

    /// The reader's offset in the text.
    fn offset(&self) -> usize {
        self.start + self.at
    }

    /// The char before the reader, without moving past it.
    fn peek(&mut self) -> Option<char> {
        if self.at == 0 {
            let end = self.offset();
            if end == 0 {
                return None;
            }
            // 0 is a char boundary, so stepping back finds one.
            let mut start = end.saturating_sub(self.size);
            self.window = loop {
                match self.buffer.slice(start..end) {
                    Ok(window) => break window,
                    Err(_) => start -= 1,
                }
            };
            self.start = start;
            self.at = self.window.len();
            self.size *= 2;
        }
        self.window[..self.at].chars().next_back()
    }

    /// Moves back past the char before the reader, if there is one.
    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.at -= c.len_utf8();
        }
    }
}

/// Whether `c` is part of a line break.
fn is_break(c: char) -> bool {
    c == '\n' || c == '\r'
}

/// Whether `c` is a blank: whitespace that isn't part of a line break.
fn is_blank(c: char) -> bool {
    c.is_whitespace() && !is_break(c)
}

/// A char's class for the word motions, besides line breaks.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Word,
    Blank,
    Punctuation,
}

fn class(c: char) -> Class {
    if is_word(c) {
        Class::Word
    } else if is_blank(c) {
        Class::Blank
    } else {
        Class::Punctuation
    }
}

/// Moves `reader` past one char, or both bytes of a CRLF, and returns
/// where it ends.
fn right(reader: &mut Forward<'_>) -> usize {
    let first = reader.peek();
    reader.advance();
    if first == Some('\r') && reader.peek() == Some('\n') {
        reader.advance();
    }
    reader.offset()
}

/// Moves `reader` back past one char, or both bytes of a CRLF, and returns
/// where it ends.
fn left(reader: &mut Backward<'_>) -> usize {
    let last = reader.peek();
    reader.advance();
    if last == Some('\n') && reader.peek() == Some('\r') {
        reader.advance();
    }
    reader.offset()
}

/// [`Motion::WordRight`] from `head`.
fn word_right(buffer: &Buffer, head: usize) -> usize {
    let mut reader = Forward::new(buffer, head);
    match reader.peek() {
        None => return head,
        Some(c) if is_break(c) => return right(&mut reader),
        Some(_) => {}
    }
    while reader.peek().is_some_and(is_blank) {
        reader.advance();
    }
    let Some(run) = reader.peek().filter(|&c| !is_break(c)).map(class) else {
        return reader.offset();
    };
    while reader
        .peek()
        .is_some_and(|c| !is_break(c) && class(c) == run)
    {
        reader.advance();
    }
    reader.offset()
}

/// [`Motion::WordLeft`] from `head`.
fn word_left(buffer: &Buffer, head: usize) -> usize {
    let mut reader = Backward::new(buffer, head);
    match reader.peek() {
        None => return head,
        Some(c) if is_break(c) => return left(&mut reader),
        Some(_) => {}
    }
    while reader.peek().is_some_and(is_blank) {
        reader.advance();
    }
    let Some(run) = reader.peek().filter(|&c| !is_break(c)).map(class) else {
        return reader.offset();
    };
    while reader
        .peek()
        .is_some_and(|c| !is_break(c) && class(c) == run)
    {
        reader.advance();
    }
    reader.offset()
}

/// [`Motion::LineStart`] from `head`: the line's first non-blank, or the
/// line's start if the head is already there.
fn line_start(buffer: &Buffer, head: usize) -> Option<usize> {
    let start = buffer.line_to_byte(buffer.byte_to_line(head).ok()?).ok()?;
    let mut reader = Forward::new(buffer, start);
    while reader.peek().is_some_and(is_blank) {
        reader.advance();
    }
    let first = reader.offset();
    Some(if head == first { start } else { first })
}

/// [`Motion::LineEnd`] from `head`: the end of its line's text, before the
/// line break.
fn line_end(buffer: &Buffer, head: usize) -> Option<usize> {
    let line = buffer.byte_to_line(head).ok()?;
    if line + 1 >= buffer.len_lines() {
        return Some(buffer.len_bytes());
    }
    let next = buffer.line_to_byte(line + 1).ok()?;
    Some(left(&mut Backward::new(buffer, next)))
}

/// A reader just after `head`'s line start, and `head`'s column as
/// [`column`] counts it. A head between the CR and LF of a CRLF counts
/// from before the CR.
fn line_column(buffer: &Buffer, head: usize) -> (Backward<'_>, usize) {
    let mut reader = Backward::new(buffer, head);
    if reader.peek() == Some('\r') && Forward::new(buffer, head).peek() == Some('\n') {
        reader.advance();
    }
    let mut column = 0;
    while reader.peek().is_some_and(|c| !is_break(c)) {
        reader.advance();
        column += 1;
    }
    (reader, column)
}

/// Moves `reader` up to `column` chars forward, stopping before a line
/// break, and returns where it ends.
fn to_column(reader: &mut Forward<'_>, column: usize) -> usize {
    for _ in 0..column {
        if !reader.peek().is_some_and(|c| !is_break(c)) {
            break;
        }
        reader.advance();
    }
    reader.offset()
}

/// Up (`down` false) or Down from `head`, at column `goal` or the head's
/// own column. It reads the head's line and the line it moves to.
fn vertical(buffer: &Buffer, head: usize, down: bool, goal: Option<usize>) -> usize {
    let (mut back, own) = line_column(buffer, head);
    let column = goal.unwrap_or(own);
    if down {
        let mut reader = Forward::new(buffer, head);
        while reader.peek().is_some_and(|c| !is_break(c)) {
            reader.advance();
        }
        if reader.peek().is_none() {
            return buffer.len_bytes();
        }
        right(&mut reader);
        to_column(&mut reader, column)
    } else {
        if back.peek().is_none() {
            return 0;
        }
        left(&mut back);
        while back.peek().is_some_and(|c| !is_break(c)) {
            back.advance();
        }
        to_column(&mut Forward::new(buffer, back.offset()), column)
    }
}
