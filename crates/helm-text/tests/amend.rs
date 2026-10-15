//! T-110 acceptance tests: `BufferSet::amend` folds new edits into an
//! applied transaction, so a run of keystrokes undoes in one step. Covers
//! offsets, multi-buffer amends, when an amend is allowed, the errors and
//! their order, redo dropping, versions and snapshots.

use std::ops::Range;

use helm_text::{
    AmendError, Buffer, BufferId, BufferSet, Edit, HistoryError, TextBuffer, TextError,
    Transaction, TransactionError, TransactionId,
};

/// The range `start..end`, which may start after it ends. Built from
/// values so clippy's `reversed_empty_ranges` doesn't reject the literal.
fn reversed(start: usize, end: usize) -> Range<usize> {
    start..end
}

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

/// The text of every buffer in `ids`, in order.
fn texts(set: &BufferSet, ids: &[BufferId]) -> Vec<String> {
    ids.iter().map(|&id| text(set, id)).collect()
}

/// The version number of every buffer in `ids`, in order.
fn versions(set: &BufferSet, ids: &[BufferId]) -> Vec<u64> {
    ids.iter()
        .map(|&id| set.version(id).expect("buffer is in the set").get())
        .collect()
}

/// Applies `edits` as one transaction and returns its id.
fn apply(set: &mut BufferSet, edits: Vec<Edit>) -> TransactionId {
    set.apply(Transaction::from(edits))
        .expect("transaction is valid")
}

/// Amends `id` with `edits`, which must be accepted.
fn amend(set: &mut BufferSet, id: TransactionId, edits: Vec<Edit>) {
    set.amend(id, Transaction::from(edits))
        .expect("amend is valid");
}

/// Everything a failed call must leave as it was: every buffer's text and
/// version.
fn state(set: &BufferSet, ids: &[BufferId]) -> (Vec<String>, Vec<u64>) {
    (texts(set, ids), versions(set, ids))
}

// Folding edits into one undo step.

#[test]
fn typing_a_word_undoes_in_one_step() {
    let (mut set, ids) = set_of(&["say !"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 4, "h")]);
    for (i, c) in "ello".chars().enumerate() {
        amend(&mut set, t, vec![Edit::insert(a, 5 + i, c.to_string())]);
    }
    assert_eq!(text(&set, a), "say hello!");

    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "say !");
    assert_eq!(set.undo(a), Ok(None));
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "say hello!");
    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "say !");
}

#[test]
fn amend_returns_unit() {
    let (mut set, ids) = set_of(&["x"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 1, "y")]);
    assert_eq!(
        set.amend(t, Transaction::from(vec![Edit::insert(a, 2, "z")])),
        Ok(())
    );
    assert_eq!(text(&set, a), "xyz");
}

#[test]
fn the_id_stays_the_same_and_no_id_is_used_up() {
    let (mut set, ids) = set_of(&["", ""]);
    let (a, b) = (ids[0], ids[1]);
    let t1 = apply(&mut set, vec![Edit::insert(a, 0, "a")]);
    amend(&mut set, t1, vec![Edit::insert(a, 1, "b")]);
    amend(&mut set, t1, vec![Edit::insert(a, 2, "c")]);
    let t2 = apply(&mut set, vec![Edit::insert(b, 0, "x")]);
    assert!(t2 > t1, "ids still increase");

    // An editor keys its undo record by the id `apply` returned, so undo
    // and redo must keep returning it.
    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(set.redo(a), Ok(Some(t1)));
    assert_eq!(set.undo(b), Ok(Some(t2)));
    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(texts(&set, &ids), ["", ""]);
}

