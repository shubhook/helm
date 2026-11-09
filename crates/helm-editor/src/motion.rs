//! Motions: where a cursor goes from its head. [`Editor::move_cursors`],
//! [`Editor::extend_selections`] and [`Editor::delete_to`] take one, and
//! the cursor commands in [`cursor_commands`](crate::cursor_commands) are
//! one motion each.
//!
//! [`Editor::move_cursors`]: crate::Editor::move_cursors
//! [`Editor::extend_selections`]: crate::Editor::extend_selections
//! [`Editor::delete_to`]: crate::Editor::delete_to

use helm_text::{Buffer, TextBuffer, TextError};

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

/// Where `motion` takes a head at `head`, and the goal column it used:
/// `goal`, or for Up and Down without one, the head's own column. Other
/// motions return a goal of 0, which callers ignore.
///
/// Returns `None` if `head` isn't a valid offset: past the end of the
/// text, or inside a char. A valid head is one clamping would leave as it
/// is, so callers that get `Some` for every selection need not clamp. The
/// check costs nothing extra: it comes from the first read near the head.
///
/// Every motion reads small windows of text next to the head. Up and Down
/// scan at most [`SCAN_LIMIT`] bytes and use the line index past that, so a
/// very long line costs a few index lookups, not a scan. Nothing copies
/// the whole text.
pub(crate) fn try_target(
    buffer: &Buffer,
    head: usize,
    motion: Motion,
    goal: Option<usize>,
    reads: &mut Reads,
) -> Option<(usize, usize)> {
    if head > buffer.len_bytes() {
        return None;
    }
    let at = match motion {
        Motion::Left => {
            let mut reader = Backward::new(buffer, &mut reads.backward, head);
            let at = left(&mut reader);
            reader.valid().then_some(at)?
        }
        Motion::Right => {
            let mut reader = Forward::new(buffer, &mut reads.forward, head);
            let at = right(&mut reader);
            reader.valid().then_some(at)?
        }
        Motion::Up | Motion::Down => {
            let down = motion == Motion::Down;
            return match vertical_scan(buffer, reads, head, down, goal) {
                Scan::Done(at, goal) => Some((at, goal)),
                Scan::Invalid => None,
                Scan::TooLong => vertical_by_lines(buffer, reads, head, down, goal),
            };
        }
        Motion::WordLeft => word_left(buffer, reads, head)?,
        Motion::WordRight => word_right(buffer, reads, head)?,
        Motion::LineStart => line_start(buffer, reads, head)?,
        Motion::LineEnd => line_end(buffer, reads, head)?,
        Motion::TextStart => is_valid(buffer, head).then_some(0)?,
        Motion::TextEnd => is_valid(buffer, head).then_some(buffer.len_bytes())?,
    };
    Some((at, 0))
}

/// Where `motion` takes `head`, a clamped offset, aiming for column `goal`
/// on Up and Down.
pub(crate) fn target(
    buffer: &Buffer,
    head: usize,
    motion: Motion,
    goal: Option<usize>,
    reads: &mut Reads,
) -> usize {
    try_target(buffer, head, motion, goal, reads).map_or(head, |(at, _)| at)
}

/// Whether `at` is an offset clamping leaves as it is: at most the length,
/// and on a char boundary.
pub(crate) fn is_valid(buffer: &Buffer, at: usize) -> bool {
    at <= buffer.len_bytes() && buffer.byte_to_char(at).is_ok()
}

/// The most bytes Up and Down scan each way from a head before they switch
/// to the line index. Past this limit the index costs a fixed dozen lookups
/// however long the line is, so a minified line doesn't make each press
/// scan it.
const SCAN_LIMIT: usize = 512;

/// The first window a word reader takes, in bytes. Each later one doubles,
/// so a long run takes a few reads, not one per window.
const WINDOW: usize = 32;

