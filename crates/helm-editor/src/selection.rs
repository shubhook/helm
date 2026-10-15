//! `Selection` and `Selections`: anchors and heads in byte offsets, kept
//! sorted and merged.

use std::ops::Range;

/// One selection: an anchor and a head, both byte offsets into a buffer's
/// text. A cursor is a selection whose anchor equals its head.
///
/// A `Selection` on its own is plain data and checks nothing. The offsets
/// are checked against a buffer only when an [`Editor`](crate::Editor)
/// command uses them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Selection {
    /// The end that stays put when the selection grows.
    pub anchor: usize,
    /// The end the cursor is drawn at, which moves when the selection grows.
    pub head: usize,
}

impl Selection {
    /// A selection from `anchor` to `head`. `head < anchor` is a selection
    /// that points left.
    pub const fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// A cursor at byte offset `offset`: an empty selection.
    pub const fn cursor(offset: usize) -> Self {
        Self::new(offset, offset)
    }

    /// The lower of the anchor and the head.
    pub const fn start(&self) -> usize {
        if self.anchor < self.head {
            self.anchor
        } else {
            self.head
        }
    }

    /// The higher of the anchor and the head.
    pub const fn end(&self) -> usize {
        if self.anchor < self.head {
            self.head
        } else {
            self.anchor
        }
    }

    /// The selected bytes, `start()..end()`. Empty for a cursor.
    pub const fn range(&self) -> Range<usize> {
        self.start()..self.end()
    }

    /// Whether this is a cursor: the anchor equals the head.
    pub const fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Whether the selection points left: the head is before the anchor.
    pub const fn is_reversed(&self) -> bool {
        self.head < self.anchor
    }
}

/// One or more selections in normal form, one of them the primary.
///
/// There is always at least one selection. Every constructor and every
/// method that changes the selections leaves them in this form:
///
/// - **Sorted.** By [`Selection::start`], lowest first.
/// - **Merged.** Two of the given selections `a` and `b`, with
///   `a.start() <= b.start()`, are joined when `b.start() < a.end()` (they
///   overlap), or when `b.start() == a.end()` and at least one of them is
///   empty (they touch). Joining is transitive: every group of selections
///   linked by joins becomes one selection. So a cursor where two
///   selections touch joins all three. Two non-empty selections that only
///   touch, with no cursor between them, stay apart.
/// - **One merged selection** runs from the lowest start to the highest end
///   of the selections merged into it. It points the way the primary does
///   if the primary is one of them and isn't empty. Otherwise it points the
///   way the first non-empty one among them does, in the order they were
///   given: the input order for [`new`](Self::new), and for
///   [`push`](Self::push) the existing selections in order, then the new
///   one. If all of them are empty, they were all the same cursor, and so
///   is the result.
/// - **Primary.** The selection the primary merged into is the primary.
///
/// Equality compares the selections and which one is primary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selections {
    /// In normal form, never empty.
    selections: Vec<Selection>,
    /// Index of the primary in `selections`.
    primary: usize,
}

impl Selections {
    /// One selection, which is the primary.
    pub fn single(selection: Selection) -> Self {
        Self {
            selections: vec![selection],
            primary: 0,
        }
    }

    /// `selections` in normal form, with `selections[primary]` as the
    /// primary before merging. The input can be in any order and can
    /// overlap.
    ///
    /// Returns `None` if `selections` is empty or `primary` is not an index
    /// into it.
    pub fn new(selections: Vec<Selection>, primary: usize) -> Option<Self> {
        if primary >= selections.len() {
            return None;
        }
        Some(normalize(&selections, primary))
    }

    /// Adds `selection`, puts the set back in normal form, and makes the
    /// selection it ends up in the primary.
    pub fn push(&mut self, selection: Selection) {
        let mut selections = std::mem::take(&mut self.selections);
        selections.push(selection);
        *self = normalize(&selections, selections.len() - 1);
    }

    /// The selections in normal form: sorted by start, none overlapping.
    pub fn as_slice(&self) -> &[Selection] {
        &self.selections
    }

    /// The primary selection.
    pub fn primary(&self) -> Selection {
        self.selections[self.primary]
    }

    /// The index of the primary in [`as_slice`](Self::as_slice).
    pub fn primary_index(&self) -> usize {
        self.primary
    }
}

/// `input` in normal form, with `input[primary]` as the primary before
/// merging. `primary` must be an index into `input`.
fn normalize(input: &[Selection], primary: usize) -> Selections {
    // Input indexes sorted by start, cursors first at the same start. Then a
    // cursor that joins two touching selections is seen before the second
    // one, and one sweep finds every group.
    let mut order: Vec<usize> = (0..input.len()).collect();
    order.sort_by_key(|&i| (input[i].start(), !input[i].is_empty(), i));

    let mut selections: Vec<Selection> = Vec::new();
    let mut new_primary = 0;
    let mut group = Group::start(input, order[0], primary);
    for &i in &order[1..] {
        let selection = input[i];
        let joins = selection.start() < group.end
            || (selection.start() == group.end && (selection.is_empty() || group.cursor_at_end));
        if joins {
            group.add(input, i, primary);
        } else {
            if group.has_primary {
                new_primary = selections.len();
            }
            selections.push(group.finish(input, primary));
            group = Group::start(input, i, primary);
        }
    }
    if group.has_primary {
        new_primary = selections.len();
    }
    selections.push(group.finish(input, primary));

    Selections {
        selections,
        primary: new_primary,
    }
}

/// The selections joined so far into one, while [`normalize`] sweeps.
struct Group {
    start: usize,
    end: usize,
    /// Whether a cursor sits at `end`, so a non-empty selection starting
    /// there joins too.
    cursor_at_end: bool,
    /// Whether the primary is in the group.
    has_primary: bool,
    /// The lowest input index of a non-empty selection in the group.
    first_non_empty: Option<usize>,
}

impl Group {
    fn start(input: &[Selection], i: usize, primary: usize) -> Self {
        let selection = input[i];
        Self {
            start: selection.start(),
            end: selection.end(),
            cursor_at_end: selection.is_empty(),
            has_primary: i == primary,
            first_non_empty: (!selection.is_empty()).then_some(i),
        }
    }

    fn add(&mut self, input: &[Selection], i: usize, primary: usize) {
        let selection = input[i];
        if selection.end() > self.end {
            self.end = selection.end();
            self.cursor_at_end = false;
        } else if selection.is_empty() && selection.start() == self.end {
            self.cursor_at_end = true;
        }
        self.has_primary |= i == primary;
        if !selection.is_empty() {
            self.first_non_empty = Some(self.first_non_empty.map_or(i, |first| first.min(i)));
        }
    }

    /// The joined selection, pointing the way the primary does if it is
    /// here and not empty, else the way the first non-empty one given does.
    fn finish(&self, input: &[Selection], primary: usize) -> Selection {
        let leader = if self.has_primary && !input[primary].is_empty() {
            Some(primary)
        } else {
            self.first_non_empty
        };
        match leader {
            Some(i) if input[i].is_reversed() => Selection::new(self.end, self.start),
            _ => Selection::new(self.start, self.end),
        }
    }
}
