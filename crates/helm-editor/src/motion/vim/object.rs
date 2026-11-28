//! Vim's text objects (T-109): the range each object gives from a
//! normal-mode cursor, read with T-107's readers. The [`vim`](crate::vim)
//! module docs' "Text objects" hold the rules.
//!
//! Word objects read a window at a time from the cursor and stop at the
//! line's ends. Quote objects read the cursor's line from its start.
//! Bracket objects read outward from the cursor, at most 1 MiB each way,
//! and use the line index only for the block rule at the end. Paragraph objects use a fixed
//! number of line-index lookups per line they cross.

use std::cell::Cell;

use helm_text::{Buffer, TextBuffer};

use super::{
    Span, char_at, first_non_blank, last_line, line_of, line_start, line_text_end, next, prev,
    vsettle,
};
use crate::editor::is_word;
use crate::motion::{Backward, Forward, Reads, WINDOW, is_blank, is_break, left, right};
use crate::vim_commands as ids;

/// What an object's range is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// `w`, or `W` with `big`.
    Word {
        /// WORDs, not words.
        big: bool,
    },
    /// A bracket pair.
    Pair {
        /// The opening bracket.
        open: char,
        /// The closing bracket.
        close: char,
    },
    /// A quote pair within one line.
    Quote(char),
    /// `p`.
    Paragraph,
}

/// One of Vim's text objects: what it covers, and whether it is the `i`
/// or the `a` form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VimObject {
    /// What it covers.
    pub(crate) kind: Kind,
    /// `i`, not `a`.
    pub(crate) inner: bool,
}

impl VimObject {
    /// Every object, with the id an operator takes and the id that runs it
    /// in a visual mode.
    pub(crate) const ALL: [(Self, &'static str, &'static str); 20] = {
        const fn object(kind: Kind, inner: bool) -> VimObject {
            VimObject { kind, inner }
        }
        const WORD: Kind = Kind::Word { big: false };
        const BIG: Kind = Kind::Word { big: true };
        const PAREN: Kind = Kind::Pair {
            open: '(',
            close: ')',
        };
        const BRACKET: Kind = Kind::Pair {
            open: '[',
            close: ']',
        };
        const BRACE: Kind = Kind::Pair {
            open: '{',
            close: '}',
        };
        const ANGLE: Kind = Kind::Pair {
            open: '<',
            close: '>',
        };
        [
            (object(WORD, true), ids::INNER_WORD, ids::SELECT_INNER_WORD),
            (object(WORD, false), ids::A_WORD, ids::SELECT_A_WORD),
            (
                object(BIG, true),
                ids::INNER_BIG_WORD,
                ids::SELECT_INNER_BIG_WORD,
            ),
            (object(BIG, false), ids::A_BIG_WORD, ids::SELECT_A_BIG_WORD),
            (
                object(PAREN, true),
                ids::INNER_PAREN,
                ids::SELECT_INNER_PAREN,
            ),
            (object(PAREN, false), ids::A_PAREN, ids::SELECT_A_PAREN),
            (
                object(BRACKET, true),
                ids::INNER_BRACKET,
                ids::SELECT_INNER_BRACKET,
            ),
            (
                object(BRACKET, false),
                ids::A_BRACKET,
                ids::SELECT_A_BRACKET,
            ),
            (
                object(BRACE, true),
                ids::INNER_BRACE,
                ids::SELECT_INNER_BRACE,
            ),
            (object(BRACE, false), ids::A_BRACE, ids::SELECT_A_BRACE),
            (
                object(ANGLE, true),
                ids::INNER_ANGLE,
                ids::SELECT_INNER_ANGLE,
            ),
            (object(ANGLE, false), ids::A_ANGLE, ids::SELECT_A_ANGLE),
            (
                object(Kind::Quote('"'), true),
                ids::INNER_DOUBLE_QUOTE,
                ids::SELECT_INNER_DOUBLE_QUOTE,
            ),
            (
                object(Kind::Quote('"'), false),
                ids::A_DOUBLE_QUOTE,
                ids::SELECT_A_DOUBLE_QUOTE,
            ),
            (
                object(Kind::Quote('\''), true),
                ids::INNER_SINGLE_QUOTE,
                ids::SELECT_INNER_SINGLE_QUOTE,
            ),
            (
                object(Kind::Quote('\''), false),
                ids::A_SINGLE_QUOTE,
                ids::SELECT_A_SINGLE_QUOTE,
            ),
            (
                object(Kind::Quote('`'), true),
                ids::INNER_BACKTICK,
                ids::SELECT_INNER_BACKTICK,
            ),
            (
                object(Kind::Quote('`'), false),
                ids::A_BACKTICK,
                ids::SELECT_A_BACKTICK,
            ),
            (
                object(Kind::Paragraph, true),
                ids::INNER_PARAGRAPH,
                ids::SELECT_INNER_PARAGRAPH,
            ),
            (
                object(Kind::Paragraph, false),
                ids::A_PARAGRAPH,
                ids::SELECT_A_PARAGRAPH,
            ),
        ]
    };

