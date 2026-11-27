//! Vim's motions (T-108), read with T-107's [`Forward`] and [`Backward`]
//! readers and [`Reads`]: where each motion takes a normal-mode cursor,
//! what range an operator covers, and the positions Vim's cursor may be on.
//! The [`vim`](crate::vim) module docs hold the rules.
//!
//! Every function here takes offsets that are valid in the buffer: on a
//! char boundary and at most its length. The ones that check an offset
//! first say so, and return `None` for one that isn't.
//!
//! Word motions read a window at a time from the cursor and use the line
//! index only where they need a line's start or end. Line motions use a
//! fixed number of line-index lookups however far they go, so `999999j`
//! costs what `j` does.

use std::ops::Range;

use helm_text::{Buffer, TextBuffer};

use super::{
    Backward, Forward, Motion, Reads, is_blank, is_break, left, right, text_end, try_target,
};
use crate::editor::is_word;
use crate::vim_commands as ids;

pub(crate) mod object;

/// One of Vim's sixteen motions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum VimMotion {
    /// `h`.
    Left,
    /// `l`.
    Right,
    /// `j`.
    Down,
    /// `k`.
    Up,
    /// `w`.
    NextWord,
    /// `b`.
    PrevWord,
    /// `e`.
    WordEnd,
    /// `W`.
    NextBigWord,
    /// `B`.
    PrevBigWord,
    /// `E`.
    BigWordEnd,
    /// `0`.
    LineStart,
    /// `^`.
    FirstNonBlank,
    /// `$`.
    LineEnd,
    /// `gg`.
    FirstLine,
    /// `G`.
    LastLine,
    /// The second `d` of `dd`, and the second `c` of `cc`.
    Line,
}

impl VimMotion {
    /// Every motion, with the id that runs it in normal mode and the id
    /// that runs it in the visual modes, if it has one.
    pub(crate) const ALL: [(Self, &'static str, Option<&'static str>); 16] = [
        (Self::Left, ids::LEFT, Some(ids::SELECT_LEFT)),
        (Self::Right, ids::RIGHT, Some(ids::SELECT_RIGHT)),
        (Self::Down, ids::DOWN, Some(ids::SELECT_DOWN)),
        (Self::Up, ids::UP, Some(ids::SELECT_UP)),
        (Self::NextWord, ids::NEXT_WORD, Some(ids::SELECT_NEXT_WORD)),
        (Self::PrevWord, ids::PREV_WORD, Some(ids::SELECT_PREV_WORD)),
        (Self::WordEnd, ids::WORD_END, Some(ids::SELECT_WORD_END)),
        (
            Self::NextBigWord,
            ids::NEXT_BIG_WORD,
            Some(ids::SELECT_NEXT_BIG_WORD),
        ),
        (
            Self::PrevBigWord,
            ids::PREV_BIG_WORD,
            Some(ids::SELECT_PREV_BIG_WORD),
        ),
        (
            Self::BigWordEnd,
            ids::BIG_WORD_END,
            Some(ids::SELECT_BIG_WORD_END),
        ),
        (
            Self::LineStart,
            ids::LINE_START,
            Some(ids::SELECT_LINE_START),
        ),
        (
            Self::FirstNonBlank,
            ids::FIRST_NON_BLANK,
            Some(ids::SELECT_FIRST_NON_BLANK),
        ),
        (Self::LineEnd, ids::LINE_END, Some(ids::SELECT_LINE_END)),
        (
            Self::FirstLine,
            ids::FIRST_LINE,
            Some(ids::SELECT_FIRST_LINE),
        ),
        (Self::LastLine, ids::LAST_LINE, Some(ids::SELECT_LAST_LINE)),
        (Self::Line, ids::LINE, None),
    ];

