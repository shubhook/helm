//! T-102 acceptance tests: applying transactions to one buffer and to
//! several, how edit offsets are read, and rejected transactions changing
//! nothing.

use std::error::Error;
use std::ops::Range;

use helm_text::{
    Buffer, BufferId, BufferSet, Edit, TextBuffer, TextError, Transaction, TransactionError,
};

/// A set holding one buffer per text, and their ids in the same order.
fn set_of(texts: &[&str]) -> (BufferSet, Vec<BufferId>) {
    let mut set = BufferSet::new();
    let ids = texts.iter().map(|&t| set.add(Buffer::from(t))).collect();
    (set, ids)
}

/// The text of buffer `id`.
fn text(set: &BufferSet, id: BufferId) -> String {
    set.get(id).expect("buffer is in the set").to_string()
}

/// Edits on buffer `id`, each replacing a range with a text.
fn edits(id: BufferId, list: &[(Range<usize>, &str)]) -> Vec<Edit> {
    list.iter()
        .map(|(range, text)| Edit::replace(id, range.clone(), *text))
        .collect()
}

/// Applies `edits` as one transaction.
fn apply(set: &mut BufferSet, edits: Vec<Edit>) -> Result<(), TransactionError> {
    set.apply(Transaction::from(edits)).map(drop)
}