    /// The object an operator takes under id `id`, or `None` for every
    /// other id.
    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(_, operator, _)| *operator == id)
            .map(|&(object, _, _)| object)
    }
}

/// What `object` covers from a cursor at normal-mode position `p`, with
/// count `count`, or `None` if it finds nothing. A quote object's `i` may
/// give an empty range, between two quotes with nothing between them.
pub(crate) fn span(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    object: VimObject,
    count: Option<usize>,
) -> Option<Span> {
    let n = count.unwrap_or(1).max(1);
    match object.kind {
        Kind::Word { big } => word(buffer, reads, p, n, object.inner, big, false),
        Kind::Pair { open, close } => pair(buffer, reads, p, n, object.inner, open, close),
        Kind::Quote(quote) => self::quote(buffer, reads, p, n, object.inner, quote),
        Kind::Paragraph => paragraph(buffer, reads, p, n, object.inner),
    }
}

/// A word object's runs taken onward from `p`, not from the start of the
/// run `p` is in: what a word object adds to a visual selection whose
/// visual cursor is just before `p`.
pub(crate) fn word_from(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    object: VimObject,
    count: Option<usize>,
) -> Option<Span> {
    let Kind::Word { big } = object.kind else {
        return None;
    };
    word(
        buffer,
        reads,
        p,
        count.unwrap_or(1).max(1),
        object.inner,
        big,
        true,
    )
}

/// [`is_blank`], for the objects' backward reads. A function of its own
/// makes `Backward::skip_while` a copy of its own here, so T-107's and
/// T-108's motions keep theirs inlined as before T-109: when the objects
/// shared it, LLVM stopped inlining it into `try_target`'s word motions on
/// x86_64.
fn blank_back(c: char) -> bool {
    is_blank(c)
}

/// A char's class for the word objects: `None` for a line break, then 0
/// for a blank, 2 for a word char and 1 for the rest. For WORDs every char
/// that isn't a blank is 1.
fn class(c: char, big: bool) -> Option<u8> {
    if is_break(c) {
        None
    } else if is_blank(c) {
        Some(0)
    } else if big || !is_word(c) {
        Some(1)
    } else {
        Some(2)
    }
}

/// The class of the char at `reader`, or `None` at a line break or the end
/// of the text.
fn next_class(reader: &mut Forward<'_>, big: bool) -> Option<u8> {
    reader.peek().and_then(|c| class(c, big))
}