    /// The motion whose normal-mode id is `id`: the motions an operator
    /// takes. `None` for every other id, the `vim.select_` ones included.
    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, normal, _)| *normal == id)
            .map(|&(motion, _, _)| motion)
    }

    /// Whether every target of this motion from a normal-mode position is
    /// a normal-mode position too, so it needs no settling: `b` and `e`
    /// land on the first or last char of a run, an empty line, offset 0,
    /// or where they started.
    pub(crate) fn lands_on_a_char(self) -> bool {
        matches!(
            self,
            Self::PrevWord | Self::PrevBigWord | Self::WordEnd | Self::BigWordEnd
        )
    }

    /// Whether this is `j` or `k`, which keep goal columns.
    pub(crate) fn is_vertical(self) -> bool {
        matches!(self, Self::Down | Self::Up)
    }

    /// Whether this is `gg` or `G`, whose target doesn't depend on the
    /// cursor.
    pub(crate) fn is_absolute(self) -> bool {
        matches!(self, Self::FirstLine | Self::LastLine)
    }
}

/// What an operator acts on from one cursor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Span {
    /// These bytes. Never empty.
    Chars(Range<usize>),
    /// Lines `a` to `b`, `a <= b`, which the operator turns into bytes.
    Lines(usize, usize),
}

/// A char's class for Vim's word motions: 0 for white space (blanks and
/// line breaks), 2 for word chars, 1 for the rest. For WORDs every char
/// that isn't white space is 1.
fn class(c: char, big: bool) -> u8 {
    if is_break(c) || is_blank(c) {
        0
    } else if big || !is_word(c) {
        1
    } else {
        2
    }
}

/// The char at `p`, or `None` at the end of the text.
fn char_at(buffer: &Buffer, reads: &mut Reads, p: usize) -> Option<char> {
    Forward::new(buffer, &mut reads.forward, p).peek()
}

/// The char before `p`, or `None` at offset 0.
fn char_before(buffer: &Buffer, reads: &mut Reads, p: usize) -> Option<char> {
    Backward::new(buffer, &mut reads.backward, p).peek()
}

/// The offset after the char at `p`, or both bytes of a CRLF. At the end
/// of the text, `p`.
pub(crate) fn next(buffer: &Buffer, reads: &mut Reads, p: usize) -> usize {
    right(&mut Forward::new(buffer, &mut reads.forward, p))
}

/// The offset of the char before `p`, or of the CR of a CRLF just before
/// it. At offset 0, 0.
pub(crate) fn prev(buffer: &Buffer, reads: &mut Reads, p: usize) -> usize {
    left(&mut Backward::new(buffer, &mut reads.backward, p))
}

/// Whether `p` starts a line: offset 0, or right after a line break.
pub(crate) fn is_line_start(buffer: &Buffer, reads: &mut Reads, p: usize) -> bool {
    p == 0 || char_before(buffer, reads, p).is_some_and(is_break)
}

/// The index of the last line.
fn last_line(buffer: &Buffer) -> usize {
    buffer.len_lines() - 1
}

/// The line `p` is on.
pub(crate) fn line_of(buffer: &Buffer, p: usize) -> usize {
    buffer.byte_to_line(p).unwrap_or_else(|_| last_line(buffer))
}

/// Where line `line` starts. `line` is at most the last line.
fn line_start(buffer: &Buffer, line: usize) -> usize {
    buffer.line_to_byte(line).unwrap_or(buffer.len_bytes())
}

/// Where line `line`'s text ends, before its line break.
fn line_text_end(buffer: &Buffer, reads: &mut Reads, line: usize) -> usize {
    text_end(buffer, reads, line).unwrap_or(buffer.len_bytes())
}

/// Line `line`'s first char that isn't a blank, or the end of its text if
/// it has none.
pub(crate) fn first_non_blank(buffer: &Buffer, reads: &mut Reads, line: usize) -> usize {
    let mut reader = Forward::new(buffer, &mut reads.forward, line_start(buffer, line));
    reader.skip_while(is_blank);
    reader.offset()
}

/// `p`'s column: chars from its line's start.
fn column(buffer: &Buffer, p: usize) -> usize {
    let start = line_start(buffer, line_of(buffer, p));
    let chars = |at| buffer.byte_to_char(at).unwrap_or(0);
    chars(p).saturating_sub(chars(start))
}