#[test]
fn offsets_refer_to_the_text_after_the_transaction() {
    let (mut set, ids) = set_of(&["abcdef"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::delete(a, 0..2)]);
    assert_eq!(text(&set, a), "cdef");

    // 0..1 is "c" in the text after `t`, not "a" in the text before it.
    amend(&mut set, t, vec![Edit::replace(a, 0..1, "X")]);
    assert_eq!(text(&set, a), "Xdef");

    // Within one amend, ranges refer to the text before the amend, in any
    // listing order, as for `apply`.
    amend(
        &mut set,
        t,
        vec![
            Edit::delete(a, 3..4),
            Edit::insert(a, 0, "<"),
            Edit::delete(a, 1..2),
        ],
    );
    assert_eq!(text(&set, a), "<Xe");

    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "abcdef");
    set.redo(a).unwrap();
    assert_eq!(text(&set, a), "<Xe");
}

#[test]
fn an_offset_valid_only_before_the_transaction_is_rejected() {
    let (mut set, ids) = set_of(&["abcdef"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::delete(a, 2..6)]);
    assert_eq!(text(&set, a), "ab");

    let before = state(&set, &ids);
    assert_eq!(
        set.amend(t, Transaction::from(vec![Edit::insert(a, 5, "!")])),
        Err(AmendError::Transaction(TransactionError::InvalidRange {
            edit: 0,
            error: TextError::ByteOutOfBounds { offset: 5, len: 2 },
        }))
    );
    assert_eq!(state(&set, &ids), before);
}

#[test]
fn amend_can_change_text_the_transaction_put_there() {
    // Delete part of what the transaction inserted.
    let (mut set, ids) = set_of(&["ac"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 1, "XYZ")]);
    amend(&mut set, t, vec![Edit::delete(a, 2..3)]);
    assert_eq!(text(&set, a), "aXZc");
    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "ac");
    set.redo(a).unwrap();
    assert_eq!(text(&set, a), "aXZc");

    // Delete across a replacement and the text around it.
    let (mut set, ids) = set_of(&["xabcy"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::replace(a, 1..4, "Q")]);
    assert_eq!(text(&set, a), "xQy");
    amend(&mut set, t, vec![Edit::delete(a, 0..2)]);
    assert_eq!(text(&set, a), "y");
    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "xabcy");
    set.redo(a).unwrap();
    assert_eq!(text(&set, a), "y");

    // Backspace through everything typed, and past it.
    let (mut set, ids) = set_of(&["ab"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 2, "c")]);
    amend(&mut set, t, vec![Edit::delete(a, 2..3)]);
    amend(&mut set, t, vec![Edit::delete(a, 1..2)]);
    amend(&mut set, t, vec![Edit::delete(a, 0..1)]);
    assert_eq!(text(&set, a), "");
    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "ab");
    set.redo(a).unwrap();
    assert_eq!(text(&set, a), "");
}

#[test]
fn amend_with_edits_far_from_the_transaction() {
    let (mut set, ids) = set_of(&["one two three"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::replace(a, 4..7, "2")]);
    assert_eq!(text(&set, a), "one 2 three");
    amend(
        &mut set,
        t,
        vec![Edit::replace(a, 0..3, "1"), Edit::replace(a, 6..11, "3")],
    );
    assert_eq!(text(&set, a), "1 2 3");
    amend(
        &mut set,
        t,
        vec![Edit::insert(a, 5, "!"), Edit::insert(a, 0, "¡")],
    );
    assert_eq!(text(&set, a), "¡1 2 3!");
    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "one two three");
    set.redo(a).unwrap();
    assert_eq!(text(&set, a), "¡1 2 3!");
}

#[test]
fn inserts_at_one_offset_keep_their_order() {
    let (mut set, ids) = set_of(&["ab"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 1, "-")]);
    amend(
        &mut set,
        t,
        vec![
            Edit::replace(a, 1..2, "="),
            Edit::insert(a, 1, "1"),
            Edit::insert(a, 1, "2"),
            Edit::insert(a, 2, "3"),
        ],
    );
    assert_eq!(text(&set, a), "a12=3b");
    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "ab");
}

