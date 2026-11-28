//! Vim's commands on an [`Editor`] (T-108): motions in normal and visual
//! modes, the operators, entering and leaving insert mode, and undo and
//! redo by a count. The [`vim`](crate::vim) module docs hold the rules, and
//! [`vim_commands`](crate::vim_commands) registers a command for each.
//!
//! Every command here reads the selections the way T-107's motions do: it
//! checks each offset as it reads next to it, and only if one isn't valid
//! does it clamp the selections first. So the usual case reads each
//! cursor's neighbourhood once and doesn't look up the line index to check
//! offsets.

use std::ops::Range;

use helm_text::{Buffer, BufferSet, Edit, TextBuffer, Transaction, TransactionId};

use super::{Editor, clamp_each, normal, owned_goals};
use crate::motion::Reads;
use crate::motion::vim::object::{self as o, VimObject};
use crate::motion::vim::{self as m, Span, VimMotion};
use crate::{EditorError, Register, Selection, Selections};

/// Vim's two operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operator {
    /// `d`.
    Delete,
    /// `c`: delete, then type where the text was.
    Change,
}

/// Where `i`, `a`, `I` and `A` start typing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// `i`: before the char under the cursor.
    Insert,
    /// `a`: after it.
    Append,
    /// `I`: before the line's first non-blank.
    LineStart,
    /// `A`: at the end of the line's text.
    LineEnd,
}

/// A normal-mode command's starting point for one selection: its settled
/// head, or `None` if an offset of it isn't valid.
fn settled_head(buffer: &Buffer, reads: &mut Reads, selection: Selection) -> Option<usize> {
    if !selection.is_empty() && !reads.is_valid(buffer, selection.anchor) {
        return None;
    }
    m::settle(buffer, reads, selection.head)
}

/// A visual-mode command's starting point for one selection: its anchor
/// char and visual cursor, or `None` if an offset of it isn't valid.
fn visual_ends(buffer: &Buffer, reads: &mut Reads, selection: Selection) -> Option<(usize, usize)> {
    m::decode(buffer, reads, selection.anchor, selection.head)
}

/// What one selection contributes to an edit: where its cursor goes if it
/// deletes nothing, and what it deletes.
type Piece = (usize, Option<Span>);

/// What a yank or an operator acts on (T-109): a motion or a text object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Target {
    /// One of Vim's motions.
    Motion(VimMotion),
    /// One of Vim's text objects.
    Object(VimObject),
}

/// The register pieces `spans` take from the text, one per span that
/// covers anything, or `None` if none does. Linewise spans are whole lines
/// ending with a line break, `newline` where the last line has none. With
/// a mix, the charwise pieces get `newline` too and the register is
/// linewise.
fn taken<'a>(
    buffer: &Buffer,
    spans: impl Iterator<Item = &'a Span> + Clone,
    newline: &str,
) -> Option<Register> {
    let linewise = spans.clone().any(|span| matches!(span, Span::Lines(..)));
    let count = spans.size_hint().0;
    taken_or_read(
        buffer,
        spans.map(|span| (span, None)),
        count,
        linewise,
        newline,
    )
}

/// [`taken`], with each span's text given where the caller already has
/// it: `Some` is used as the piece as it is, and `None` is copied out of
/// the buffer. There are at most `count` spans, and `linewise` says
/// whether any is linewise.
fn taken_or_read<'a>(
    buffer: &Buffer,
    spans: impl Iterator<Item = (&'a Span, Option<String>)>,
    count: usize,
    linewise: bool,
    newline: &str,
) -> Option<Register> {
    let mut pieces = Vec::with_capacity(count);
    for (span, text) in spans {
        match span {
            Span::Lines(a, b) => {
                pieces.push(text.unwrap_or_else(|| m::lines_text(buffer, *a, *b, newline)));
            }
            Span::Chars(range) if !range.is_empty() => {
                let mut text = text.unwrap_or_else(|| m::copy(buffer, range.clone()));
                if linewise {
                    text.push_str(newline);
                }
                pieces.push(text);
            }
            Span::Chars(_) => {}
        }
    }
    (!pieces.is_empty()).then_some(Register { pieces, linewise })
}

/// One selection's part of a Vim edit (T-109): its [`Piece`], and what the
/// register copies of it if the motion already read it: a deleted span of
/// lines before the last line as its byte range, and the text.
struct Read {
    home: usize,
    span: Option<Span>,
    lines: Option<Range<usize>>,
    text: Option<String>,
}

/// Whether `text` holds a line break.
fn has_break(text: &str) -> bool {
    text.contains(['\n', '\r'])
}

/// `text` without its last line break, and that break.
fn split_break(text: &str) -> (&str, &str) {
    let body = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .or_else(|| text.strip_suffix('\r'))
        .unwrap_or(text);
    (body, &text[body.len()..])
}

/// Where the cursor goes in charwise text put at `at`: on its last char,
/// or at its start when it spans lines.
fn put_cursor(at: usize, text: &str) -> usize {
    match text.chars().next_back() {
        Some(last) if !has_break(text) => at + text.len() - last.len_utf8(),
        _ => at,
    }
}