/// The offset `column` chars into line `line`, or the end of its text if
/// the line is shorter.
fn at_column(buffer: &Buffer, reads: &mut Reads, line: usize, column: usize) -> usize {
    let start = buffer.byte_to_char(line_start(buffer, line)).unwrap_or(0);
    let end = line_text_end(buffer, reads, line);
    let end_char = buffer.byte_to_char(end).unwrap_or(start);
    buffer
        .char_to_byte(start.saturating_add(column).min(end_char))
        .unwrap_or(end)
}

/// `p` settled to a normal-mode position: out of a CRLF, then off the end
/// of a line that has text onto its last char. `None` if `p` isn't a
/// valid offset.
pub(crate) fn settle(buffer: &Buffer, reads: &mut Reads, p: usize) -> Option<usize> {
    if p > buffer.len_bytes() {
        return None;
    }
    let mut after = {
        let mut reader = Forward::new(buffer, &mut reads.forward, p);
        let c = reader.peek();
        reader.valid().then_some(c)?
    };
    // On a char that isn't a line break, `p` is settled already, and can't
    // be inside a CRLF: only the text after it needs reading.
    if after.is_some_and(|c| !is_break(c)) {
        return Some(p);
    }
    let mut reader = Backward::new(buffer, &mut reads.backward, p);
    let mut before = reader.peek();
    if !reader.valid() {
        return None;
    }
    let mut p = p;
    if before == Some('\r') && after == Some('\n') {
        reader.advance();
        p -= 1;
        after = Some('\r');
        before = reader.peek();
    }
    match before {
        Some(c) if !is_break(c) && after.is_none_or(is_break) => Some(p - c.len_utf8()),
        _ => Some(p),
    }
}

/// `p` settled to a visual-mode end: a normal-mode position that isn't the
/// empty last line after a final line break. On that line it is the line
/// before's position instead. `None` if `p` isn't valid.
pub(crate) fn vsettle(buffer: &Buffer, reads: &mut Reads, p: usize) -> Option<usize> {
    let p = settle(buffer, reads, p)?;
    // A settled offset at the end of a text that isn't empty is the empty
    // last line.
    if p == buffer.len_bytes() && p > 0 {
        let before = prev(buffer, reads, p);
        settle(buffer, reads, before)
    } else {
        Some(p)
    }
}

/// Escape in insert mode: `p` moved back one char unless it starts its
/// line, then settled. `None` if `p` isn't valid.
pub(crate) fn step_back(buffer: &Buffer, reads: &mut Reads, p: usize) -> Option<usize> {
    if p > buffer.len_bytes() {
        return None;
    }
    let mut reader = Backward::new(buffer, &mut reads.backward, p);
    let before = reader.peek();
    if !reader.valid() {
        return None;
    }
    let back = match before {
        Some(c) if !is_break(c) => p - c.len_utf8(),
        _ => p,
    };
    drop(reader);
    settle(buffer, reads, back)
}

/// Where `a` (Vim's append) starts typing from a cursor on the char at
/// `p`: after that char, or at `p` on an empty line.
pub(crate) fn after_char(buffer: &Buffer, reads: &mut Reads, p: usize) -> usize {
    match char_at(buffer, reads, p) {
        Some(c) if !is_break(c) => p + c.len_utf8(),
        _ => p,
    }
}

/// The first non-blank of `p`'s line, Vim's `I`.
pub(crate) fn line_first_non_blank(buffer: &Buffer, reads: &mut Reads, p: usize) -> usize {
    first_non_blank(buffer, reads, line_of(buffer, p))
}

/// Where line `line` starts, for callers outside this module.
pub(crate) fn line_start_of_line(buffer: &Buffer, line: usize) -> usize {
    line_start(buffer, line)
}

/// Where `p`'s line starts, `O`'s insert point.
pub(crate) fn line_start_of(buffer: &Buffer, p: usize) -> usize {
    line_start(buffer, line_of(buffer, p))
}