/// A list of `(range, text)` edits on one buffer.
type EditList<'a> = &'a [(Range<usize>, &'a str)];

/// A range that starts after it ends. Bound to names so clippy's
/// `reversed_empty_ranges` doesn't reject the literal.
const REVERSED: (usize, usize) = (3, 1);

/// a(0) é(1..3) 中(3..6) 😀(6..10). 10 bytes.
const WIDE: &str = "aé中😀";

#[test]
#[ignore = "T-102: not implemented yet"]
fn added_buffers_keep_their_text() {
    let (set, ids) = set_of(&["", "hello", "a\r\nb"]);
    assert_eq!(text(&set, ids[0]), "");
    assert_eq!(text(&set, ids[1]), "hello");
    assert_eq!(text(&set, ids[2]), "a\r\nb");
    assert!(ids[0] < ids[1] && ids[1] < ids[2], "ids increase");

    let mut set = BufferSet::default();
    let id = set.add(Buffer::new());
    assert_eq!(text(&set, id), "");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn edit_constructors() {
    let (_, ids) = set_of(&["abc"]);
    let a = ids[0];
    assert_eq!(Edit::insert(a, 3, "x"), Edit::replace(a, 3..3, "x"));
    assert_eq!(Edit::delete(a, 1..2), Edit::replace(a, 1..2, ""));
    let edit = Edit::replace(a, 0..1, String::from("z"));
    assert_eq!(
        (edit.buffer, edit.range, edit.text.as_str()),
        (a, 0..1, "z")
    );

    let mut built = Transaction::new();
    built.push(Edit::insert(a, 0, "x"));
    built.push(Edit::delete(a, 1..2));
    let collected: Transaction = [Edit::insert(a, 0, "x"), Edit::delete(a, 1..2)]
        .into_iter()
        .collect();
    assert_eq!(built, collected);
    assert_eq!(built.edits().len(), 2);
    assert_eq!(built.edits()[1], Edit::delete(a, 1..2));
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn single_buffer_edits() {
    let (mut set, ids) = set_of(&["hello world"]);
    let a = ids[0];

    apply(&mut set, vec![Edit::insert(a, 5, ",")]).unwrap();
    assert_eq!(text(&set, a), "hello, world");

    apply(&mut set, vec![Edit::delete(a, 0..7)]).unwrap();
    assert_eq!(text(&set, a), "world");

    apply(&mut set, vec![Edit::replace(a, 0..5, "there")]).unwrap();
    assert_eq!(text(&set, a), "there");

    apply(&mut set, edits(a, &[(5..5, "\r\n"), (0..0, "hi ")])).unwrap();
    assert_eq!(text(&set, a), "hi there\r\n");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn apply_returns_increasing_ids() {
    let (mut set, ids) = set_of(&["", "x"]);
    let t1 = set.apply(edits(ids[0], &[(0..0, "a")]).into()).unwrap();
    let t2 = set.apply(edits(ids[1], &[(0..0, "b")]).into()).unwrap();
    let both = vec![Edit::insert(ids[0], 0, "c"), Edit::insert(ids[1], 0, "d")];
    let t3 = set.apply(both.into()).unwrap();
    assert!(t1 < t2 && t2 < t3, "{t1:?} < {t2:?} < {t3:?}");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn offsets_refer_to_the_text_before_the_transaction() {
    // Read against the old text, these remove "ab" and "cd". Read one after
    // another, the second would remove "ef" instead.
    let (mut set, ids) = set_of(&["abcdef"]);
    apply(&mut set, edits(ids[0], &[(0..2, ""), (2..4, "")])).unwrap();
    assert_eq!(text(&set, ids[0]), "ef");

    // Every listing order gives the same result.
    let list = [(0..0, "A"), (6..11, ""), (11..11, "!")];
    for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
        let (mut set, ids) = set_of(&["hello world"]);
        let listed: Vec<_> = order.iter().map(|&i| list[i].clone()).collect();
        apply(&mut set, edits(ids[0], &listed)).unwrap();
        assert_eq!(text(&set, ids[0]), "Ahello !", "order {order:?}");
    }

    // An offset that is valid only once an earlier edit has lengthened the
    // text is rejected.
    let (mut set, ids) = set_of(&["xy"]);
    let err = apply(&mut set, edits(ids[0], &[(0..0, "abc"), (3..4, "")]));
    assert_eq!(
        err,
        Err(TransactionError::InvalidRange {
            edit: 1,
            error: TextError::ByteOutOfBounds { offset: 4, len: 2 },
        })
    );
    assert_eq!(text(&set, ids[0]), "xy");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn inserts_at_one_offset_keep_their_order() {
    let (mut set, ids) = set_of(&["ac", "ac"]);
    apply(&mut set, edits(ids[0], &[(1..1, "x"), (1..1, "y")])).unwrap();
    apply(&mut set, edits(ids[1], &[(1..1, "y"), (1..1, "x")])).unwrap();
    assert_eq!(text(&set, ids[0]), "axyc");
    assert_eq!(text(&set, ids[1]), "ayxc");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn inserts_at_the_ends_of_a_replaced_range() {
    // An insert at the start of the range lands before the replacement, one
    // at its end after it, whatever the listing order.
    let orders: [EditList; 2] = [
        &[(1..4, "X"), (1..1, "<"), (4..4, ">")],
        &[(4..4, ">"), (1..1, "<"), (1..4, "X")],
    ];
    for list in orders {
        let (mut set, ids) = set_of(&["abcdef"]);
        apply(&mut set, edits(ids[0], list)).unwrap();
        assert_eq!(text(&set, ids[0]), "a<X>ef", "{list:?}");
    }

    // At one start, every insert comes first, in transaction order, then the
    // deleted range.
    let (mut set, ids) = set_of(&["abcdef"]);
    apply(
        &mut set,
        edits(ids[0], &[(1..4, ""), (1..1, "<"), (1..1, "[")]),
    )
    .unwrap();
    assert_eq!(text(&set, ids[0]), "a<[ef");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn edits_across_wide_characters_and_line_breaks() {
    // é(0..2) \r(2) \n(3) 中(4..7) 😀(7..11).
    let (mut set, ids) = set_of(&["é\r\n中😀"]);
    let list = [(3..3, "|"), (4..7, "zh"), (7..11, ""), (0..0, "^")];
    apply(&mut set, edits(ids[0], &list)).unwrap();
    // The insert between CR and LF splits the CRLF into two line breaks.
    assert_eq!(text(&set, ids[0]), "^é\r|\nzh");
    assert_eq!(set.get(ids[0]).unwrap().len_lines(), 3);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn one_transaction_edits_several_buffers() {
    // A rename across two files, listed out of buffer order. The third
    // buffer isn't touched.
    let (mut set, ids) = set_of(&["fn old() {}\n", "old();\nold();\n", "old"]);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let rename = vec![
        Edit::replace(b, 7..10, "new"),
        Edit::replace(a, 3..6, "new"),
        Edit::replace(b, 0..3, "new"),
    ];
    apply(&mut set, rename).unwrap();
    assert_eq!(text(&set, a), "fn new() {}\n");
    assert_eq!(text(&set, b), "new();\nnew();\n");
    assert_eq!(text(&set, c), "old");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn equal_ranges_in_different_buffers_do_not_overlap() {
    let (mut set, ids) = set_of(&["abc", "abc"]);
    let both = vec![
        Edit::replace(ids[0], 1..2, "X"),
        Edit::replace(ids[1], 1..2, "Y"),
    ];
    apply(&mut set, both).unwrap();
    assert_eq!(text(&set, ids[0]), "aXc");
    assert_eq!(text(&set, ids[1]), "aYc");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn edits_that_change_nothing_still_apply() {
    let (mut set, ids) = set_of(&["abc"]);
    let a = ids[0];
    set.apply(edits(a, &[(1..1, ""), (2..2, "")]).into())
        .unwrap();
    assert_eq!(text(&set, a), "abc");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn empty_transaction_is_rejected() {
    let (mut set, ids) = set_of(&["abc"]);
    assert_eq!(set.apply(Transaction::new()), Err(TransactionError::Empty));
    assert_eq!(set.apply(Vec::new().into()), Err(TransactionError::Empty));
    assert_eq!(text(&set, ids[0]), "abc");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn unknown_buffer_is_rejected() {
    let (mut set, ids) = set_of(&["abc"]);
    let a = ids[0];
    // The second id of a set with two buffers isn't in a set with one.
    let (_, other) = set_of(&["x", "y"]);
    let unknown = other[1];

    let err = apply(
        &mut set,
        vec![Edit::insert(a, 0, "x"), Edit::insert(unknown, 0, "y")],
    );
    assert_eq!(
        err,
        Err(TransactionError::UnknownBuffer {
            edit: 1,
            buffer: unknown,
        })
    );
    assert_eq!(text(&set, a), "abc");
    assert!(set.get(unknown).is_none());
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn invalid_ranges_are_rejected() {
    let (start, end) = REVERSED;
    let cases = [
        (start..end, TextError::InvalidRange { start, end }),
        (
            0..11,
            TextError::ByteOutOfBounds {
                offset: 11,
                len: 10,
            },
        ),
        (
            11..11,
            TextError::ByteOutOfBounds {
                offset: 11,
                len: 10,
            },
        ),
        (
            usize::MAX..usize::MAX,
            TextError::ByteOutOfBounds {
                offset: usize::MAX,
                len: 10,
            },
        ),
        // The end is checked against the length before either end is
        // checked for a char boundary.
        (
            2..12,
            TextError::ByteOutOfBounds {
                offset: 12,
                len: 10,
            },
        ),
        (2..3, TextError::NotCharBoundary { offset: 2 }),
        (1..2, TextError::NotCharBoundary { offset: 2 }),
        (4..5, TextError::NotCharBoundary { offset: 4 }),
        (7..7, TextError::NotCharBoundary { offset: 7 }),
    ];
    for (range, error) in cases {
        let (mut set, ids) = set_of(&[WIDE]);
        let a = ids[0];
        // The range check matches what `slice` says about the same range.
        assert_eq!(set.get(a).unwrap().slice(range.clone()), Err(error));

        let err = apply(
            &mut set,
            vec![
                Edit::insert(a, 0, "ok"),
                Edit::replace(a, range.clone(), "x"),
            ],
        );
        assert_eq!(
            err,
            Err(TransactionError::InvalidRange { edit: 1, error }),
            "{range:?}"
        );
        assert_eq!(text(&set, a), WIDE, "{range:?}");

        let err = err.unwrap_err();
        let source = err.source().expect("an invalid range has a source");
        assert_eq!(source.to_string(), error.to_string());
    }
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn overlapping_edits_are_rejected() {
    let overlap = |edit, other| Err(TransactionError::Overlap { edit, other });
    let cases: [(EditList, _); 5] = [
        (&[(1..4, ""), (3..5, "")], overlap(1, 0)),
        (&[(1..4, ""), (1..4, "x")], overlap(1, 0)),
        (&[(1..2, ""), (1..3, "x")], overlap(1, 0)),
        // An insert strictly inside a replaced range overlaps it.
        (&[(1..4, ""), (2..2, "x")], overlap(1, 0)),
        // The error names the earliest edit the bad one overlaps.
        (
            &[(0..1, ""), (2..4, ""), (5..6, ""), (3..6, "")],
            overlap(3, 1),
        ),
    ];
    for (list, want) in cases {
        let (mut set, ids) = set_of(&["abcdef"]);
        assert_eq!(apply(&mut set, edits(ids[0], list)), want, "{list:?}");
        assert_eq!(text(&set, ids[0]), "abcdef", "{list:?}");
    }

    // Ranges that only touch don't overlap, and neither do empty ranges at
    // one offset.
    let (mut set, ids) = set_of(&["abcdef"]);
    let list = [(1..3, ""), (3..5, ""), (5..5, "x"), (5..5, "")];
    apply(&mut set, edits(ids[0], &list)).unwrap();
    assert_eq!(text(&set, ids[0]), "axf");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn the_first_failing_edit_is_reported() {
    let (_, other) = set_of(&["x", "y"]);
    let unknown = other[1];
    let (start, end) = REVERSED;

    let (mut set, ids) = set_of(&["abcdef"]);
    let a = ids[0];

    // Edit 1 fails before edit 2 is looked at.
    let err = apply(
        &mut set,
        vec![
            Edit::insert(a, 0, "x"),
            Edit::delete(a, 0..99),
            Edit::insert(unknown, 0, ""),
        ],
    );
    assert_eq!(
        err,
        Err(TransactionError::InvalidRange {
            edit: 1,
            error: TextError::ByteOutOfBounds { offset: 99, len: 6 },
        })
    );

    // Within one edit, the buffer is checked before the range.
    let err = apply(&mut set, vec![Edit::delete(unknown, start..end)]);
    assert_eq!(
        err,
        Err(TransactionError::UnknownBuffer {
            edit: 0,
            buffer: unknown,
        })
    );

    // And the range before overlaps.
    let err = apply(&mut set, edits(a, &[(1..4, ""), (2..99, "")]));
    assert_eq!(
        err,
        Err(TransactionError::InvalidRange {
            edit: 1,
            error: TextError::ByteOutOfBounds { offset: 99, len: 6 },
        })
    );
    assert_eq!(text(&set, a), "abcdef");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_failed_transaction_changes_no_buffer() {
    let (mut set, ids) = set_of(&["one", "two", "three"]);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let err = apply(
        &mut set,
        vec![
            Edit::replace(a, 0..3, "1"),
            Edit::replace(b, 0..3, "2"),
            Edit::delete(c, 2..99),
        ],
    );
    assert_eq!(
        err,
        Err(TransactionError::InvalidRange {
            edit: 2,
            error: TextError::ByteOutOfBounds { offset: 99, len: 5 },
        })
    );
    assert_eq!(text(&set, a), "one");
    assert_eq!(text(&set, b), "two");
    assert_eq!(text(&set, c), "three");

    // The set still works afterwards.
    apply(
        &mut set,
        vec![Edit::replace(a, 0..3, "1"), Edit::replace(c, 0..5, "3")],
    )
    .unwrap();
    assert_eq!(text(&set, a), "1");
    assert_eq!(text(&set, b), "two");
    assert_eq!(text(&set, c), "3");
}