#[test]
fn wide_characters_and_crlf() {
    let (mut set, ids) = set_of(&["é中\r\n😀"]);
    let a = ids[0];
    // é is 0..2, 中 is 2..5, \r\n is 5..7, 😀 is 7..11.
    let t = apply(&mut set, vec![Edit::insert(a, 6, "|")]);
    assert_eq!(text(&set, a), "é中\r|\n😀");
    amend(
        &mut set,
        t,
        vec![Edit::replace(a, 2..5, "😀"), Edit::delete(a, 8..12)],
    );
    assert_eq!(text(&set, a), "é😀\r|\n");

    let before = state(&set, &ids);
    assert_eq!(
        set.amend(t, Transaction::from(vec![Edit::insert(a, 3, "x")])),
        Err(AmendError::Transaction(TransactionError::InvalidRange {
            edit: 0,
            error: TextError::NotCharBoundary { offset: 3 },
        }))
    );
    assert_eq!(state(&set, &ids), before);

    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "é中\r\n😀");
    set.redo(a).unwrap();
    assert_eq!(text(&set, a), "é😀\r|\n");
}

#[test]
fn a_no_op_amend_is_accepted() {
    let (mut set, ids) = set_of(&["abc"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 3, "d")]);
    let version = set.version(a).unwrap();
    amend(&mut set, t, vec![Edit::insert(a, 1, "")]);
    amend(&mut set, t, vec![Edit::replace(a, 0..1, "a")]);
    assert_eq!(text(&set, a), "abcd");
    assert_eq!(set.version(a).unwrap().get(), version.get() + 2);

    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "abc");
    assert_eq!(set.undo(a), Ok(None));
}

#[test]
fn amend_after_redo() {
    let (mut set, ids) = set_of(&["a"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 1, "b")]);
    set.undo(a).unwrap();
    set.redo(a).unwrap();

    // Redone, `t` is on top of the undo history again.
    amend(&mut set, t, vec![Edit::insert(a, 2, "c")]);
    assert_eq!(text(&set, a), "abc");
    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "a");
}

#[test]
fn amend_an_earlier_transaction_once_the_later_one_is_undone() {
    let (mut set, ids) = set_of(&["a"]);
    let a = ids[0];
    let t1 = apply(&mut set, vec![Edit::insert(a, 1, "b")]);
    apply(&mut set, vec![Edit::insert(a, 2, "c")]);
    set.undo(a).unwrap();
    assert_eq!(text(&set, a), "ab");

    amend(&mut set, t1, vec![Edit::insert(a, 2, "B")]);
    assert_eq!(text(&set, a), "abB");
    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(text(&set, a), "a");
}

// Several buffers.

#[test]
fn one_undo_reverts_an_amended_transaction_in_every_buffer() {
    let (mut set, ids) = set_of(&["fn old() {}", "old();", "other"]);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let t = apply(
        &mut set,
        vec![Edit::replace(a, 3..6, "n"), Edit::replace(b, 0..3, "n")],
    );
    amend(
        &mut set,
        t,
        vec![Edit::insert(a, 4, "ew"), Edit::insert(b, 1, "ew")],
    );
    assert_eq!(texts(&set, &ids), ["fn new() {}", "new();", "other"]);

    assert_eq!(set.undo(b), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["fn old() {}", "old();", "other"]);
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["fn new() {}", "new();", "other"]);
    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["fn old() {}", "old();", "other"]);
    assert_eq!(set.undo(c), Ok(None));
}

#[test]
fn amend_may_touch_some_of_the_transactions_buffers() {
    let (mut set, ids) = set_of(&["a", "b", "c"]);
    let t = apply(
        &mut set,
        vec![
            Edit::insert(ids[0], 1, "1"),
            Edit::insert(ids[1], 1, "1"),
            Edit::insert(ids[2], 1, "1"),
        ],
    );
    amend(&mut set, t, vec![Edit::insert(ids[0], 2, "2")]);
    amend(&mut set, t, vec![Edit::insert(ids[2], 2, "2")]);
    assert_eq!(texts(&set, &ids), ["a12", "b1", "c12"]);

    assert_eq!(set.undo(ids[1]), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["a", "b", "c"]);
    assert_eq!(set.redo(ids[2]), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["a12", "b1", "c12"]);
}