/// `iw`, `aw`, `iW` or `aW` from `p` with count `n`. With `onward`, the
/// first run starts at `p` and `aw` takes no white space before it.
fn word(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    n: usize,
    inner: bool,
    big: bool,
    onward: bool,
) -> Option<Span> {
    let first = class(char_at(buffer, reads, p)?, big)?;
    let start = if onward {
        p
    } else {
        let mut back = Backward::new(buffer, &mut reads.backward, p);
        back.skip_while(|c| class(c, big) == Some(first));
        back.offset()
    };
    let mut reader = Forward::new(buffer, &mut reads.forward, p);
    // The class of the next run, or `None` at a line break or the end.
    let next_run = |reader: &mut Forward<'_>| next_class(reader, big);
    if inner {
        for _ in 0..n {
            let run = next_run(&mut reader)?;
            reader.skip_while(|c| class(c, big) == Some(run));
        }
        return Some(Span::Chars(start..reader.offset()));
    }
    let mut trailing = false;
    if first == 0 {
        // On white space: the white space and the word after it, `n`
        // times. A word with no white space before it, such as `.` right
        // after `foo`, counts as the next word on its own.
        for _ in 0..n {
            reader.skip_while(is_blank);
            let run = next_run(&mut reader)?;
            reader.skip_while(|c| class(c, big) == Some(run));
        }
        trailing = true;
    } else {
        for _ in 0..n {
            let run = next_run(&mut reader).filter(|&run| run != 0)?;
            reader.skip_while(|c| class(c, big) == Some(run));
            trailing = reader.skip_while(is_blank) > 0;
        }
    }
    let end = reader.offset();
    drop(reader);
    if trailing || onward {
        return Some(Span::Chars(start..end));
    }
    // No white space after the last word: take the white space before the
    // first instead.
    let mut back = Backward::new(buffer, &mut reads.backward, start);
    back.skip_while(blank_back);
    Some(Span::Chars(back.offset()..end))
}

/// How far a bracket object reads from where its scan starts, each way
/// (T-114): 1 MiB. A pair with a bracket further away counts as no pair.
pub(crate) const PAIR_SCAN: usize = 1 << 20;

/// The text a bracket scan from `p` may read: from `p - PAIR_SCAN` up to,
/// not including, `p + PAIR_SCAN`.
fn scan_window(p: usize) -> (usize, usize) {
    (p.saturating_sub(PAIR_SCAN), p.saturating_add(PAIR_SCAN))
}

/// The `n`th pair of `open` and `close` out from `p`, as the offsets of its
/// two brackets, with both inside `window`. With `on`, an opening bracket
/// at `p` is the first pair's, as for a cursor on it. Without, the scan
/// starts before `p` and the char at `p` doesn't count, as for the start of
/// a visual selection.
#[expect(
    clippy::too_many_arguments,
    reason = "each argument is one part of the scan"
)]
fn find_pair(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    n: usize,
    open: char,
    close: char,
    on: bool,
    (floor, ceiling): (usize, usize),
) -> Option<(usize, usize)> {
    // The opening bracket: the one at `p`, or the `n`th unmatched one
    // before it.
    let mut found = usize::from(on && char_at(buffer, reads, p) == Some(open));
    let mut o = p;
    if found < n {
        let mut back = Backward::floored(buffer, &mut reads.backward, p, floor);
        let depth = Cell::new(0usize);
        let keep = |c: char| {
            if c == close {
                depth.set(depth.get() + 1);
            } else if c == open {
                if depth.get() == 0 {
                    return false;
                }
                depth.set(depth.get() - 1);
            }
            true
        };
        while found < n {
            back.skip_while(keep);
            back.peek()?;
            back.advance();
            found += 1;
        }
        o = back.offset();
    }
    // Its closing bracket.
    let after = o + open.len_utf8();
    let budget = ceiling.saturating_sub(after);
    let mut ahead = Forward::bounded(buffer, &mut reads.forward, after, WINDOW, budget);
    let depth = Cell::new(0usize);
    ahead.skip_while(|c| {
        if c == open {
            depth.set(depth.get() + 1);
        } else if c == close {
            if depth.get() == 0 {
                return false;
            }
            depth.set(depth.get() - 1);
        }
        true
    });
    ahead.peek()?;
    Some((o, ahead.offset()))
}

/// A bracket object from `p` with count `n`: the `n`th pair of `open` and
/// `close` out from the cursor.
fn pair(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    n: usize,
    inner: bool,
    open: char,
    close: char,
) -> Option<Span> {
    let (o, c) = find_pair(buffer, reads, p, n, open, close, true, scan_window(p))?;
    if inner {
        inside(buffer, reads, o + open.len_utf8(), c)
    } else {
        Some(Span::Chars(o..c + close.len_utf8()))
    }
}

