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
        todo!("T-105: sort and merge into normal form")
    }

    /// Adds `selection`, puts the set back in normal form, and makes the
    /// selection it ends up in the primary.
    pub fn push(&mut self, selection: Selection) {
        todo!("T-105: add one selection and make it primary")
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