#[test]
fn a_later_transaction_in_another_buffer_blocks_only_that_buffer() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let t1 = apply(
        &mut set,
        vec![Edit::insert(a, 1, "1"), Edit::insert(b, 1, "1")],
    );
    let t2 = apply(&mut set, vec![Edit::insert(b, 2, "2")]);

    // `t1` is still on top of `a`, so amending it there works.
    amend(&mut set, t1, vec![Edit::insert(a, 2, "+")]);
    assert_eq!(texts(&set, &ids), ["a1+", "b12"]);

    // Undoing `t1` stays blocked by `t2`, as before the amend.
    assert_eq!(
        set.undo(a),
        Err(HistoryError::Blocked {
            transaction: t1,
            buffer: b,
        })
    );
    assert_eq!(set.undo(b), Ok(Some(t2)));
    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(texts(&set, &ids), ["a", "b"]);
}

// When an amend is refused.

#[test]
fn a_later_transaction_on_top_refuses_the_amend() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let t1 = apply(
        &mut set,
        vec![Edit::insert(a, 1, "1"), Edit::insert(b, 1, "1")],
    );
    apply(&mut set, vec![Edit::insert(b, 2, "2")]);
    let before = state(&set, &ids);

    assert_eq!(
        set.amend(t1, Transaction::from(vec![Edit::insert(b, 0, "x")])),
        Err(AmendError::NotOnTop { edit: 0, buffer: b })
    );
    // The edit on `a` is fine, but nothing applies when edit 1 fails.
    assert_eq!(
        set.amend(
            t1,
            Transaction::from(vec![Edit::insert(a, 0, "x"), Edit::insert(b, 0, "x")])
        ),
        Err(AmendError::NotOnTop { edit: 1, buffer: b })
    );
    assert_eq!(state(&set, &ids), before);
}

#[test]
fn an_undone_transaction_is_refused() {
    let (mut set, ids) = set_of(&["a"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 1, "b")]);
    set.undo(a).unwrap();
    let before = state(&set, &ids);

    assert_eq!(
        set.amend(t, Transaction::from(vec![Edit::insert(a, 1, "c")])),
        Err(AmendError::Undone { transaction: t })
    );
    assert_eq!(state(&set, &ids), before);
    // Its redo history is untouched.
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "ab");
}

#[test]
fn a_dropped_transaction_is_unknown() {
    let (mut set, ids) = set_of(&["a"]);
    let a = ids[0];
    let t1 = apply(&mut set, vec![Edit::insert(a, 1, "b")]);
    set.undo(a).unwrap();
    // A new transaction drops `t1` from the redo history.
    apply(&mut set, vec![Edit::insert(a, 1, "c")]);
    let before = state(&set, &ids);

    assert_eq!(
        set.amend(t1, Transaction::from(vec![Edit::insert(a, 0, "x")])),
        Err(AmendError::UnknownTransaction { transaction: t1 })
    );
    assert_eq!(state(&set, &ids), before);
}

#[test]
fn a_transaction_from_another_set_is_unknown() {
    let (mut other, other_ids) = set_of(&["x"]);
    let t = apply(&mut other, vec![Edit::insert(other_ids[0], 1, "y")]);

    let (mut set, ids) = set_of(&["a"]);
    let before = state(&set, &ids);
    assert_eq!(
        set.amend(t, Transaction::from(vec![Edit::insert(ids[0], 0, "z")])),
        Err(AmendError::UnknownTransaction { transaction: t })
    );
    assert_eq!(state(&set, &ids), before);
}