/// Where `p`'s line's text ends, `A`'s and `o`'s insert point.
pub(crate) fn line_end_of(buffer: &Buffer, reads: &mut Reads, p: usize) -> usize {
    line_text_end(buffer, reads, line_of(buffer, p))
}

/// The visual cursor and the anchor char a visual-mode selection stands
/// for, each a visual-mode end: `(anchor, cursor)`. An empty selection at
/// the end of the text is the empty visual selection there, with both at
/// the end. `None` if an offset isn't valid.
pub(crate) fn decode(
    buffer: &Buffer,
    reads: &mut Reads,
    anchor: usize,
    head: usize,
) -> Option<(usize, usize)> {
    let len = buffer.len_bytes();
    if anchor > len || head > len {
        return None;
    }
    if anchor == head {
        let at = settle(buffer, reads, head)?;
        let at = if at == len {
            at
        } else {
            vsettle(buffer, reads, at)?
        };
        return Some((at, at));
    }
    // Each end is read, and so checked, by the settle at it or the step
    // back over it.
    if anchor < head {
        let anchor = vsettle(buffer, reads, anchor)?;
        let last = checked_prev(buffer, reads, head)?;
        let cursor = vsettle(buffer, reads, last)?;
        Some((anchor, cursor))
    } else {
        let cursor = vsettle(buffer, reads, head)?;
        let last = checked_prev(buffer, reads, anchor)?;
        let anchor = vsettle(buffer, reads, last)?;
        Some((anchor, cursor))
    }
}

/// [`prev`], or `None` if `p` isn't a valid offset.
fn checked_prev(buffer: &Buffer, reads: &mut Reads, p: usize) -> Option<usize> {
    let mut reader = Backward::new(buffer, &mut reads.backward, p);
    let at = left(&mut reader);
    reader.valid().then_some(at)
}

/// The selection with its anchor char at `anchor` and its visual cursor at
/// `cursor`, both visual-mode ends: the bytes from one char to the other,
/// both included, pointing from the anchor.
pub(crate) fn encode(
    buffer: &Buffer,
    reads: &mut Reads,
    anchor: usize,
    cursor: usize,
) -> (usize, usize) {
    if cursor >= anchor {
        (anchor, next(buffer, reads, cursor))
    } else {
        (next(buffer, reads, anchor), cursor)
    }
}

/// `step` from `p` up to `n` times, stopping as soon as a step doesn't
/// move.
fn repeat(p: usize, n: usize, mut step: impl FnMut(usize) -> usize) -> usize {
    let mut p = p;
    for _ in 0..n {
        let q = step(p);
        if q == p {
            break;
        }
        p = q;
    }
    p
}

/// `w` (or `W`, with `big`) from `p`.
fn next_word(buffer: &Buffer, reads: &mut Reads, p: usize, big: bool) -> usize {
    let mut reader = Forward::new(buffer, &mut reads.forward, p);
    let Some(c) = reader.peek() else {
        return p;
    };
    let run = class(c, big);
    if run != 0 {
        reader.skip_while(|c| class(c, big) == run);
    }
    // Whether the reader is at a line start it reached by moving: there a
    // line break means an empty line, where `w` stops.
    let mut line_start = false;
    loop {
        match reader.peek() {
            None => return reader.offset(),
            Some(c) if is_break(c) => {
                if line_start {
                    return reader.offset();
                }
                right(&mut reader);
                line_start = true;
            }
            Some(c) if is_blank(c) => {
                reader.skip_while(is_blank);
                line_start = false;
            }
            Some(_) => return reader.offset(),
        }
    }
}

/// `b` (or `B`, with `big`) from `p`.
fn prev_word(buffer: &Buffer, reads: &mut Reads, p: usize, big: bool) -> usize {
    let mut reader = Backward::new(buffer, &mut reads.backward, p);
    loop {
        let Some(c) = reader.peek() else {
            return reader.offset();
        };
        let run = class(c, big);
        if run != 0 {
            reader.advance();
            reader.skip_while(|c| class(c, big) == run);
            return reader.offset();
        }
        left(&mut reader);
        let at = reader.offset();
        // Stop at offset 0, or at the start of an empty line.
        if at == 0 || is_break(c) && reader.peek().is_some_and(is_break) {
            return at;
        }
    }
}