/// One edit of a put: the bytes it replaces, the text, and where the
/// cursor goes: an offset from the edit's start, and whether the cursor
/// then moves to the first non-blank of the line at that offset.
struct Put {
    range: Range<usize>,
    text: String,
    cursor: (usize, bool),
}

impl Editor {
    /// The selections a Vim command starts from and what `each` reads from
    /// each one. The selections are this editor's own when `each` finds
    /// every offset valid, and clamped first otherwise. Drops the goals.
    fn vim_start<T>(
        &mut self,
        buffer: &Buffer,
        reads: &mut Reads,
        mut each: impl FnMut(&Buffer, &mut Reads, Selection) -> Option<T>,
    ) -> (Selections, Vec<T>) {
        self.vim_start_with(buffer, reads, false, |b, r, s, _| each(b, r, s))
    }

    /// [`vim_start`](Self::vim_start), with `each` also given the
    /// selection's goal column when `keep_goals` is set and it has one. So
    /// `each` can run a motion as soon as it has read a selection, while
    /// what it read is still in `reads`.
    fn vim_start_with<T>(
        &mut self,
        buffer: &Buffer,
        reads: &mut Reads,
        keep_goals: bool,
        mut each: impl FnMut(&Buffer, &mut Reads, Selection, Option<usize>) -> Option<T>,
    ) -> (Selections, Vec<T>) {
        let goals = std::mem::take(&mut self.goals);
        let goals = if keep_goals { goals } else { Vec::new() };
        let fast: Option<Vec<T>> = self
            .selections
            .as_slice()
            .iter()
            .enumerate()
            .map(|(i, &selection)| each(buffer, reads, selection, goals.get(i).copied()))
            .collect();
        if let Some(values) = fast {
            return (self.selections.clone(), values);
        }
        let list = clamp_each(buffer, &self.selections);
        let clamped = normal(list.clone(), self.selections.primary_index());
        // Each selection keeps its goal through clamping, as in
        // `move_heads`.
        let goals = if goals.len() == list.len() {
            let members = list.iter().map(Selection::start).zip(goals).collect();
            owned_goals(members, &clamped)
        } else {
            Vec::new()
        };
        let values = clamped
            .as_slice()
            .iter()
            .enumerate()
            .map(|(i, &selection)| {
                each(buffer, reads, selection, goals.get(i).copied())
                    .expect("clamped offsets are valid")
            })
            .collect();
        (clamped, values)
    }