/// The first window Up and Down take. Down needs the rest of the head's
/// line and the start of the next; Up needs the head's line up to it and
/// the whole line above. For code lines of typical length (about 40 bytes)
/// that is one read; a longer line takes a second, twice as large. A
/// larger first window costs more on every press, because the copy out of
/// the rope grows with it.
const LINE_WINDOW: usize = 128;

/// Text one command has read, kept so the next selection's reads can use
/// it. Selections are sorted, so the next head is often inside what the
/// last one read: cursors on adjacent lines, or matches close together.
/// One command reads one buffer that doesn't change while it runs, so
/// nothing here goes stale. Make a new one for each command.
#[derive(Debug, Default)]
pub(crate) struct Reads {
    /// The last text read forward.
    forward: Window,
    /// The last text read backward, joined with what was read before it.
    backward: Window,
}

impl Reads {
    /// Whether `at` is a char boundary, if it falls in text already read.
    fn boundary(&self, at: usize) -> Option<bool> {
        [&self.forward, &self.backward]
            .into_iter()
            .find_map(|window| {
                let i = at.checked_sub(window.start)?;
                (i <= window.text.len()).then(|| window.text.is_char_boundary(i))
            })
    }

    /// Whether `at` is an offset clamping leaves as it is, looking first in
    /// text already read.
    pub(crate) fn is_valid(&self, buffer: &Buffer, at: usize) -> bool {
        at <= buffer.len_bytes() && self.boundary(at).unwrap_or_else(|| is_valid(buffer, at))
    }

    /// Where VS Code's Left puts a selection that isn't empty: a cursor at
    /// `start`, or before the CR if `start` is inside a CRLF. `None` if
    /// `start` isn't valid.
    pub(crate) fn collapse_left(&mut self, buffer: &Buffer, start: usize) -> Option<usize> {
        if start > buffer.len_bytes() {
            return None;
        }
        let mut after = Forward::new(buffer, &mut self.forward, start);
        let next = after.peek();
        if !after.valid() {
            return None;
        }
        let inside = next == Some('\n')
            && Backward::new(buffer, &mut self.backward, start).peek() == Some('\r');
        Some(if inside { start - 1 } else { start })
    }

    /// Where VS Code's Right puts a selection that isn't empty: a cursor at
    /// `end`, or after the LF if `end` is inside a CRLF. `None` if `end`
    /// isn't valid.
    pub(crate) fn collapse_right(&mut self, buffer: &Buffer, end: usize) -> Option<usize> {
        if end > buffer.len_bytes() {
            return None;
        }
        let mut before = Backward::new(buffer, &mut self.backward, end);
        let last = before.peek();
        if !before.valid() {
            return None;
        }
        let inside =
            last == Some('\r') && Forward::new(buffer, &mut self.forward, end).peek() == Some('\n');
        Some(if inside { end + 1 } else { end })
    }
}

/// One run of text read out of the buffer.
#[derive(Debug, Default)]
struct Window {
    /// Where `text` starts in the buffer. A char boundary.
    start: usize,
    text: String,
}

impl Window {
    fn end(&self) -> usize {
        self.start + self.text.len()
    }
}

/// Reads chars forward from an offset, a window of text at a time, through
/// `window`.
struct Forward<'a> {
    buffer: &'a Buffer,
    /// Where `window` goes back when the reader is done.
    slot: &'a mut Window,
    /// The text the reader reads from, taken out of `slot` so the hot loop
    /// reads it directly.
    window: Window,
    /// The reader's offset in the buffer.
    at: usize,
    /// The size of the next window read.
    size: usize,
    /// Where the reader stops reading new text and sets `cut`.
    limit: usize,
    /// The reader stopped at `limit`, not at the end of the text.
    cut: bool,
    /// The starting offset wasn't a char boundary.
    invalid: bool,
}

impl<'a> Forward<'a> {
    fn new(buffer: &'a Buffer, slot: &'a mut Window, offset: usize) -> Self {
        Self::bounded(buffer, slot, offset, WINDOW, usize::MAX)
    }