/// What `i` covers of the pair whose opening bracket ends at `after` and
/// whose closing bracket starts at `c`, or `None` if that is empty.
fn inside(buffer: &Buffer, reads: &mut Reads, after: usize, c: usize) -> Option<Span> {
    // A line break right after the opening bracket is left out.
    let mut start = after;
    let opens_a_line = char_at(buffer, reads, start).is_some_and(is_break);
    if opens_a_line {
        start = next(buffer, reads, start);
    }
    // So is the closing bracket's line when only blanks come before it.
    let mut end = c;
    let mut back = Backward::new(buffer, &mut reads.backward, c);
    back.skip_while(blank_back);
    let closes_a_line = back.offset() >= start && back.peek().is_some_and(is_break);
    if closes_a_line {
        end = left(&mut back);
    }
    drop(back);
    // With both left out, the interior is whole lines, even one empty
    // line: in `{\n\n}` both `start` and `end` are that line's break.
    if opens_a_line && closes_a_line && start <= end {
        return Some(Span::Lines(line_of(buffer, start), line_of(buffer, end)));
    }
    if start >= end {
        return None;
    }
    Some(Span::Chars(start..end))
}

/// A bracket object from a visual selection longer than one char, whose
/// first char is at `lo` and last at `hi`, as Vim 9.1 does it (T-114). The
/// pair is the `n`th out from before `lo`, so an opening bracket at `lo`
/// isn't that pair's: `va(a(` grows to the next pair out. `a` selects that
/// pair. `i` selects its inside, and while that inside lies within the
/// selection and is more than one char, it moves out one pair, so `vi(i(`
/// grows too. Gives the new selection's first and last chars, in that
/// order, or `None` if there is no such pair.
pub(crate) fn grow_pair(
    buffer: &Buffer,
    reads: &mut Reads,
    (lo, hi): (usize, usize),
    object: VimObject,
    count: Option<usize>,
) -> Option<(usize, usize)> {
    let Kind::Pair { open, close } = object.kind else {
        return None;
    };
    let window = scan_window(lo);
    let n = count.unwrap_or(1).max(1);
    let (mut o, mut c) = find_pair(buffer, reads, lo, n, open, close, false, window)?;
    loop {
        if !object.inner {
            let span = Span::Chars(o..c + close.len_utf8());
            return Some(visual_ends(buffer, reads, &span));
        }
        let span = inside(buffer, reads, o + open.len_utf8(), c)?;
        let (first, last) = visual_ends(buffer, reads, &span);
        if first < lo || last > hi || first == last {
            return Some((first, last));
        }
        (o, c) = find_pair(buffer, reads, o, 1, open, close, false, window)?;
    }
}

/// A quote object from `p` with count `n`, within `p`'s line.
fn quote(
    buffer: &Buffer,
    reads: &mut Reads,
    p: usize,
    n: usize,
    inner: bool,
    quote: char,
) -> Option<Span> {
    let from = line_start(buffer, line_of(buffer, p));
    let mut reader = Forward::new(buffer, &mut reads.forward, from);
    // One pass over the line, keeping only what the pair needs: how many
    // quotes came before the cursor and the last of them, the cursor's own
    // quote and its index, and up to two quotes after the cursor. Each
    // char costs a fixed amount, so a long line is read once.
    let mut count = 0;
    let mut last_before: Option<usize> = None;
    let mut on: Option<(usize, usize)> = None;
    let mut after: [usize; 2] = [0; 2];
    let mut found = 0;
    let mut escaped = false;
    loop {
        let at = reader.offset();
        if at > p {
            let enough = match on {
                Some((i, _)) if i % 2 == 0 => found >= 1,
                Some(_) => true,
                None if last_before.is_some() => found >= 1,
                None => found >= 2,
            };
            if enough {
                break;
            }
        }
        let Some(c) = reader.peek() else { break };
        if is_break(c) {
            break;
        }
        if c == quote && !escaped {
            if at == p {
                on = Some((count, at));
            } else if at < p {
                last_before = Some(at);
            } else if found < 2 {
                after[found] = at;
                found += 1;
            }
            count += 1;
        }
        // A backslash escapes the char after it, another backslash too, so
        // a quote is escaped after an odd number of them (T-114).
        escaped = c == '\\' && !escaped;
        reader.advance();
    }
    drop(reader);
    let next = |k: usize| (k < found).then(|| after[k]);
    let (o, c) = match on {
        Some((i, at)) if i % 2 == 0 => (at, next(0)?),
        Some((_, at)) => (last_before?, at),
        None => match last_before {
            Some(before) => (before, next(0)?),
            None => (next(0)?, next(1)?),
        },
    };
    let q = quote.len_utf8();
    if inner {
        return Some(if n >= 2 {
            Span::Chars(o..c + q)
        } else {
            Span::Chars(o + q..c)
        });
    }
    let end = {
        let mut ahead = Forward::new(buffer, &mut reads.forward, c + q);
        ahead.skip_while(is_blank);
        ahead.offset()
    };
    if end > c + q {
        return Some(Span::Chars(o..end));
    }
    let mut back = Backward::new(buffer, &mut reads.backward, o);
    back.skip_while(blank_back);
    Some(Span::Chars(back.offset()..end))
}