    /// Moves every cursor by `motion` in normal mode (T-108): each
    /// selection becomes a cursor at its settled head, moves, and settles.
    /// `j` and `k` use and keep goal columns. Returns whether the
    /// selections changed.
    pub(crate) fn vim_move(
        &mut self,
        set: &BufferSet,
        motion: VimMotion,
        count: Option<usize>,
    ) -> Result<bool, EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let vertical = motion.is_vertical();
        // Found once: read from every cursor, it would take turns with that
        // cursor's text in `reads`.
        let absolute = motion.is_absolute().then(|| {
            let (to, _) = m::target(buffer, &mut reads, 0, motion, count, None)
                .expect("gg and G always have a target");
            m::settle(buffer, &mut reads, to).unwrap_or(to)
        });
        let (start, moved) = self.vim_start_with(
            buffer,
            &mut reads,
            vertical,
            |buffer, reads, selection, goal| {
                let p = settled_head(buffer, reads, selection)?;
                if let Some(to) = absolute {
                    return Some((to, 0));
                }
                Some(match m::target(buffer, reads, p, motion, count, goal) {
                    Some((to, goal)) if motion.lands_on_a_char() => (to, goal),
                    Some((to, goal)) => (m::settle(buffer, reads, to).unwrap_or(to), goal),
                    None => (p, goal.unwrap_or_else(|| m::goal_at(buffer, p))),
                })
            },
        );
        let cursors = moved.iter().map(|&(to, _)| Selection::cursor(to)).collect();
        let members = if vertical { moved } else { Vec::new() };
        let selections = normal(cursors, start.primary_index());
        if vertical {
            self.goals = owned_goals(members, &selections);
        }
        let changed = selections != self.selections;
        self.selections = selections;
        Ok(changed)
    }

    /// Moves every visual cursor by `motion` (T-108), keeping each anchor
    /// char, as the `vim` module docs' "Visual selections" describe.
    /// Returns whether the selections changed.
    pub(crate) fn vim_select(
        &mut self,
        set: &BufferSet,
        motion: VimMotion,
        count: Option<usize>,
    ) -> Result<bool, EditorError> {
        let buffer = self.buffer_in(set)?;
        let len = buffer.len_bytes();
        let mut reads = Reads::default();
        let vertical = motion.is_vertical();
        // Found once, as in `vim_move`.
        let absolute = motion.is_absolute().then(|| {
            let (to, _) = m::target(buffer, &mut reads, 0, motion, count, None)
                .expect("gg and G always have a target");
            m::vsettle(buffer, &mut reads, to).unwrap_or(to)
        });
        let (start, moved) = self.vim_start_with(
            buffer,
            &mut reads,
            vertical,
            |buffer, reads, selection, goal| {
                let (anchor, cursor) = visual_ends(buffer, reads, selection)?;
                let (to, goal) = match absolute {
                    Some(to) => (to, 0),
                    None => match m::target(buffer, reads, cursor, motion, count, goal) {
                        Some((to, goal)) => (m::vsettle(buffer, reads, to).unwrap_or(to), goal),
                        None => (cursor, goal.unwrap_or_else(|| m::goal_at(buffer, cursor))),
                    },
                };
                // The empty visual selection on the empty last line re-anchors
                // on the line before once its cursor moves.
                let anchor = if anchor == len && to != cursor {
                    m::vsettle(buffer, reads, anchor).unwrap_or(anchor)
                } else {
                    anchor
                };
                let (anchor, head) = m::encode(buffer, reads, anchor, to);
                Some((Selection::new(anchor, head), goal))
            },
        );
        let selections = moved.iter().map(|&(selection, _)| selection).collect();
        let members = if vertical {
            moved
                .into_iter()
                .map(|(selection, goal)| (selection.start(), goal))
                .collect()
        } else {
            Vec::new()
        };
        let selections = normal(selections, start.primary_index());
        if vertical {
            self.goals = owned_goals(members, &selections);
        }
        let changed = selections != self.selections;
        self.selections = selections;
        Ok(changed)
    }

    /// `v` and `V` in normal mode (T-108): each cursor, settled, becomes a
    /// selection of its char.
    pub(crate) fn vim_visual(&mut self, set: &BufferSet) -> Result<(), EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, heads) = self.vim_start(buffer, &mut reads, settled_head);
        let list = heads
            .iter()
            .map(|&p| {
                let (anchor, head) = m::encode(buffer, &mut reads, p, p);
                Selection::new(anchor, head)
            })
            .collect();
        self.selections = normal(list, start.primary_index());
        Ok(())
    }

    /// Escape in a visual mode (T-108): each selection becomes a cursor on
    /// its visual cursor char, settled.
    pub(crate) fn vim_exit_visual(&mut self, set: &BufferSet) -> Result<(), EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, ends) = self.vim_start(buffer, &mut reads, visual_ends);
        let list = ends
            .iter()
            .map(|&(_, cursor)| {
                Selection::cursor(m::settle(buffer, &mut reads, cursor).unwrap_or(cursor))
            })
            .collect();
        self.selections = normal(list, start.primary_index());
        Ok(())
    }

    /// Vim's visual `o` (T-108): the visual cursor and the anchor char
    /// swap.
    pub(crate) fn vim_swap_ends(&mut self, set: &BufferSet) -> Result<(), EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, ends) = self.vim_start(buffer, &mut reads, visual_ends);
        let list = ends
            .iter()
            .map(|&(anchor, cursor)| {
                let (anchor, head) = m::encode(buffer, &mut reads, cursor, anchor);
                Selection::new(anchor, head)
            })
            .collect();
        self.selections = normal(list, start.primary_index());
        Ok(())
    }

    /// `d` or `c` with `motion` at every cursor (T-108), and the one-key
    /// edits built on them. `c` opens an undo group first, even when the
    /// motion finds nothing.
    pub(crate) fn vim_operate(
        &mut self,
        set: &mut BufferSet,
        operator: Operator,
        target: Target,
        count: Option<usize>,
    ) -> Result<Option<Register>, EditorError> {
        let change = operator == Operator::Change;
        self.vim_edit(set, change, true, |buffer, reads, selection| {
            let p = settled_head(buffer, reads, selection)?;
            Some(match target {
                Target::Motion(motion) => {
                    (p, m::operator_span(buffer, reads, p, motion, count, change))
                }
                // An object edit's undo puts the cursor at its range's start.
                Target::Object(object) => match o::span(buffer, reads, p, object, count) {
                    Some(Span::Chars(range)) if range.is_empty() && !change => (p, None),
                    Some(Span::Chars(range)) => (range.start, Some(Span::Chars(range))),
                    Some(Span::Lines(a, b)) => {
                        (m::line_start_of_line(buffer, a), Some(Span::Lines(a, b)))
                    }
                    None => (p, None),
                },
            })
        })
    }

    /// Visual `d` and `c` (T-108): deletes or changes each selection, or,
    /// with `lines`, each selection's lines.
    pub(crate) fn vim_edit_selections(
        &mut self,
        set: &mut BufferSet,
        operator: Operator,
        lines: bool,
    ) -> Result<Option<Register>, EditorError> {
        self.vim_edit(
            set,
            operator == Operator::Change,
            false,
            |buffer, reads, selection| {
                let (anchor, cursor) = visual_ends(buffer, reads, selection)?;
                let home = anchor.min(cursor);
                if lines {
                    let (a, b) = (m::line_of(buffer, anchor), m::line_of(buffer, cursor));
                    return Some((home, Some(Span::Lines(a.min(b), a.max(b)))));
                }
                let (from, to) = m::encode(buffer, reads, anchor, cursor);
                let range = from.min(to)..from.max(to);
                Some((home, (!range.is_empty()).then_some(Span::Chars(range))))
            },
        )
    }

    /// Deletes what `piece` gives each selection, in one transaction, as
    /// the `vim` module docs' "Operators" describe: line spans that overlap
    /// or touch join first, then byte ranges that overlap. With `change`,
    /// opens an undo group first and leaves each cursor where typing
    /// starts. Otherwise each cursor settles. With `at_homes`, undo puts
    /// back a cursor at each piece's home instead of the selections.
    /// Returns what it took from the text, as a register holds it (T-109).
    fn vim_edit(
        &mut self,
        set: &mut BufferSet,
        change: bool,
        at_homes: bool,
        piece: impl FnMut(&Buffer, &mut Reads, Selection) -> Option<Piece>,
    ) -> Result<Option<Register>, EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let mut piece = piece;
        // A range is usually inside the text its motion just read, so its
        // register piece is copied from there, with no second read of the
        // buffer. A deleted span of whole lines before the last line is the
        // bytes from its first line's start to the next line's start, which
        // the delete below reuses.
        let (start, mut read) = self.vim_start(buffer, &mut reads, |buffer, reads, selection| {
            let (home, span) = piece(buffer, reads, selection)?;
            let (lines, text) = match &span {
                Some(Span::Chars(range)) if !range.is_empty() => {
                    (None, reads.text(range).map(str::to_owned))
                }
                &Some(Span::Lines(a, b)) if !change && b + 1 < buffer.len_lines() => {
                    let range = m::lines_range(buffer, a, b);
                    let text = reads.text(&range).map(str::to_owned);
                    (Some(range), text)
                }
                _ => (None, None),
            };
            Some(Read {
                home,
                span,
                lines,
                text,
            })
        });
        let primary = start.primary_index();
        let before = if at_homes {
            let homes = read.iter().map(|r| Selection::cursor(r.home));
            normal(homes.collect(), primary)
        } else {
            start
        };

        // Each selection's byte range, and whether it is linewise.
        let mut ranges: Vec<Option<(Range<usize>, bool)>> = read
            .iter()
            .map(|r| match &r.span {
                Some(Span::Chars(range)) => Some((range.clone(), false)),
                _ => None,
            })
            .collect();
        let mut spans: Vec<(usize, usize, usize)> = read
            .iter()
            .enumerate()
            .filter_map(|(i, r)| match r.span {
                Some(Span::Lines(a, b)) => Some((a, b, i)),
                _ => None,
            })
            .collect();
        spans.sort_unstable();
        let mut k = 0;
        while k < spans.len() {
            let (a, mut b, _) = spans[k];
            let mut j = k + 1;
            while j < spans.len() && spans[j].0 <= b.saturating_add(1) {
                b = b.max(spans[j].1);
                j += 1;
            }
            // One span on its own before the last line deletes the range
            // read with it, which is also its register piece.
            let own = &mut read[spans[k].2];
            let range = match own.lines.clone().filter(|_| j == k + 1) {
                Some(range) => {
                    if own.text.is_none() {
                        own.text = Some(m::copy(buffer, range.clone()));
                    }
                    range
                }
                None if change => m::change_lines(buffer, &mut reads, a, b),
                None => m::delete_lines(buffer, &mut reads, a, b),
            };
            for &(_, _, i) in &spans[k..j] {
                ranges[i] = Some((range.clone(), true));
            }
            k = j;
        }
        let newline = self.format.newline();
        let count = read.len();
        let register = taken_or_read(
            buffer,
            read.iter_mut()
                .filter_map(|r| Some((r.span.as_ref()?, r.text.take()))),
            count,
            !spans.is_empty(),
            newline.as_str(),
        );

        let mut sorted: Vec<Range<usize>> = ranges
            .iter()
            .flatten()
            .map(|(range, _)| range.clone())
            .filter(|range| !range.is_empty())
            .collect();
        sorted.sort_unstable_by_key(|range| (range.start, range.end));
        let mut joined: Vec<Range<usize>> = Vec::with_capacity(sorted.len());
        for range in sorted {
            match joined.last_mut() {
                Some(last) if range.start < last.end => last.end = last.end.max(range.end),
                _ => joined.push(range),
            }
        }
        // `removed[k]` is the bytes the first `k` joined ranges remove.
        let mut removed = Vec::with_capacity(joined.len() + 1);
        removed.push(0);
        for range in &joined {
            removed.push(removed[removed.len() - 1] + range.len());
        }
        // Where each cursor lands in the text after the edit: at its range's
        // start, or where it was, or, inside a range it didn't make, where
        // that range began.
        let places: Vec<(usize, bool)> = read
            .iter()
            .zip(&ranges)
            .map(|(r, range)| {
                let (at, linewise) = match range {
                    Some((range, linewise)) => (range.start, *linewise),
                    None => (r.home, false),
                };
                let j = joined.partition_point(|r| r.end <= at);
                let at = match joined.get(j) {
                    Some(r) if r.start < at => r.start,
                    _ => at,
                };
                (at - removed[j], linewise)
            })
            .collect();

        if change {
            self.begin_undo_group();
        }
        let moved = normal(
            places
                .iter()
                .map(|&(at, _)| Selection::cursor(at))
                .collect(),
            primary,
        );
        let id = if joined.is_empty() {
            self.selections = moved;
            None
        } else {
            let transaction: Transaction = joined
                .into_iter()
                .map(|range| Edit::delete(self.buffer, range))
                .collect();
            Some(self.apply(set, transaction, before, moved))
        };
        if change {
            return Ok(register);
        }

        // Settle in the text after the edit.
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let cursors = places
            .iter()
            .map(|&(at, linewise)| {
                let at = if linewise {
                    m::first_non_blank(buffer, &mut reads, m::line_of(buffer, at))
                } else {
                    at
                };
                Selection::cursor(m::settle(buffer, &mut reads, at).unwrap_or(at))
            })
            .collect();
        let after = normal(cursors, primary);
        if let Some(record) = id.and_then(|id| self.history.get_mut(&id)) {
            record.after = after.clone();
        }
        self.selections = after;
        Ok(register)
    }

    /// `i`, `a`, `I` and `A` (T-108): moves each cursor to where typing
    /// starts, then opens an undo group.
    pub(crate) fn vim_enter_insert(
        &mut self,
        set: &BufferSet,
        entry: Entry,
    ) -> Result<(), EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, heads) = self.vim_start(buffer, &mut reads, settled_head);
        let list = heads
            .iter()
            .map(|&p| {
                Selection::cursor(match entry {
                    Entry::Insert => p,
                    Entry::Append => m::after_char(buffer, &mut reads, p),
                    Entry::LineStart => m::line_first_non_blank(buffer, &mut reads, p),
                    Entry::LineEnd => m::line_end_of(buffer, &mut reads, p),
                })
            })
            .collect();
        self.selections = normal(list, start.primary_index());
        self.begin_undo_group();
        Ok(())
    }

    /// `o` (with `below`) and `O` (T-108): opens an undo group, then inserts
    /// a line break at the end of each cursor's line's text, or at its
    /// start, as the group's first edit, and puts each cursor on the new
    /// empty line. Undo puts back a cursor at each settled head from
    /// before.
    pub(crate) fn vim_open_line(
        &mut self,
        set: &mut BufferSet,
        below: bool,
    ) -> Result<TransactionId, EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, heads) = self.vim_start(buffer, &mut reads, settled_head);
        let primary = start.primary_index();
        let before = normal(
            heads.iter().map(|&p| Selection::cursor(p)).collect(),
            primary,
        );
        let points: Vec<usize> = heads
            .iter()
            .map(|&p| {
                if below {
                    m::line_end_of(buffer, &mut reads, p)
                } else {
                    m::line_start_of(buffer, p)
                }
            })
            .collect();
        // Heads are sorted, so the points are too. One line break per line.
        let mut unique = points.clone();
        unique.dedup();
        let newline = self.format.newline();
        let newline = newline.as_str();
        let transaction: Transaction = unique
            .iter()
            .map(|&at| Edit::insert(self.buffer, at, newline))
            .collect();
        let shift = usize::from(below) * newline.len();
        let cursors = points
            .iter()
            .map(|&at| {
                let earlier = unique.partition_point(|&u| u < at);
                Selection::cursor(at + earlier * newline.len() + shift)
            })
            .collect();
        let after = normal(cursors, primary);
        self.begin_undo_group();
        Ok(self.apply(set, transaction, before, after))
    }

    /// Escape in insert mode (T-108): closes the undo group and moves each
    /// cursor back one char unless it starts its line, then settles it.
    pub(crate) fn vim_leave_insert(&mut self, set: &BufferSet) -> Result<(), EditorError> {
        let buffer = self.buffer_in(set)?;
        self.end_undo_group();
        let mut reads = Reads::default();
        let (start, cursors) = self.vim_start(buffer, &mut reads, |b, r, s| {
            if !s.is_empty() && !r.is_valid(b, s.anchor) {
                return None;
            }
            m::step_back(b, r, s.head)
        });
        let list = cursors.into_iter().map(Selection::cursor).collect();
        self.selections = normal(list, start.primary_index());
        Ok(())
    }

    /// `u` (or `ctrl-r`, with `redo`) (T-108): undoes or redoes up to
    /// `count` times, stopping when there is nothing left, then turns each
    /// selection into a cursor at its start, settled. An error in the first
    /// step changes nothing. A later one stops the steps and comes back
    /// after the cursors settle.
    pub(crate) fn vim_history(
        &mut self,
        set: &mut BufferSet,
        redo: bool,
        count: Option<usize>,
    ) -> Result<(), EditorError> {
        let mut result = Ok(());
        for step in 0..count.unwrap_or(1).max(1) {
            let done = if redo { self.redo(set) } else { self.undo(set) };
            match done {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(error) if step == 0 => return Err(error),
                Err(error) => {
                    result = Err(error);
                    break;
                }
            }
        }
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, cursors) = self.vim_start(buffer, &mut reads, |b, r, s| {
            if !r.is_valid(b, s.end()) {
                return None;
            }
            m::settle(b, r, s.start())
        });
        let list = cursors.into_iter().map(Selection::cursor).collect();
        self.selections = normal(list, start.primary_index());
        result
    }
}