/// From a reader on a char of class `run`, the offset of the run's last
/// char.
fn run_end(reader: &mut Forward<'_>, run: u8, big: bool) -> usize {
    let mut last = reader.offset();
    while let Some(c) = reader.peek() {
        if class(c, big) != run {
            break;
        }
        last = reader.offset();
        reader.advance();
    }
    last
}

/// `e` (or `E`, with `big`) from `p`. Stays at `p` when no run follows.
fn word_end(buffer: &Buffer, reads: &mut Reads, p: usize, big: bool) -> usize {
    let mut reader = Forward::new(buffer, &mut reads.forward, p);
    let Some(c) = reader.peek() else {
        return p;
    };
    let run = class(c, big);
    right(&mut reader);
    if run != 0 && reader.peek().is_some_and(|c| class(c, big) == run) {
        return run_end(&mut reader, run, big);
    }
    reader.skip_while(|c| class(c, big) == 0);
    match reader.peek() {
        None => p,
        Some(c) => run_end(&mut reader, class(c, big), big),
    }
}

/// Whether the char at `p`, which isn't white space, is the last of its
/// run.
fn is_run_end(buffer: &Buffer, reads: &mut Reads, p: usize, big: bool) -> bool {
    let mut reader = Forward::new(buffer, &mut reads.forward, p);
    let run = reader.peek().map_or(0, |c| class(c, big));
    reader.advance();
    reader.peek().is_none_or(|c| class(c, big) != run)
}

/// The last step of `w` under an operator: like `w`, but it stops at the
/// end of the line's text instead of crossing a line break, and from the
/// end of a line's text it goes to the start of the next line.
fn word_in_line(buffer: &Buffer, reads: &mut Reads, p: usize, big: bool) -> usize {
    let mut reader = Forward::new(buffer, &mut reads.forward, p);
    match reader.peek() {
        None => p,
        Some(c) if is_break(c) => right(&mut reader),
        Some(c) => {
            let run = class(c, big);
            if run != 0 {
                reader.skip_while(|c| class(c, big) == run);
            }
            reader.skip_while(is_blank);
            reader.offset()
        }
    }
}

/// `h` and `l` with a count: `n` chars back or forward within the line.
fn sideways(buffer: &Buffer, reads: &mut Reads, p: usize, n: usize, forward: bool) -> usize {
    if n == 1 {
        // The common case reads the char next to the cursor.
        let c = if forward {
            char_at(buffer, reads, p)
        } else {
            char_before(buffer, reads, p)
        };
        return match c {
            Some(c) if !is_break(c) && forward => p + c.len_utf8(),
            Some(c) if !is_break(c) => p - c.len_utf8(),
            _ => p,
        };
    }
    // A count is arithmetic on the column.
    let line = line_of(buffer, p);
    let chars = |at| buffer.byte_to_char(at).unwrap_or(0);
    let at = chars(p);
    let to = if forward {
        at.saturating_add(n)
            .min(chars(line_text_end(buffer, reads, line)))
    } else {
        at.saturating_sub(n).max(chars(line_start(buffer, line)))
    };
    buffer.char_to_byte(to).unwrap_or(p)
}

/// `j` (with `down`) or `k` from `p`, `n` lines, aiming for `goal`, and the
/// goal it used. `None` on the last or first line.
fn vertical(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    n: usize,
    down: bool,
    goal: Option<usize>,
) -> Option<(usize, usize)> {
    let line = line_of(buffer, p);
    let last = last_line(buffer);
    if down && line == last || !down && line == 0 {
        return None;
    }
    if n == 1 {
        let motion = if down { Motion::Down } else { Motion::Up };
        return try_target(buffer, p, motion, goal, reads);
    }
    let target = if down {
        line.saturating_add(n).min(last)
    } else {
        line.saturating_sub(n)
    };
    let goal = goal.unwrap_or_else(|| column(buffer, p));
    Some((at_column(buffer, reads, target, goal), goal))
}