    fn bounded(
        buffer: &'a Buffer,
        slot: &'a mut Window,
        offset: usize,
        size: usize,
        budget: usize,
    ) -> Self {
        let window = std::mem::take(slot);
        // A starting offset inside text already read must be a boundary
        // there; one outside it is checked by the read that fetches it.
        let invalid = window.start <= offset
            && offset < window.end()
            && !window.text.is_char_boundary(offset - window.start);
        Self {
            buffer,
            slot,
            window,
            at: offset,
            size,
            limit: offset.saturating_add(budget),
            cut: false,
            invalid,
        }
    }

    /// The reader's offset in the text.
    fn offset(&self) -> usize {
        self.at
    }

    /// Whether the starting offset was valid, as far as the reader read.
    fn valid(&self) -> bool {
        !self.invalid
    }

    /// The next char, without moving past it.
    fn peek(&mut self) -> Option<char> {
        if self.invalid {
            return None;
        }
        // One compare: wraps to a huge index when `at` is before the window.
        if self.at.wrapping_sub(self.window.start) >= self.window.text.len() && !self.fill() {
            return None;
        }
        self.window.text[self.at - self.window.start..]
            .chars()
            .next()
    }

    /// Reads the window that starts at the reader. False at the end of the
    /// text, at the limit, or if the starting offset was invalid.
    fn fill(&mut self) -> bool {
        let start = self.at;
        let len = self.buffer.len_bytes();
        if start >= len {
            return false;
        }
        if start >= self.limit {
            self.cut = true;
            return false;
        }
        // At least four bytes, so stepping back always finds a boundary.
        let size = self.size.min(self.limit - start).max(4);
        let mut end = start.saturating_add(size).min(len);
        let text = loop {
            match self.buffer.slice(start..end) {
                Ok(text) => break text,
                // `start` is a boundary and a char is at most four bytes,
                // so stepping back finds one past `start`.
                Err(TextError::NotCharBoundary { offset }) if offset == end && end > start + 1 => {
                    end -= 1;
                }
                Err(_) => {
                    self.invalid = true;
                    return false;
                }
            }
        };
        self.window = Window { start, text };
        self.size = self.size.saturating_mul(2);
        true
    }

    /// Moves past the next char, if there is one.
    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.at += c.len_utf8();
        }
    }

    /// Moves to the next line break, or the end of what it can read.
    fn skip_line(&mut self) {
        while self.peek().is_some() {
            let rest = &self.window.text[self.at - self.window.start..];
            match rest.find(['\n', '\r']) {
                Some(i) => {
                    self.at += i;
                    return;
                }
                None => self.at += rest.len(),
            }
        }
    }

    /// Moves past chars while `keep` holds, and returns how many it moved
    /// past.
    fn skip_while(&mut self, keep: impl Fn(char) -> bool) -> usize {
        let mut count = 0;
        while self.peek().is_some() {
            let rest = &self.window.text[self.at - self.window.start..];
            for (i, c) in rest.char_indices() {
                if !keep(c) {
                    self.at += i;
                    return count;
                }
                count += 1;
            }
            self.at += rest.len();
        }
        count
    }

    /// Moves `count` chars forward, stopping early at a line break or the
    /// end of what it can read.
    fn skip_chars(&mut self, mut count: usize) {
        while count > 0 && self.peek().is_some() {
            let rest = &self.window.text[self.at - self.window.start..];
            let mut moved = rest.len();
            for (i, c) in rest.char_indices() {
                if count == 0 || is_break(c) {
                    moved = i;
                    count = 0;
                    break;
                }
                count -= 1;
            }
            self.at += moved;
        }
    }
}

impl Drop for Forward<'_> {
    fn drop(&mut self) {
        *self.slot = std::mem::take(&mut self.window);
    }
}