impl Editor {
    /// `y` with `target` at every cursor (T-109), and `Y`: what each cursor's
    /// range holds, as a register holds it, or `None` if no range covers
    /// anything. Applies no transaction. Each cursor goes to its range's
    /// start, or for a linewise range to its first line at the same
    /// column, settled.
    pub(crate) fn vim_yank(
        &mut self,
        set: &BufferSet,
        target: Target,
        count: Option<usize>,
    ) -> Result<Option<Register>, EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, spans) = self.vim_start(buffer, &mut reads, |buffer, reads, selection| {
            let p = settled_head(buffer, reads, selection)?;
            let span = match target {
                Target::Motion(motion) => m::operator_span(buffer, reads, p, motion, count, false),
                Target::Object(object) => o::span(buffer, reads, p, object, count)
                    .filter(|span| !matches!(span, Span::Chars(range) if range.is_empty())),
            };
            Some((p, span))
        });
        let newline = self.format.newline();
        let register = taken(
            buffer,
            spans.iter().filter_map(|(_, span)| span.as_ref()),
            newline.as_str(),
        );
        let cursors = spans
            .iter()
            .map(|(p, span)| {
                let at = match span {
                    Some(Span::Chars(range)) => range.start,
                    Some(Span::Lines(a, _)) => m::same_column(buffer, &mut reads, *p, *a),
                    None => *p,
                };
                Selection::cursor(m::settle(buffer, &mut reads, at).unwrap_or(at))
            })
            .collect();
        self.selections = normal(cursors, start.primary_index());
        Ok(register)
    }

    /// Visual `y` (T-109): what each selection, or with `lines` each
    /// selection's lines, holds, as a register holds it. Each selection
    /// becomes a cursor at its start, settled.
    pub(crate) fn vim_yank_selections(
        &mut self,
        set: &BufferSet,
        lines: bool,
    ) -> Result<Option<Register>, EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, ends) = self.vim_start(buffer, &mut reads, visual_ends);
        let spans: Vec<Span> = ends
            .iter()
            .map(|&(anchor, cursor)| visual_span(buffer, &mut reads, anchor, cursor, lines))
            .collect();
        let newline = self.format.newline();
        let register = taken(buffer, spans.iter(), newline.as_str());
        let cursors = ends
            .iter()
            .map(|&(anchor, cursor)| {
                let at = anchor.min(cursor);
                Selection::cursor(m::settle(buffer, &mut reads, at).unwrap_or(at))
            })
            .collect();
        self.selections = normal(cursors, start.primary_index());
        Ok(register)
    }

    /// `p` (with `after`) and `P` (T-109): puts `register` `count` times
    /// after or before every cursor, in one transaction, as the `vim`
    /// module docs' "Putting" says.
    pub(crate) fn vim_put(
        &mut self,
        set: &mut BufferSet,
        register: &Register,
        after: bool,
        count: Option<usize>,
    ) -> Result<Option<TransactionId>, EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, heads) = self.vim_start(buffer, &mut reads, settled_head);
        let newline = self.format.newline();
        let Some(texts) = put_texts(register, heads.len(), count, newline.as_str()) else {
            // Too large to put: nothing changes but the clamped cursors.
            self.selections = start;
            return Ok(None);
        };
        let len = buffer.len_bytes();
        let puts: Vec<Put> = heads
            .iter()
            .zip(texts)
            .map(|(&p, text)| {
                if !register.linewise {
                    let at = if after {
                        m::after_char(buffer, &mut reads, p)
                    } else {
                        p
                    };
                    let cursor = put_cursor(0, &text);
                    return Put {
                        range: at..at,
                        text,
                        cursor: (cursor, false),
                    };
                }
                let line = m::line_of(buffer, p);
                let next = m::line_start_of_line(buffer, line + 1);
                if !after {
                    let at = m::line_start_of_line(buffer, line);
                    Put {
                        range: at..at,
                        text,
                        cursor: (0, true),
                    }
                } else if line + 1 < buffer.len_lines() {
                    Put {
                        range: next..next,
                        text,
                        cursor: (0, true),
                    }
                } else {
                    // Below a last line with no line break: the break moves
                    // to the front.
                    // The text stays in its own allocation: `split_off`
                    // copies only the line break, 1 or 2 bytes, into a
                    // small string, and `insert_str` shifts the rest of the
                    // text along to put the break in front.
                    let mut text = text;
                    let body = split_break(&text).0.len();
                    let end: String = text.split_off(body);
                    text.insert_str(0, &end);
                    Put {
                        range: len..len,
                        text,
                        cursor: (end.len(), true),
                    }
                }
            })
            .collect();
        Ok(self.apply_puts(set, start, puts))
    }

    /// Visual `p` and `P` (T-109): replaces each selection, or with `lines`
    /// each selection's lines, with `register`, `count` times, in one
    /// transaction, and, unless `keep`, returns what the selections held,
    /// as a register holds it. With no register, or one too large to put
    /// `count` times (see [`MAX_PUT_BYTES`]), each selection becomes a
    /// cursor on its visual cursor char and nothing else changes.
    pub(crate) fn vim_put_selections(
        &mut self,
        set: &mut BufferSet,
        register: Option<&Register>,
        lines: bool,
        keep: bool,
        count: Option<usize>,
    ) -> Result<Option<Register>, EditorError> {
        let Some(register) = register else {
            self.vim_exit_visual(set)?;
            return Ok(None);
        };
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, ends) = self.vim_start(buffer, &mut reads, visual_ends);
        let newline = self.format.newline();
        let newline = newline.as_str();
        let spans: Vec<Span> = ends
            .iter()
            .map(|&(anchor, cursor)| visual_span(buffer, &mut reads, anchor, cursor, lines))
            .collect();
        let Some(texts) = put_texts(register, ends.len(), count, newline) else {
            self.vim_exit_visual(set)?;
            return Ok(None);
        };
        // `P` keeps the registers, so it reads nothing for them.
        let replaced = if keep {
            None
        } else {
            taken(buffer, spans.iter(), newline)
        };
        let puts: Vec<Put> = spans
            .iter()
            .zip(texts)
            .map(|(span, text)| {
                // A span of lines is replaced with its last line break,
                // which is read on its own: at most two bytes.
                let (range, ends_line) = match span {
                    Span::Lines(a, b) => (
                        m::lines_range(buffer, *a, *b),
                        m::line_break(buffer, &mut reads, *b),
                    ),
                    Span::Chars(range) => (range.clone(), String::new()),
                };
                let (text, cursor) = match (register.linewise, lines) {
                    (true, true) if ends_line.is_empty() => {
                        let body = split_break(&text).0.len();
                        let mut text = text;
                        text.truncate(body);
                        (text, (0, true))
                    }
                    (true, true) => (text, (0, true)),
                    // Linewise text over chars goes on its own line.
                    (true, false) => {
                        let mut text = text;
                        text.insert_str(0, newline);
                        (text, (newline.len(), true))
                    }
                    // Chars over lines get a line break after them.
                    (false, _) => {
                        let cursor = put_cursor(0, &text);
                        let mut text = text;
                        text.push_str(&ends_line);
                        (text, (cursor, false))
                    }
                };
                Put {
                    range,
                    text,
                    cursor,
                }
            })
            .collect();
        self.apply_puts(set, start, puts);
        Ok(replaced)
    }

    /// A text object in a visual mode (T-109), as the `vim` module docs'
    /// "Text objects" say: a one-char selection becomes the object, and a
    /// longer one grows by it from the visual cursor. Where the object
    /// finds nothing, the selection stays.
    pub(crate) fn vim_select_object(
        &mut self,
        set: &BufferSet,
        object: VimObject,
        count: Option<usize>,
    ) -> Result<(), EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, ends) = self.vim_start(buffer, &mut reads, visual_ends);
        let list = start
            .as_slice()
            .iter()
            .zip(&ends)
            .map(|(&selection, &(anchor, cursor))| {
                let grown = if anchor == cursor {
                    o::span(buffer, &mut reads, cursor, object, count)
                        .filter(|span| !matches!(span, Span::Chars(r) if r.is_empty()))
                        .map(|span| o::visual_ends(buffer, &mut reads, &span))
                } else if matches!(object.kind, o::Kind::Pair { .. }) {
                    let ends = (anchor.min(cursor), anchor.max(cursor));
                    o::grow_pair(buffer, &mut reads, ends, object, count)
                } else if matches!(object.kind, o::Kind::Word { .. }) && cursor > anchor {
                    let from = o::after(buffer, &mut reads, cursor);
                    o::word_from(buffer, &mut reads, from, object, count).map(|span| {
                        let (_, last) = o::visual_ends(buffer, &mut reads, &span);
                        (anchor, last.max(cursor))
                    })
                } else {
                    o::span(buffer, &mut reads, cursor, object, count)
                        .filter(|span| !matches!(span, Span::Chars(r) if r.is_empty()))
                        .map(|span| {
                            let (first, last) = o::visual_ends(buffer, &mut reads, &span);
                            let lo = anchor.min(cursor).min(first);
                            let hi = anchor.max(cursor).max(last);
                            if cursor < anchor { (hi, lo) } else { (lo, hi) }
                        })
                };
                match grown {
                    Some((anchor, cursor)) => {
                        let (anchor, head) = m::encode(buffer, &mut reads, anchor, cursor);
                        Selection::new(anchor, head)
                    }
                    None => selection,
                }
            })
            .collect();
        self.selections = normal(list, start.primary_index());
        Ok(())
    }

    /// Applies `puts`, one per selection of `start` in order, as one
    /// transaction, and puts each cursor where its put says. Puts whose
    /// ranges overlap an earlier one are dropped, and their cursors go where
    /// that one's does.
    fn apply_puts(
        &mut self,
        set: &mut BufferSet,
        start: Selections,
        puts: Vec<Put>,
    ) -> Option<TransactionId> {
        let primary = start.primary_index();
        let mut transaction = Transaction::new();
        // Each put's start in the text after the edit, and whether its
        // cursor is the first non-blank of a line.
        let mut places: Vec<(usize, bool)> = Vec::with_capacity(puts.len());
        let mut shift: isize = 0;
        let mut last: Option<(Range<usize>, usize, bool)> = None;
        let mut changed = false;
        for put in puts {
            if let Some((range, at, lines)) = &last
                && (put.range.start < range.end || put.range == *range)
            {
                places.push((*at, *lines));
                continue;
            }
            let new_start = (put.range.start as isize + shift) as usize;
            let at = new_start + put.cursor.0;
            shift += put.text.len() as isize - put.range.len() as isize;
            changed |= !put.text.is_empty() || !put.range.is_empty();
            if !(put.text.is_empty() && put.range.is_empty()) {
                transaction.push(Edit::replace(self.buffer, put.range.clone(), put.text));
            }
            places.push((at, put.cursor.1));
            last = Some((put.range, at, put.cursor.1));
        }
        let moved = normal(
            places
                .iter()
                .map(|&(at, _)| Selection::cursor(at))
                .collect(),
            primary,
        );
        if !changed {
            self.selections = start;
            return None;
        }
        let id = self.apply(set, transaction, start, moved);
        // The cursors settle, and linewise ones go to their line's first
        // non-blank, in the text after the edit.
        let buffer = set.get(self.buffer).expect("the buffer was just edited");
        let mut reads = Reads::default();
        let cursors = places
            .iter()
            .map(|&(at, lines)| {
                let at = if lines {
                    m::first_non_blank(buffer, &mut reads, m::line_of(buffer, at))
                } else {
                    at
                };
                Selection::cursor(m::settle(buffer, &mut reads, at).unwrap_or(at))
            })
            .collect();
        let after = normal(cursors, primary);
        if let Some(record) = self.history.get_mut(&id) {
            record.after = after.clone();
        }
        self.selections = after;
        Some(id)
    }
}