/// Where `motion` takes a cursor at normal-mode position `p`, unsettled,
/// with count `count` and goal column `goal`, and the goal column `j` and
/// `k` used (0 for the other motions). `None` for `j` and `k` that can't
/// move.
pub(crate) fn target(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    motion: VimMotion,
    count: Option<usize>,
    goal: Option<usize>,
) -> Option<(usize, usize)> {
    let n = count.unwrap_or(1).max(1);
    let last = last_line(buffer);
    let at = match motion {
        VimMotion::Left => sideways(buffer, reads, p, n, false),
        VimMotion::Right => sideways(buffer, reads, p, n, true),
        VimMotion::Down => return vertical(buffer, reads, p, n, true, goal),
        VimMotion::Up => return vertical(buffer, reads, p, n, false, goal),
        VimMotion::NextWord | VimMotion::NextBigWord => {
            let big = motion == VimMotion::NextBigWord;
            repeat(p, n, |q| next_word(buffer, reads, q, big))
        }
        VimMotion::PrevWord | VimMotion::PrevBigWord => {
            let big = motion == VimMotion::PrevBigWord;
            repeat(p, n, |q| prev_word(buffer, reads, q, big))
        }
        VimMotion::WordEnd | VimMotion::BigWordEnd => {
            let big = motion == VimMotion::BigWordEnd;
            repeat(p, n, |q| word_end(buffer, reads, q, big))
        }
        VimMotion::LineStart => line_start_of(buffer, p),
        VimMotion::FirstNonBlank => line_first_non_blank(buffer, reads, p),
        VimMotion::LineEnd => {
            let line = line_of(buffer, p).saturating_add(n - 1).min(last);
            line_text_end(buffer, reads, line)
        }
        VimMotion::FirstLine => {
            let line = count.map_or(0, |n| n.saturating_sub(1)).min(last);
            first_non_blank(buffer, reads, line)
        }
        VimMotion::LastLine => {
            let line = count.map_or(last, |n| n.saturating_sub(1)).min(last);
            first_non_blank(buffer, reads, line)
        }
        VimMotion::Line => {
            let line = line_of(buffer, p).saturating_add(n - 1).min(last);
            first_non_blank(buffer, reads, line)
        }
    };
    Some((at, 0))
}

/// `p`'s column, for a `j` or `k` that can't move and has no goal yet.
pub(crate) fn goal_at(buffer: &Buffer, p: usize) -> usize {
    column(buffer, p)
}

/// What an operator acts on from a cursor at normal-mode position `p` with
/// `motion` and `count`, or `None` if nothing. `change` is set for `c`,
/// whose `w` and `W` act as `e` and `E` on a char that isn't white space.
pub(crate) fn operator_span(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    motion: VimMotion,
    count: Option<usize>,
    change: bool,
) -> Option<Span> {
    let n = count.unwrap_or(1).max(1);
    match motion {
        VimMotion::Down
        | VimMotion::Up
        | VimMotion::FirstLine
        | VimMotion::LastLine
        | VimMotion::Line => {
            let (t, _) = target(buffer, reads, p, motion, count, None)?;
            let (a, b) = (line_of(buffer, p), line_of(buffer, t));
            Some(Span::Lines(a.min(b), a.max(b)))
        }
        VimMotion::WordEnd | VimMotion::BigWordEnd => {
            // Inclusive. An `e` that finds no next run ends on the char under
            // the cursor.
            let (t, _) = target(buffer, reads, p, motion, count, None)?;
            let end = next(buffer, reads, t);
            (p < end).then_some(Span::Chars(p..end))
        }
        VimMotion::LineEnd => {
            let from = line_of(buffer, p);
            let to = from.saturating_add(n - 1).min(last_line(buffer));
            let t = line_text_end(buffer, reads, to);
            if to > from && t == line_start(buffer, to) && p <= first_non_blank(buffer, reads, from)
            {
                return Some(Span::Lines(from, to));
            }
            (p < t).then_some(Span::Chars(p..t))
        }
        VimMotion::NextWord | VimMotion::NextBigWord => {
            let big = motion == VimMotion::NextBigWord;
            let on_word = char_at(buffer, reads, p).is_some_and(|c| class(c, big) != 0);
            if change && on_word {
                // `cw` is `ce`, except that its first step stays on the last
                // char of a word.
                let first = if is_run_end(buffer, reads, p, big) {
                    p
                } else {
                    word_end(buffer, reads, p, big)
                };
                let t = repeat(first, n - 1, |q| word_end(buffer, reads, q, big));
                let end = next(buffer, reads, t);
                return Some(Span::Chars(p..end));
            }
            let s = repeat(p, n - 1, |q| next_word(buffer, reads, q, big));
            let t = word_in_line(buffer, reads, s, big);
            exclusive(buffer, reads, p, t)
        }
        VimMotion::Left
        | VimMotion::Right
        | VimMotion::PrevWord
        | VimMotion::PrevBigWord
        | VimMotion::LineStart
        | VimMotion::FirstNonBlank => {
            let (t, _) = target(buffer, reads, p, motion, count, None)?;
            exclusive(buffer, reads, p.min(t), p.max(t))
        }
    }
}

