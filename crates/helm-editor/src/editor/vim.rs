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
use crate::motion::vim::{self as m, Span, VimMotion};
use crate::{EditorError, Selection, Selections};

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
        let (start, moved) = self.vim_start_with(
            buffer,
            &mut reads,
            vertical,
            |buffer, reads, selection, goal| {
                let p = settled_head(buffer, reads, selection)?;
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
        let (start, moved) = self.vim_start_with(
            buffer,
            &mut reads,
            vertical,
            |buffer, reads, selection, goal| {
                let (anchor, cursor) = visual_ends(buffer, reads, selection)?;
                let (to, goal) = match m::target(buffer, reads, cursor, motion, count, goal) {
                    Some((to, goal)) => (m::vsettle(buffer, reads, to).unwrap_or(to), goal),
                    None => (cursor, goal.unwrap_or_else(|| m::goal_at(buffer, cursor))),
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
        motion: VimMotion,
        count: Option<usize>,
    ) -> Result<Option<TransactionId>, EditorError> {
        let change = operator == Operator::Change;
        self.vim_edit(set, change, true, |buffer, reads, selection| {
            let p = settled_head(buffer, reads, selection)?;
            Some((p, m::operator_span(buffer, reads, p, motion, count, change)))
        })
    }

    /// Visual `d` and `c` (T-108): deletes or changes each selection, or,
    /// with `lines`, each selection's lines.
    pub(crate) fn vim_edit_selections(
        &mut self,
        set: &mut BufferSet,
        operator: Operator,
        lines: bool,
    ) -> Result<Option<TransactionId>, EditorError> {
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
    fn vim_edit(
        &mut self,
        set: &mut BufferSet,
        change: bool,
        at_homes: bool,
        piece: impl FnMut(&Buffer, &mut Reads, Selection) -> Option<Piece>,
    ) -> Result<Option<TransactionId>, EditorError> {
        let buffer = self.buffer_in(set)?;
        let mut reads = Reads::default();
        let (start, pieces) = self.vim_start(buffer, &mut reads, piece);
        let primary = start.primary_index();
        let before = if at_homes {
            let homes = pieces.iter().map(|&(home, _)| Selection::cursor(home));
            normal(homes.collect(), primary)
        } else {
            start
        };

        // Each selection's byte range, and whether it is linewise.
        let mut ranges: Vec<Option<(Range<usize>, bool)>> = pieces
            .iter()
            .map(|(_, span)| match span {
                Some(Span::Chars(range)) => Some((range.clone(), false)),
                _ => None,
            })
            .collect();
        let mut spans: Vec<(usize, usize, usize)> = pieces
            .iter()
            .enumerate()
            .filter_map(|(i, (_, span))| match span {
                Some(Span::Lines(a, b)) => Some((*a, *b, i)),
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
            let range = if change {
                m::change_lines(buffer, &mut reads, a, b)
            } else {
                m::delete_lines(buffer, &mut reads, a, b)
            };
            for &(_, _, i) in &spans[k..j] {
                ranges[i] = Some((range.clone(), true));
            }
            k = j;
        }

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
        let places: Vec<(usize, bool)> = pieces
            .iter()
            .zip(&ranges)
            .map(|((home, _), range)| {
                let (at, linewise) = match range {
                    Some((range, linewise)) => (range.start, *linewise),
                    None => (*home, false),
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
            return Ok(id);
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
        Ok(id)
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