#[test]
fn an_untouched_buffer_is_refused() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let t = apply(&mut set, vec![Edit::insert(a, 1, "1")]);
    let before = state(&set, &ids);

    assert_eq!(
        set.amend(
            t,
            Transaction::from(vec![Edit::insert(a, 2, "2"), Edit::insert(b, 0, "x")])
        ),
        Err(AmendError::Untouched { edit: 1, buffer: b })
    );
    assert_eq!(state(&set, &ids), before);
}

#[test]
fn apply_errors_come_wrapped() {
    let (mut set, ids) = set_of(&["héllo"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 6, "!")]);
    // "héllo!": é is 1..3, and the text is 7 bytes.
    let (_, bigger) = set_of(&["", ""]);
    let stranger = bigger[1];
    let before = state(&set, &ids);

    let cases: Vec<(Vec<Edit>, TransactionError)> = vec![
        (Vec::new(), TransactionError::Empty),
        (
            vec![Edit::insert(stranger, 0, "x")],
            TransactionError::UnknownBuffer {
                edit: 0,
                buffer: stranger,
            },
        ),
        (
            vec![Edit::delete(a, reversed(4, 3))],
            TransactionError::InvalidRange {
                edit: 0,
                error: TextError::InvalidRange { start: 4, end: 3 },
            },
        ),
        (
            vec![Edit::insert(a, 0, "x"), Edit::delete(a, 6..8)],
            TransactionError::InvalidRange {
                edit: 1,
                error: TextError::ByteOutOfBounds { offset: 8, len: 7 },
            },
        ),
        (
            vec![Edit::insert(a, usize::MAX, "x")],
            TransactionError::InvalidRange {
                edit: 0,
                error: TextError::ByteOutOfBounds {
                    offset: usize::MAX,
                    len: 7,
                },
            },
        ),
        (
            vec![Edit::delete(a, 2..4)],
            TransactionError::InvalidRange {
                edit: 0,
                error: TextError::NotCharBoundary { offset: 2 },
            },
        ),
        (
            vec![Edit::delete(a, 0..2)],
            TransactionError::InvalidRange {
                edit: 0,
                error: TextError::NotCharBoundary { offset: 2 },
            },
        ),
        (
            vec![
                Edit::delete(a, 0..1),
                Edit::delete(a, 3..6),
                Edit::delete(a, 5..7),
                Edit::insert(a, 4, "x"),
            ],
            TransactionError::Overlap { edit: 2, other: 1 },
        ),
        (
            vec![
                Edit::delete(a, 3..6),
                Edit::delete(a, 0..1),
                Edit::delete(a, 0..7),
            ],
            TransactionError::Overlap { edit: 2, other: 0 },
        ),
    ];
    for (edits, error) in cases {
        let got = set.amend(t, Transaction::from(edits.clone()));
        assert_eq!(got, Err(AmendError::Transaction(error)), "{edits:?}");
        assert_eq!(state(&set, &ids), before, "{edits:?}");
    }
}

#[test]
fn invalid_ranges_match_slice_on_the_text_after_the_transaction() {
    let (mut set, ids) = set_of(&["a😀b"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::insert(a, 0, "中")]);
    let after = set.get(a).unwrap().clone();
    for start in 0..=after.len_bytes() + 1 {
        for end in 0..=after.len_bytes() + 1 {
            let Err(error) = after.slice(start..end) else {
                continue;
            };
            assert_eq!(
                set.amend(t, Transaction::from(vec![Edit::delete(a, start..end)])),
                Err(AmendError::Transaction(TransactionError::InvalidRange {
                    edit: 0,
                    error,
                })),
                "{start}..{end}"
            );
        }
    }
    assert_eq!(text(&set, a), "中a😀b");
}