/// An exclusive range from `a` to `b`, with Vim's rule for one that ends at
/// the start of a later line.
fn exclusive(buffer: &Buffer, reads: &mut Reads, a: usize, b: usize) -> Option<Span> {
    if a >= b {
        return None;
    }
    // `a < b`, so a line start at `b` is on a later line than `a`.
    if !is_line_start(buffer, reads, b) {
        return Some(Span::Chars(a..b));
    }
    let first = line_of(buffer, a);
    if a <= first_non_blank(buffer, reads, first) {
        return Some(Span::Lines(first, line_of(buffer, b) - 1));
    }
    let end = prev(buffer, reads, b);
    (a < end).then_some(Span::Chars(a..end))
}

/// The bytes a linewise delete of lines `a` to `b` removes: from the start
/// of `a` to the start of the line after `b`, or, when `b` is the last
/// line, from the end of the text of the line before `a`.
pub(crate) fn delete_lines(buffer: &Buffer, reads: &mut Reads, a: usize, b: usize) -> Range<usize> {
    if b < last_line(buffer) {
        line_start(buffer, a)..line_start(buffer, b + 1)
    } else if a > 0 {
        line_text_end(buffer, reads, a - 1)..buffer.len_bytes()
    } else {
        0..buffer.len_bytes()
    }
}

/// The text of lines `a` to `b`, as a linewise register piece holds it:
/// ending with a line break, `newline` if the last line had none.
pub(crate) fn lines_text(buffer: &Buffer, a: usize, b: usize, newline: &str) -> String {
    let end = if b < last_line(buffer) {
        line_start(buffer, b + 1)
    } else {
        buffer.len_bytes()
    };
    let mut text = buffer.slice(line_start(buffer, a)..end).unwrap_or_default();
    if !text.ends_with(['\n', '\r']) {
        text.push_str(newline);
    }
    text
}

/// The bytes lines `a` to `b` cover, their last line break included.
pub(crate) fn lines_range(buffer: &Buffer, a: usize, b: usize) -> Range<usize> {
    let end = if b < last_line(buffer) {
        line_start(buffer, b + 1)
    } else {
        buffer.len_bytes()
    };
    line_start(buffer, a)..end
}

/// `p`'s column, and the offset at that column on line `line`, for a
/// yank's cursor.
pub(crate) fn same_column(buffer: &Buffer, reads: &mut Reads, p: usize, line: usize) -> usize {
    at_column(buffer, reads, line, column(buffer, p))
}

/// The bytes a linewise change of lines `a` to `b` removes: from the start
/// of `a` to the end of `b`'s text, keeping `b`'s line break.
pub(crate) fn change_lines(buffer: &Buffer, reads: &mut Reads, a: usize, b: usize) -> Range<usize> {
    line_start(buffer, a)..line_text_end(buffer, reads, b)
}