/// The last line a paragraph can hold: the last line, unless it is the
/// empty line after a final line break.
fn last_paragraph_line(buffer: &Buffer) -> usize {
    let last = last_line(buffer);
    if last > 0 && line_start(buffer, last) == buffer.len_bytes() {
        last - 1
    } else {
        last
    }
}

/// Whether line `line` is blank: empty, or only white space.
fn is_blank_line(buffer: &Buffer, reads: &mut Reads, line: usize) -> bool {
    let at = first_non_blank(buffer, reads, line);
    char_at(buffer, reads, at).is_none_or(is_break)
}

/// The last line of the run of lines that starts at `line` and are blank
/// when `blank` is, up to `last`.
fn run_end(buffer: &Buffer, reads: &mut Reads, line: usize, blank: bool, last: usize) -> usize {
    let mut end = line;
    while end < last && is_blank_line(buffer, reads, end + 1) == blank {
        end += 1;
    }
    end
}

/// `ip` or `ap` from `p` with count `n`.
fn paragraph(buffer: &Buffer, reads: &mut Reads, p: usize, n: usize, inner: bool) -> Option<Span> {
    let last = last_paragraph_line(buffer);
    let line = line_of(buffer, p);
    if line > last {
        return None;
    }
    let blank = is_blank_line(buffer, reads, line);
    let mut start = line;
    while start > 0 && is_blank_line(buffer, reads, start - 1) == blank {
        start -= 1;
    }
    let mut end = run_end(buffer, reads, line, blank, last);
    // Each run alternates between blank and not.
    let mut next_run = |end: &mut usize| -> bool {
        if *end >= last {
            return false;
        }
        let kind = is_blank_line(buffer, reads, *end + 1);
        *end = run_end(buffer, reads, *end + 1, kind, last);
        true
    };
    if inner {
        for _ in 1..n {
            if !next_run(&mut end) {
                return None;
            }
        }
        return Some(Span::Lines(start, end));
    }
    let mut after = true;
    for k in 0..n {
        if k > 0 && !next_run(&mut end) {
            return None;
        }
        after = next_run(&mut end);
    }
    if !after && !blank && start > 0 {
        // The last paragraph takes the blank run before it instead.
        start -= 1;
        while start > 0 && is_blank_line(buffer, reads, start - 1) {
            start -= 1;
        }
    }
    Some(Span::Lines(start, end))
}

/// The visual ends that cover `span`: its first char and its last, each a
/// visual-mode end. A visual end is never the line break of a line with
/// text, so a range that ends right after one ends on the char before.
pub(crate) fn visual_ends(buffer: &Buffer, reads: &mut Reads, span: &Span) -> (usize, usize) {
    let (first, last) = match span {
        Span::Chars(range) => (range.start, prev(buffer, reads, range.end)),
        Span::Lines(a, b) => (line_start(buffer, *a), line_text_end(buffer, reads, *b)),
    };
    let first = vsettle(buffer, reads, first).unwrap_or(first);
    let last = vsettle(buffer, reads, last).unwrap_or(last).max(first);
    (first, last)
}

/// The offset after the char at `p`, for callers outside this module.
pub(crate) fn after(buffer: &Buffer, reads: &mut Reads, p: usize) -> usize {
    let mut reader = Forward::new(buffer, &mut reads.forward, p);
    right(&mut reader)
}