#[test]
fn the_checks_run_in_order() {
    let (mut set, ids) = set_of(&["a", "b", "c"]);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let (_, more) = set_of(&["", "", "", ""]);
    let stranger = more[3];
    let t1 = apply(
        &mut set,
        vec![Edit::insert(a, 1, "1"), Edit::insert(b, 1, "1")],
    );
    let t2 = apply(
        &mut set,
        vec![Edit::insert(b, 2, "2"), Edit::insert(c, 1, "2")],
    );
    set.undo(b).unwrap();
    // Now `t2` is undone; `t1` is on top of `a` and `b`. Apply `t3` on `b`
    // so `t1` is no longer on top there, and drop `t2`.
    let t3 = apply(&mut set, vec![Edit::insert(b, 2, "3")]);
    let t4 = apply(&mut set, vec![Edit::insert(c, 0, "4")]);
    set.undo(c).unwrap();
    // `t4` is undone, `t2` is unknown.
    let before = state(&set, &ids);
    let check = |set: &mut BufferSet, id: TransactionId, edits: Vec<Edit>| {
        set.amend(id, Transaction::from(edits))
    };

    // The id is checked before the edits.
    assert_eq!(
        check(&mut set, t2, Vec::new()),
        Err(AmendError::UnknownTransaction { transaction: t2 })
    );
    assert_eq!(
        check(&mut set, t4, Vec::new()),
        Err(AmendError::Undone { transaction: t4 })
    );
    assert_eq!(
        check(&mut set, t1, Vec::new()),
        Err(AmendError::Transaction(TransactionError::Empty))
    );

    // Per edit: unknown buffer, then untouched, then not on top, then the
    // range, then overlap.
    assert_eq!(
        check(&mut set, t1, vec![Edit::delete(stranger, reversed(5, 0))]),
        Err(AmendError::Transaction(TransactionError::UnknownBuffer {
            edit: 0,
            buffer: stranger,
        }))
    );
    // `c` has `t2` dropped and `t4` undone, but `t1` never touched it.
    assert_eq!(
        check(&mut set, t1, vec![Edit::delete(c, reversed(5, 0))]),
        Err(AmendError::Untouched { edit: 0, buffer: c })
    );
    assert_eq!(
        check(&mut set, t1, vec![Edit::delete(b, reversed(5, 0))]),
        Err(AmendError::NotOnTop { edit: 0, buffer: b })
    );
    assert_eq!(
        check(
            &mut set,
            t1,
            vec![Edit::delete(a, 0..2), Edit::delete(a, reversed(5, 0))]
        ),
        Err(AmendError::Transaction(TransactionError::InvalidRange {
            edit: 1,
            error: TextError::InvalidRange { start: 5, end: 0 },
        }))
    );

    // An earlier edit's problem comes first, whatever its kind.
    assert_eq!(
        check(
            &mut set,
            t1,
            vec![Edit::delete(a, reversed(5, 0)), Edit::insert(c, 0, "x")]
        ),
        Err(AmendError::Transaction(TransactionError::InvalidRange {
            edit: 0,
            error: TextError::InvalidRange { start: 5, end: 0 },
        }))
    );
    assert_eq!(
        check(
            &mut set,
            t1,
            vec![
                Edit::delete(a, 0..2),
                Edit::delete(a, 1..2),
                Edit::insert(b, 0, "x")
            ]
        ),
        Err(AmendError::Transaction(TransactionError::Overlap {
            edit: 1,
            other: 0,
        }))
    );
    assert_eq!(
        check(
            &mut set,
            t1,
            vec![
                Edit::insert(b, 0, "x"),
                Edit::delete(a, 0..2),
                Edit::delete(a, 1..2)
            ]
        ),
        Err(AmendError::NotOnTop { edit: 0, buffer: b })
    );

    assert_eq!(state(&set, &ids), before);
    // `t1` can still be amended on `a`, and the histories still work.
    assert_eq!(check(&mut set, t1, vec![Edit::insert(a, 2, "+")]), Ok(()));
    assert_eq!(set.redo(c), Ok(Some(t4)));
    assert_eq!(set.undo(b), Ok(Some(t3)));
}

// Redo.