/// What a visual selection with these ends covers: its chars, or with
/// `lines` its lines.
fn visual_span(
    buffer: &Buffer,
    reads: &mut Reads,
    anchor: usize,
    cursor: usize,
    lines: bool,
) -> Span {
    if lines {
        let (a, b) = (m::line_of(buffer, anchor), m::line_of(buffer, cursor));
        return Span::Lines(a.min(b), a.max(b));
    }
    let (from, to) = m::encode(buffer, reads, anchor, cursor);
    Span::Chars(from.min(to)..from.max(to))
}

/// The most bytes one put may insert, over all its cursors (T-109): 1 GiB.
/// A put that would insert more, such as `99999999999999999999p`, puts
/// nothing, as Vim refuses a put whose text is too long (E1240). Past this
/// size the allocation alone would take seconds or fail.
const MAX_PUT_BYTES: usize = 1 << 30;

/// The text each of `cursors` cursors puts from `register`, `count` times
/// over, one owned string per cursor: its own piece when there are as many
/// pieces as cursors, and otherwise every piece joined, as
/// [`Register::text`] joins them. `None` if all of it together would be
/// more than [`MAX_PUT_BYTES`], or too large to count; then nothing is
/// built.
fn put_texts(
    register: &Register,
    cursors: usize,
    count: Option<usize>,
    newline: &str,
) -> Option<Vec<String>> {
    let n = count.unwrap_or(1).max(1);
    let fits = |total: Option<usize>| total.filter(|&total| total <= MAX_PUT_BYTES);
    if register.pieces.len() == cursors {
        let total = register
            .pieces
            .iter()
            .try_fold(0usize, |sum, piece| sum.checked_add(piece.len()));
        fits(total.and_then(|total| total.checked_mul(n)))?;
        return Some(
            register
                .pieces
                .iter()
                .map(|piece| piece.repeat(n))
                .collect(),
        );
    }
    let joins = if register.linewise {
        0
    } else {
        register.pieces.len().saturating_sub(1)
    };
    let one = register
        .pieces
        .iter()
        .try_fold(0usize, |sum, piece| sum.checked_add(piece.len()))
        .and_then(|sum| sum.checked_add(joins.checked_mul(newline.len())?));
    fits(
        one.and_then(|one| one.checked_mul(n))
            .and_then(|each| each.checked_mul(cursors.max(1))),
    )?;
    let mut text = register.text(newline);
    if n > 1 {
        text = text.repeat(n);
    }
    // Every cursor but the last gets a copy; the last takes the text.
    let mut texts = vec![text.clone(); cursors.saturating_sub(1)];
    texts.push(text);
    Some(texts)
}