/// Reads chars backward from an offset, a window of text at a time,
/// through `window`. Windows it reads join onto the front of what the
/// window holds, so Up can look forward through the line it scanned back
/// over.
struct Backward<'a> {
    buffer: &'a Buffer,
    /// Where `window` goes back when the reader is done.
    slot: &'a mut Window,
    /// The text the reader reads from, taken out of `slot` so the hot loop
    /// reads it directly.
    window: Window,
    /// The reader's offset in the buffer: chars before it are unread.
    at: usize,
    /// The size of the next window read.
    size: usize,
    /// Where the reader stops reading new text and sets `cut`.
    limit: usize,
    /// The reader stopped at `limit`, not at offset 0.
    cut: bool,
    /// The starting offset wasn't a char boundary.
    invalid: bool,
}

impl<'a> Backward<'a> {
    fn new(buffer: &'a Buffer, slot: &'a mut Window, offset: usize) -> Self {
        Self::bounded(buffer, slot, offset, WINDOW, usize::MAX)
    }

    fn bounded(
        buffer: &'a Buffer,
        slot: &'a mut Window,
        offset: usize,
        size: usize,
        budget: usize,
    ) -> Self {
        let window = std::mem::take(slot);
        let invalid = window.start < offset
            && offset <= window.end()
            && !window.text.is_char_boundary(offset - window.start);
        Self {
            buffer,
            slot,
            window,
            at: offset,
            size,
            limit: offset.saturating_sub(budget),
            cut: false,
            invalid,
        }
    }

    /// The reader's offset in the text.
    fn offset(&self) -> usize {
        self.at
    }

    /// Whether the starting offset was valid, as far as the reader read.
    fn valid(&self) -> bool {
        !self.invalid
    }

    /// The char before the reader, without moving past it.
    fn peek(&mut self) -> Option<char> {
        if self.invalid {
            return None;
        }
        // One compare: `i - 1` wraps to a huge index when `at` is at or
        // before the window's start.
        let i = self.at.wrapping_sub(self.window.start);
        if i.wrapping_sub(1) >= self.window.text.len() && !self.fill() {
            return None;
        }
        self.window.text[..self.at - self.window.start]
            .chars()
            .next_back()
    }

    /// Reads the window that ends at the reader. False at offset 0, at the
    /// limit, or if the starting offset was invalid.
    fn fill(&mut self) -> bool {
        let end = self.at;
        if end == 0 {
            return false;
        }
        if end <= self.limit {
            self.cut = true;
            return false;
        }
        // At least four bytes, so stepping back always finds a boundary.
        let size = self.size.min(end - self.limit).max(4);
        let mut start = end.saturating_sub(size);
        let text = loop {
            match self.buffer.slice(start..end) {
                Ok(text) => break text,
                // 0 is a boundary, so stepping back finds one.
                Err(TextError::NotCharBoundary { offset }) if offset == start && start > 0 => {
                    start -= 1;
                }
                Err(_) => {
                    self.invalid = true;
                    return false;
                }
            }
        };
        if self.window.start == end && !self.window.text.is_empty() {
            // Join onto what the reader read before.
            self.window.text.insert_str(0, &text);
            self.window.start = start;
        } else {
            self.window = Window { start, text };
        }
        self.size = self.size.saturating_mul(2);
        true
    }

    /// Moves back past the char before the reader, if there is one.
    fn advance(&mut self) {
        if let Some(c) = self.peek() {
            self.at -= c.len_utf8();
        }
    }

    /// Moves back past chars while `keep` holds, and returns how many it
    /// moved past.
    fn skip_while(&mut self, keep: impl Fn(char) -> bool) -> usize {
        let mut count = 0;
        while self.peek().is_some() {
            let before = &self.window.text[..self.at - self.window.start];
            for (i, c) in before.char_indices().rev() {
                if !keep(c) {
                    self.at = self.window.start + i + c.len_utf8();
                    return count;
                }
                count += 1;
            }
            self.at = self.window.start;
        }
        count
    }