#[test]
fn amend_drops_redo_like_apply() {
    let (mut set, ids) = set_of(&["a", "b", "c", "d"]);
    let (a, b, c, d) = (ids[0], ids[1], ids[2], ids[3]);
    let t1 = apply(
        &mut set,
        vec![Edit::insert(a, 1, "1"), Edit::insert(b, 1, "1")],
    );
    // `t2` is undone in `a`; `t3` across `b` and `c` is undone; `t4` on `d`
    // is undone.
    apply(&mut set, vec![Edit::insert(a, 2, "2")]);
    apply(
        &mut set,
        vec![Edit::insert(b, 2, "3"), Edit::insert(c, 1, "3")],
    );
    let t4 = apply(&mut set, vec![Edit::insert(d, 1, "4")]);
    set.undo(a).unwrap();
    set.undo(b).unwrap();
    set.undo(d).unwrap();
    assert_eq!(texts(&set, &ids), ["a1", "b1", "c", "d"]);

    // Amending only on `a` drops `a`'s redo and keeps the rest.
    amend(&mut set, t1, vec![Edit::insert(a, 2, "+")]);
    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(set.redo(d), Ok(Some(t4)));
    set.undo(d).unwrap();

    // Amending on `b` drops `t3`, which leaves `c`'s redo history too.
    amend(&mut set, t1, vec![Edit::insert(b, 2, "+")]);
    assert_eq!(set.redo(b), Ok(None));
    assert_eq!(set.redo(c), Ok(None));
    assert_eq!(texts(&set, &ids), ["a1+", "b1+", "c", "d"]);
    // An unrelated buffer keeps its redo.
    assert_eq!(set.redo(d), Ok(Some(t4)));

    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(texts(&set, &ids), ["a", "b", "c", "d4"]);
}

#[test]
fn amend_keeps_redo_in_buffers_it_does_not_touch() {
    // `t1` touched `a` and `b`, but the amend touches only `a`, so `b`'s
    // redo history stays.
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let t1 = apply(
        &mut set,
        vec![Edit::insert(a, 1, "1"), Edit::insert(b, 1, "1")],
    );
    let t2 = apply(&mut set, vec![Edit::insert(b, 2, "2")]);
    set.undo(b).unwrap();

    amend(&mut set, t1, vec![Edit::insert(a, 2, "+")]);
    assert_eq!(set.redo(b), Ok(Some(t2)));
    assert_eq!(texts(&set, &ids), ["a1+", "b12"]);
}

#[test]
fn a_no_op_amend_still_drops_redo() {
    let (mut set, ids) = set_of(&["a"]);
    let a = ids[0];
    let t1 = apply(&mut set, vec![Edit::insert(a, 1, "b")]);
    apply(&mut set, vec![Edit::insert(a, 2, "c")]);
    set.undo(a).unwrap();
    amend(&mut set, t1, vec![Edit::insert(a, 0, "")]);
    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(text(&set, a), "ab");
}

#[test]
fn a_failed_amend_keeps_redo() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let t1 = apply(&mut set, vec![Edit::insert(a, 1, "1")]);
    let t2 = apply(&mut set, vec![Edit::insert(a, 2, "2")]);
    set.undo(a).unwrap();

    for edits in [
        vec![Edit::insert(a, 9, "x")],
        vec![Edit::insert(b, 0, "x")],
        vec![
            Edit::insert(a, 0, "x"),
            Edit::delete(a, 0..1),
            Edit::delete(a, 0..2),
        ],
        Vec::new(),
    ] {
        assert!(set.amend(t1, Transaction::from(edits)).is_err());
    }
    assert_eq!(set.redo(a), Ok(Some(t2)));
    assert_eq!(text(&set, a), "a12");
}

// Versions and snapshots.

