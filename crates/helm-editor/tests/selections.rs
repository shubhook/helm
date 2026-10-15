//! T-105 acceptance tests: `Selection` and the normal form of `Selections`:
//! sorted, merged when they overlap or when a cursor touches a selection,
//! and one primary.

use helm_editor::{Selection, Selections};

/// A selection from `anchor` to `head`.
fn s(anchor: usize, head: usize) -> Selection {
    Selection::new(anchor, head)
}

/// A cursor at `offset`.
fn c(offset: usize) -> Selection {
    Selection::cursor(offset)
}

/// `Selections::new`, which must accept the input.
fn sels(selections: &[Selection], primary: usize) -> Selections {
    Selections::new(selections.to_vec(), primary).expect("non-empty, primary in range")
}

/// Asserts the selections and the primary's index.
fn assert_sels(actual: &Selections, expected: &[Selection], primary: usize) {
    assert_eq!(actual.as_slice(), expected);
    assert_eq!(actual.primary_index(), primary);
    assert_eq!(actual.primary(), expected[primary]);
}

#[test]
fn selection_accessors() {
    let left = s(5, 2);
    assert_eq!((left.start(), left.end(), left.range()), (2, 5, 2..5));
    assert!(left.is_reversed());
    assert!(!left.is_empty());

    let right = s(2, 5);
    assert_eq!(right.range(), 2..5);
    assert!(!right.is_reversed());

    let cursor = c(3);
    assert_eq!(cursor, s(3, 3));
    assert_eq!(cursor.range(), 3..3);
    assert!(cursor.is_empty());
    assert!(!cursor.is_reversed());

    assert_eq!(sels(&[left], 0), Selections::single(left));
    assert_sels(&sels(&[left], 0), &[left], 0);
}

#[test]
fn new_rejects_empty_input_and_bad_primary() {
    assert_eq!(Selections::new(Vec::new(), 0), None);
    assert_eq!(Selections::new(vec![c(1)], 1), None);
    assert_eq!(Selections::new(vec![c(1), c(2)], usize::MAX), None);
}

#[test]
fn new_sorts_by_start() {
    let set = sels(&[c(9), s(2, 4), c(0)], 0);
    assert_sels(&set, &[c(0), s(2, 4), c(9)], 2);

    let set = sels(&[s(8, 6), s(1, 3), c(4)], 1);
    assert_sels(&set, &[s(1, 3), c(4), s(8, 6)], 0);
}

#[test]
fn equal_cursors_merge() {
    let set = sels(&[c(3), c(3), c(1)], 1);
    assert_sels(&set, &[c(1), c(3)], 1);

    let set = sels(&[c(7), c(7), c(7)], 2);
    assert_sels(&set, &[c(7)], 0);
}

#[test]
fn overlapping_selections_merge() {
    assert_sels(&sels(&[s(0, 4), s(2, 6)], 0), &[s(0, 6)], 0);
    // One inside another.
    assert_sels(&sels(&[s(0, 10), s(3, 5)], 1), &[s(0, 10)], 0);
    // A cursor strictly inside a selection.
    assert_sels(&sels(&[s(0, 10), c(5)], 1), &[s(0, 10)], 0);
    // Equal ranges.
    assert_sels(&sels(&[s(2, 4), s(2, 4), c(9)], 2), &[s(2, 4), c(9)], 1);
}

#[test]
fn a_cursor_touching_a_selection_merges() {
    // At its end.
    assert_sels(&sels(&[s(2, 5), c(5)], 1), &[s(2, 5)], 0);
    // At its start.
    assert_sels(&sels(&[c(5), s(5, 8)], 0), &[s(5, 8)], 0);
    // The merged selection keeps the non-empty one's direction, even when
    // the cursor was the primary.
    assert_sels(&sels(&[s(5, 2), c(2)], 1), &[s(5, 2)], 0);
    assert_sels(&sels(&[c(8), s(8, 5)], 0), &[s(8, 5)], 0);
}

#[test]
fn non_empty_selections_that_only_touch_stay_apart() {
    let set = sels(&[s(1, 2), s(0, 1), s(2, 3)], 0);
    assert_sels(&set, &[s(0, 1), s(1, 2), s(2, 3)], 1);

    let set = sels(&[s(4, 2), s(4, 6)], 1);
    assert_sels(&set, &[s(4, 2), s(4, 6)], 1);
}

#[test]
fn a_cursor_where_two_selections_touch_joins_all_three() {
    assert_sels(&sels(&[s(2, 5), s(5, 8), c(5)], 0), &[s(2, 8)], 0);
    assert_sels(&sels(&[c(5), s(8, 5), s(5, 2)], 0), &[s(8, 2)], 0);
    // Without the cursor they stay apart.
    assert_sels(&sels(&[s(2, 5), s(5, 8)], 1), &[s(2, 5), s(5, 8)], 1);
}

#[test]
fn merging_chains_through_several_selections() {
    // 0..3 and 2..5 overlap, the result overlaps 4..8, and the cursor at 8
    // touches that. 9..10 stays apart.
    let set = sels(&[s(0, 3), s(2, 5), s(4, 8), c(8), s(9, 10)], 4);
    assert_sels(&set, &[s(0, 8), s(9, 10)], 1);

    // Given in another order, the result is the same.
    let set = sels(&[c(8), s(9, 10), s(4, 8), s(2, 5), s(0, 3)], 1);
    assert_sels(&set, &[s(0, 8), s(9, 10)], 1);
}

#[test]
fn merged_direction_follows_the_primary_then_input_order() {
    // The primary is in the group and not empty: its direction wins.
    assert_sels(&sels(&[s(0, 3), s(5, 2)], 1), &[s(5, 0)], 0);
    assert_sels(&sels(&[s(3, 0), s(2, 5)], 1), &[s(0, 5)], 0);

    // The primary is elsewhere: the first non-empty one given wins.
    assert_sels(&sels(&[s(3, 0), s(2, 5), c(20)], 2), &[s(5, 0), c(20)], 1);
    assert_sels(&sels(&[c(20), s(2, 5), s(3, 0)], 0), &[s(0, 5), c(20)], 1);

    // The primary is in the group but empty: the first non-empty one given
    // wins.
    assert_sels(&sels(&[c(3), s(4, 1), s(2, 6)], 0), &[s(6, 1)], 0);
}

#[test]
fn push_adds_a_selection_and_makes_it_primary() {
    let mut set = Selections::single(c(5));
    set.push(c(1));
    assert_sels(&set, &[c(1), c(5)], 0);

    // 4..6 swallows the cursor at 5.
    set.push(s(4, 6));
    assert_sels(&set, &[c(1), s(4, 6)], 1);

    // Pushing an existing cursor only moves the primary.
    set.push(c(1));
    assert_sels(&set, &[c(1), s(4, 6)], 0);

    // A cursor touching a reversed selection keeps it reversed.
    let mut set = Selections::single(s(9, 3));
    set.push(c(9));
    assert_sels(&set, &[s(9, 3)], 0);

    // A non-empty selection pushed into a group sets its direction.
    let mut set = sels(&[s(0, 3), c(10)], 1);
    set.push(s(5, 2));
    assert_sels(&set, &[s(5, 0), c(10)], 0);
}

#[test]
fn offsets_are_not_checked() {
    // `Selections` is plain data: offsets are checked against a buffer only
    // when an editor command uses them.
    let set = sels(&[c(usize::MAX), s(usize::MAX, 0)], 0);
    assert_sels(&set, &[s(usize::MAX, 0)], 0);
}