    /// Moves back to just after the previous line break, or the start of
    /// what it can read, and returns how many chars it moved over.
    fn skip_line(&mut self) -> usize {
        let mut chars = 0;
        while self.peek().is_some() {
            let before = &self.window.text[..self.at - self.window.start];
            match before.rfind(['\n', '\r']) {
                Some(i) => {
                    // A line break is one byte, so the line starts after it.
                    chars += before[i + 1..].chars().count();
                    self.at = self.window.start + i + 1;
                    return chars;
                }
                None => {
                    chars += before.chars().count();
                    self.at = self.window.start;
                }
            }
        }
        chars
    }

    /// The text from the reader to `end`, which must be inside what the
    /// reader has read.
    fn text_to(&self, end: usize) -> &str {
        &self.window.text[self.at - self.window.start..end - self.window.start]
    }
}

impl Drop for Backward<'_> {
    fn drop(&mut self) {
        *self.slot = std::mem::take(&mut self.window);
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

/// [`Motion::WordRight`] from `head`, or `None` if `head` is invalid.
fn word_right(buffer: &Buffer, reads: &mut Reads, head: usize) -> Option<usize> {
    let mut reader = Forward::new(buffer, &mut reads.forward, head);
    let at = match reader.peek() {
        None => head,
        Some(c) if is_break(c) => right(&mut reader),
        Some(_) => {
            reader.skip_while(is_blank);
            if let Some(run) = reader.peek().filter(|&c| !is_break(c)).map(class) {
                reader.skip_while(|c| !is_break(c) && class(c) == run);
            }
            reader.offset()
        }
    };
    reader.valid().then_some(at)
}

/// [`Motion::WordLeft`] from `head`, or `None` if `head` is invalid.
fn word_left(buffer: &Buffer, reads: &mut Reads, head: usize) -> Option<usize> {
    let mut reader = Backward::new(buffer, &mut reads.backward, head);
    let at = match reader.peek() {
        None => head,
        Some(c) if is_break(c) => left(&mut reader),
        Some(_) => {
            reader.skip_while(is_blank);
            if let Some(run) = reader.peek().filter(|&c| !is_break(c)).map(class) {
                reader.skip_while(|c| !is_break(c) && class(c) == run);
            }
            reader.offset()
        }
    };
    reader.valid().then_some(at)
}

/// [`Motion::LineStart`] from `head`: the line's first non-blank, or the
/// line's start if the head is already there. `None` if `head` is invalid.
fn line_start(buffer: &Buffer, reads: &mut Reads, head: usize) -> Option<usize> {
    let start = buffer.line_to_byte(buffer.byte_to_line(head).ok()?).ok()?;
    let mut reader = Forward::new(buffer, &mut reads.forward, start);
    reader.skip_while(is_blank);
    let first = reader.offset();
    Some(if head == first { start } else { first })
}

/// [`Motion::LineEnd`] from `head`, or `None` if `head` is invalid.
fn line_end(buffer: &Buffer, reads: &mut Reads, head: usize) -> Option<usize> {
    text_end(buffer, reads, buffer.byte_to_line(head).ok()?)
}

/// Where line `line`'s text ends, before its line break.
fn text_end(buffer: &Buffer, reads: &mut Reads, line: usize) -> Option<usize> {
    if line + 1 >= buffer.len_lines() {
        return Some(buffer.len_bytes());
    }
    let next = buffer.line_to_byte(line + 1).ok()?;
    let mut reader = Backward::new(buffer, &mut reads.backward, next);
    let at = left(&mut reader);
    reader.valid().then_some(at)
}

/// How a bounded scan for Up or Down ended.
enum Scan {
    /// The target and the goal column used.
    Done(usize, usize),
    /// The head isn't a valid offset.
    Invalid,
    /// A line ran past [`SCAN_LIMIT`]; use the line index instead.
    TooLong,
}

/// Up (`down` false) or Down from `head` by scanning the text next to it,
/// at most [`SCAN_LIMIT`] bytes each way. Without a `goal`, the goal is the
/// head's column. A head between the CR and LF of a CRLF counts its column
/// from before the CR.
fn vertical_scan(
    buffer: &Buffer,
    reads: &mut Reads,
    head: usize,
    down: bool,
    goal: Option<usize>,
) -> Scan {
    let Reads { forward, backward } = reads;
    // Down with a goal never needs the head's own line: skip reading it.
    let mut back = None;
    let goal = match goal {
        Some(goal) if down => goal,
        _ => {
            let mut reader = Backward::bounded(buffer, backward, head, LINE_WINDOW, SCAN_LIMIT);
            if reader.peek() == Some('\r')
                && Forward::new(buffer, forward, head).peek() == Some('\n')
            {
                reader.advance();
            }
            let column = reader.skip_line();
            if !reader.valid() {
                return Scan::Invalid;
            }
            if reader.cut {
                return Scan::TooLong;
            }
            back = Some(reader);
            goal.unwrap_or(column)
        }
    };

    if down {
        let mut reader = Forward::bounded(buffer, forward, head, LINE_WINDOW, SCAN_LIMIT);
        reader.skip_line();
        if reader.peek().is_none() {
            return match (reader.valid(), reader.cut) {
                (false, _) => Scan::Invalid,
                (true, true) => Scan::TooLong,
                (true, false) => Scan::Done(buffer.len_bytes(), goal),
            };
        }
        right(&mut reader);
        reader.skip_chars(goal);
        return match (reader.valid(), reader.cut) {
            (false, _) => Scan::Invalid,
            (true, true) => Scan::TooLong,
            (true, false) => Scan::Done(reader.offset(), goal),
        };
    }

    let Some(mut reader) = back else {
        return Scan::Invalid;
    };
    if reader.offset() == 0 {
        return Scan::Done(0, goal);
    }
    let line_end = left(&mut reader);
    reader.skip_line();
    if reader.cut {
        return Scan::TooLong;
    }
    // The line above is in what the reader read: walk `goal` chars into it.
    let line = reader.text_to(line_end);
    let at = line
        .char_indices()
        .nth(goal)
        .map_or(line.len(), |(at, _)| at);
    Scan::Done(reader.offset() + at, goal)
}

/// Up (`down` false) or Down from `head` through the line index: a fixed
/// number of lookups however long the lines are. `None` if `head` is
/// invalid.
fn vertical_by_lines(
    buffer: &Buffer,
    reads: &mut Reads,
    head: usize,
    down: bool,
    goal: Option<usize>,
) -> Option<(usize, usize)> {
    let line = buffer.byte_to_line(head).ok()?;
    let goal = match goal {
        Some(goal) => goal,
        None => {
            let start = buffer.byte_to_char(buffer.line_to_byte(line).ok()?).ok()?;
            buffer
                .byte_to_char(head.min(text_end(buffer, reads, line)?))
                .ok()?
                - start
        }
    };
    let target = if down {
        if line + 1 >= buffer.len_lines() {
            return Some((buffer.len_bytes(), goal));
        }
        line + 1
    } else {
        match line.checked_sub(1) {
            Some(target) => target,
            None => return Some((0, goal)),
        }
    };
    let start = buffer
        .byte_to_char(buffer.line_to_byte(target).ok()?)
        .ok()?;
    let end = buffer.byte_to_char(text_end(buffer, reads, target)?).ok()?;
    let at = buffer
        .char_to_byte(start.saturating_add(goal).min(end))
        .ok()?;
    Some((at, goal))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every char boundary of `text` that isn't inside a CRLF.
    fn positions(text: &str) -> Vec<usize> {
        text.char_indices()
            .map(|(at, _)| at)
            .chain([text.len()])
            .filter(|&at| !(at > 0 && text[..at].ends_with('\r') && text[at..].starts_with('\n')))
            .collect()
    }

    /// Lines short enough to scan and long enough to need the line index,
    /// with every line break and char width.
    fn mixed_text() -> String {
        let long = "ab é中😀 ".repeat(SCAN_LIMIT / 4);
        format!("x\r\nab中cd\n{long}\r\n\n😀é\r{long}\nshort\r\n{long}")
    }

    #[test]
    fn the_scan_and_the_line_index_agree() {
        let text = mixed_text();
        let buffer = Buffer::from(text.as_str());
        for at in positions(&text) {
            for down in [false, true] {
                for goal in [None, Some(0), Some(3), Some(SCAN_LIMIT), Some(usize::MAX)] {
                    let by_lines =
                        vertical_by_lines(&buffer, &mut Reads::default(), at, down, goal);
                    match vertical_scan(&buffer, &mut Reads::default(), at, down, goal) {
                        Scan::Done(to, used) => {
                            assert_eq!(Some((to, used)), by_lines, "{at} {down} {goal:?}");
                        }
                        Scan::TooLong => assert!(by_lines.is_some()),
                        Scan::Invalid => panic!("{at} is valid"),
                    }
                }
            }
        }
    }

    #[test]
    fn reads_shared_across_sorted_heads_change_nothing() {
        // One command moves its selections in order with one `Reads`; each
        // target must be what a fresh read gives.
        let text = mixed_text();
        let buffer = Buffer::from(text.as_str());
        let motions = [
            Motion::Left,
            Motion::Right,
            Motion::Up,
            Motion::Down,
            Motion::WordLeft,
            Motion::WordRight,
            Motion::LineStart,
            Motion::LineEnd,
        ];
        for motion in motions {
            for goal in [None, Some(2)] {
                let mut shared = Reads::default();
                for at in positions(&text) {
                    assert_eq!(
                        try_target(&buffer, at, motion, goal, &mut shared),
                        try_target(&buffer, at, motion, goal, &mut Reads::default()),
                        "{motion:?} from {at} goal {goal:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn long_lines_use_the_line_index() {
        let text = mixed_text();
        let buffer = Buffer::from(text.as_str());
        // 64 repeats of the 13-byte piece into the long line after "😀é\r":
        // more than SCAN_LIMIT from both of its ends.
        let start = text.find("😀é\r").expect("in the text") + "😀é\r".len();
        let middle = start + 13 * (SCAN_LIMIT / 8);
        for down in [false, true] {
            assert!(matches!(
                vertical_scan(&buffer, &mut Reads::default(), middle, down, None),
                Scan::TooLong
            ));
        }
        assert_eq!(
            try_target(&buffer, middle, Motion::Down, None, &mut Reads::default()),
            vertical_by_lines(&buffer, &mut Reads::default(), middle, true, None)
        );
    }

    #[test]
    fn invalid_heads_are_reported() {
        let buffer = Buffer::from("aé\r\nb");
        for motion in [
            Motion::Left,
            Motion::Right,
            Motion::Up,
            Motion::Down,
            Motion::WordLeft,
            Motion::WordRight,
            Motion::LineStart,
            Motion::LineEnd,
            Motion::TextStart,
            Motion::TextEnd,
        ] {
            assert_eq!(
                try_target(&buffer, 2, motion, None, &mut Reads::default()),
                None,
                "{motion:?}"
            );
            assert_eq!(
                try_target(&buffer, 7, motion, None, &mut Reads::default()),
                None,
                "{motion:?}"
            );
            assert!(
                try_target(&buffer, 3, motion, None, &mut Reads::default()).is_some(),
                "{motion:?}"
            );
        }
    }

    #[test]
    fn small_windows_never_split_a_char() {
        // The budget runs down to fewer bytes than a char holds.
        let text = "😀".repeat(200);
        let buffer = Buffer::from(text.as_str());
        let mut window = Window::default();
        let mut reader = Forward::bounded(&buffer, &mut window, 0, 3, 7);
        reader.skip_line();
        assert!(reader.valid() && reader.cut);
        let mut window = Window::default();
        let mut reader = Backward::bounded(&buffer, &mut window, text.len(), 3, 7);
        reader.skip_line();
        assert!(reader.valid() && reader.cut);
    }
}