#[test]
fn amend_bumps_each_touched_buffer_once() {
    let (mut set, ids) = set_of(&["a", "b", "c", "d"]);
    let t = apply(
        &mut set,
        vec![
            Edit::insert(ids[0], 1, "1"),
            Edit::insert(ids[1], 1, "1"),
            Edit::insert(ids[2], 1, "1"),
        ],
    );
    assert_eq!(versions(&set, &ids), [1, 1, 1, 0]);

    // Three edits in `a` and one in `b` bump each once. `c`, which `t`
    // touched but the amend doesn't, and `d` keep their versions.
    amend(
        &mut set,
        t,
        vec![
            Edit::insert(ids[0], 0, "x"),
            Edit::insert(ids[1], 0, "x"),
            Edit::insert(ids[0], 1, "y"),
            Edit::insert(ids[0], 2, "z"),
        ],
    );
    assert_eq!(versions(&set, &ids), [2, 2, 1, 0]);
    amend(&mut set, t, vec![Edit::insert(ids[0], 0, "w")]);
    assert_eq!(versions(&set, &ids), [3, 2, 1, 0]);

    // Undo and redo bump every buffer `t` touched once.
    set.undo(ids[2]).unwrap();
    assert_eq!(versions(&set, &ids), [4, 3, 2, 0]);
    set.redo(ids[1]).unwrap();
    assert_eq!(versions(&set, &ids), [5, 4, 3, 0]);
}

#[test]
fn a_failed_amend_keeps_every_version() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let t1 = apply(
        &mut set,
        vec![Edit::insert(a, 1, "1"), Edit::insert(b, 1, "1")],
    );
    let t2 = apply(&mut set, vec![Edit::insert(b, 2, "2")]);
    set.undo(b).unwrap();
    apply(&mut set, vec![Edit::insert(b, 2, "3")]);
    let before = versions(&set, &ids);

    let failures: Vec<(TransactionId, Vec<Edit>)> = vec![
        (t2, vec![Edit::insert(a, 0, "x")]),
        (t1, Vec::new()),
        (t1, vec![Edit::insert(a, 0, "x"), Edit::insert(b, 0, "x")]),
        (t1, vec![Edit::insert(a, 0, "x"), Edit::insert(a, 7, "x")]),
        (t1, vec![Edit::delete(a, 0..2), Edit::delete(a, 1..2)]),
    ];
    for (id, edits) in failures {
        assert!(set.amend(id, Transaction::from(edits)).is_err());
        assert_eq!(versions(&set, &ids), before);
    }
    let t3 = set.undo(b).unwrap().unwrap();
    set.undo(a).unwrap();
    let before = versions(&set, &ids);
    assert_eq!(
        set.amend(t3, Transaction::from(vec![Edit::insert(b, 0, "x")])),
        Err(AmendError::Undone { transaction: t3 })
    );
    assert_eq!(versions(&set, &ids), before);
}

#[test]
fn snapshots_go_stale_on_amend() {
    let (mut set, ids) = set_of(&["hello", "world"]);
    let (a, b) = (ids[0], ids[1]);
    let t = apply(
        &mut set,
        vec![Edit::insert(a, 5, "!"), Edit::insert(b, 5, "!")],
    );
    let old_a = set.snapshot(a).unwrap();
    let old_b = set.snapshot(b).unwrap();

    amend(&mut set, t, vec![Edit::insert(a, 6, "!")]);
    assert!(old_a.is_stale());
    assert!(!old_b.is_stale(), "the amend didn't touch b");
    assert_eq!(old_a.to_string(), "hello!");
    assert_eq!(old_a.version().get(), 1);

    let new_a = set.snapshot(a).unwrap();
    assert_eq!(new_a.to_string(), "hello!!");
    assert_eq!(new_a.version().get(), 2);
    assert!(!new_a.is_stale());

    // A failed amend leaves snapshots fresh.
    assert!(
        set.amend(t, Transaction::from(vec![Edit::insert(a, 99, "x")]))
            .is_err()
    );
    assert!(!new_a.is_stale());

    // Undo makes both stale, and gives `a` its text from before `t`.
    set.undo(a).unwrap();
    assert!(new_a.is_stale());
    assert!(old_b.is_stale());
    assert_eq!(set.snapshot(a).unwrap().to_string(), "hello");
    assert_eq!(new_a.to_string(), "hello!!");
}
