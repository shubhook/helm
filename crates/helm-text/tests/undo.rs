//! T-102 acceptance tests: undo and redo chains, multi-buffer undo in one
//! step, redo dropped by a new transaction, and a later edit blocking an
//! undo or redo.

use helm_text::{Buffer, BufferId, BufferSet, Edit, HistoryError, Transaction, TransactionId};

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

/// Applies `edits` as one transaction and returns its id.
fn apply(set: &mut BufferSet, edits: Vec<Edit>) -> TransactionId {
    set.apply(Transaction::from(edits))
        .expect("transaction is valid")
}

/// Appends `suffix` to every buffer in `ids`, as one transaction.
fn append(set: &mut BufferSet, ids: &[BufferId], suffix: &str) -> TransactionId {
    let edits = ids
        .iter()
        .map(|&id| Edit::insert(id, set.get(id).unwrap().to_string().len(), suffix))
        .collect();
    apply(set, edits)
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn undo_and_redo_one_transaction() {
    let (mut set, ids) = set_of(&["hello"]);
    let a = ids[0];
    let t = apply(&mut set, vec![Edit::replace(a, 0..5, "world")]);

    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "hello");
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "world");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn nothing_to_undo_or_redo() {
    let (mut set, ids) = set_of(&["x"]);
    let a = ids[0];
    assert_eq!(set.undo(a), Ok(None));
    assert_eq!(set.redo(a), Ok(None));

    apply(&mut set, vec![Edit::insert(a, 1, "y")]);
    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(text(&set, a), "xy");

    set.undo(a).unwrap();
    assert_eq!(set.undo(a), Ok(None));
    assert_eq!(text(&set, a), "x");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn undo_and_redo_chains() {
    let (mut set, ids) = set_of(&[""]);
    let a = ids[0];
    let t1 = apply(&mut set, vec![Edit::insert(a, 0, "a")]);
    let t2 = apply(&mut set, vec![Edit::insert(a, 1, "b")]);
    let t3 = apply(&mut set, vec![Edit::insert(a, 2, "c")]);
    assert_eq!(text(&set, a), "abc");

    assert_eq!(set.undo(a), Ok(Some(t3)));
    assert_eq!(text(&set, a), "ab");
    assert_eq!(set.undo(a), Ok(Some(t2)));
    assert_eq!(text(&set, a), "a");
    assert_eq!(set.redo(a), Ok(Some(t2)));
    assert_eq!(text(&set, a), "ab");
    assert_eq!(set.undo(a), Ok(Some(t2)));
    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(text(&set, a), "");
    assert_eq!(set.undo(a), Ok(None));

    assert_eq!(set.redo(a), Ok(Some(t1)));
    assert_eq!(set.redo(a), Ok(Some(t2)));
    assert_eq!(set.redo(a), Ok(Some(t3)));
    assert_eq!(text(&set, a), "abc");
    assert_eq!(set.redo(a), Ok(None));
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn undo_restores_the_text_exactly() {
    // é(0..2) \r(2) \n(3) 中(4..7) 😀(7..11).
    let start = "é\r\n中😀";
    let (mut set, ids) = set_of(&[start]);
    let a = ids[0];
    apply(
        &mut set,
        vec![
            Edit::insert(a, 3, "\r"),
            Edit::delete(a, 4..7),
            Edit::replace(a, 7..11, "\n"),
        ],
    );
    let after = text(&set, a);
    assert_eq!(after, "é\r\r\n\n");

    set.undo(a).unwrap();
    assert_eq!(text(&set, a), start);
    set.redo(a).unwrap();
    assert_eq!(text(&set, a), after);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn one_undo_reverts_every_buffer_a_transaction_touched() {
    let starts = ["a", "b", "c", "d", "e"];
    let (mut set, ids) = set_of(&starts);
    // Buffers 0 to 3 change together, like a rename or an agent's change
    // across files. Buffer 4 isn't touched.
    let t = append(&mut set, &ids[..4], "!");
    assert_eq!(texts(&set, &ids), ["a!", "b!", "c!", "d!", "e"]);

    // Undo from any touched buffer reverts all of them.
    assert_eq!(set.undo(ids[2]), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), starts);
    // The transaction left every undo history at once.
    for &id in &ids {
        assert_eq!(set.undo(id), Ok(None));
    }

    // Redo from any touched buffer re-applies all of them.
    assert_eq!(set.redo(ids[0]), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["a!", "b!", "c!", "d!", "e"]);
    for &id in &ids {
        assert_eq!(set.redo(id), Ok(None));
    }

    assert_eq!(set.undo(ids[3]), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), starts);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn histories_are_per_buffer() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let ta = append(&mut set, &[a], "1");
    let tb = append(&mut set, &[b], "1");

    // Undo in `a` reverts `a`'s last transaction, even though `b` changed
    // later.
    assert_eq!(set.undo(a), Ok(Some(ta)));
    assert_eq!(texts(&set, &ids), ["a", "b1"]);
    assert_eq!(set.undo(b), Ok(Some(tb)));
    assert_eq!(texts(&set, &ids), ["a", "b"]);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_transaction_that_changes_nothing_is_recorded() {
    let (mut set, ids) = set_of(&["abc"]);
    let a = ids[0];
    let t1 = apply(&mut set, vec![Edit::insert(a, 1, "")]);
    let t2 = apply(&mut set, vec![Edit::insert(a, 3, "d")]);
    assert_eq!(set.undo(a), Ok(Some(t2)));
    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(text(&set, a), "abc");
    assert_eq!(set.undo(a), Ok(None));
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_new_transaction_drops_redo() {
    let (mut set, ids) = set_of(&[""]);
    let a = ids[0];
    let t1 = append(&mut set, &[a], "1");
    append(&mut set, &[a], "2");
    set.undo(a).unwrap();
    let t3 = append(&mut set, &[a], "3");

    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(text(&set, a), "13");
    assert_eq!(set.undo(a), Ok(Some(t3)));
    assert_eq!(set.undo(a), Ok(Some(t1)));
    assert_eq!(set.undo(a), Ok(None));
    assert_eq!(text(&set, a), "");
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_new_transaction_keeps_redo_of_other_buffers() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    append(&mut set, &[a], "1");
    let tb = append(&mut set, &[b], "1");
    set.undo(b).unwrap();

    append(&mut set, &[a], "2");
    assert_eq!(set.redo(b), Ok(Some(tb)));
    assert_eq!(texts(&set, &ids), ["a12", "b1"]);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_dropped_transaction_leaves_every_redo_history() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    append(&mut set, &[a, b], "1");
    set.undo(a).unwrap();
    assert_eq!(texts(&set, &ids), ["a", "b"]);

    // A new transaction in `a` drops the shared transaction, so `b` can't
    // redo it either: that would redo it in part.
    append(&mut set, &[a], "2");
    assert_eq!(set.redo(b), Ok(None));
    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(texts(&set, &ids), ["a2", "b"]);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn dropping_redo_also_drops_what_depends_on_it() {
    let (mut set, ids) = set_of(&["a", "b", "c"]);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    append(&mut set, &[a, b], "1");
    append(&mut set, &[b], "2");
    let tc = append(&mut set, &[c], "1");

    // Undo `b`'s two transactions and `c`'s one. `b`'s redo history now
    // holds the shared transaction on top of the one applied after it.
    set.undo(b).unwrap();
    set.undo(b).unwrap();
    set.undo(c).unwrap();
    assert_eq!(texts(&set, &ids), ["a", "b", "c"]);

    // A new transaction in `a` drops the shared transaction, and with it
    // `b`'s later transaction, which was made on top of it. `c` is
    // unrelated and keeps its redo.
    append(&mut set, &[a], "3");
    assert_eq!(set.redo(b), Ok(None));
    assert_eq!(texts(&set, &ids), ["a3", "b", "c"]);
    assert_eq!(set.redo(c), Ok(Some(tc)));
    assert_eq!(texts(&set, &ids), ["a3", "b", "c1"]);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_later_transaction_blocks_undo() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let t = append(&mut set, &[a, b], "1");
    let u = append(&mut set, &[b], "2");

    // `t` is on top of `a`'s undo history, but `u` is on top of `b`'s.
    // Undoing `t` now would revert it in `a` only, or undo `u` behind the
    // user's back, so it is refused and nothing changes.
    let blocked = Err(HistoryError::Blocked {
        transaction: t,
        buffer: b,
    });
    assert_eq!(set.undo(a), blocked);
    assert_eq!(texts(&set, &ids), ["a1", "b12"]);
    // Nothing moved to a redo history.
    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(set.redo(b), Ok(None));
    // Asking again gives the same answer.
    assert_eq!(set.undo(a), blocked);

    // Undoing `b` first unblocks it.
    assert_eq!(set.undo(b), Ok(Some(u)));
    assert_eq!(texts(&set, &ids), ["a1", "b1"]);
    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["a", "b"]);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_blocked_undo_names_the_lowest_blocking_buffer() {
    let (mut set, ids) = set_of(&["a", "b", "c"]);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let t = append(&mut set, &[a, b, c], "1");
    let u = append(&mut set, &[c], "2");
    let v = append(&mut set, &[b], "2");

    assert_eq!(
        set.undo(a),
        Err(HistoryError::Blocked {
            transaction: t,
            buffer: b,
        })
    );
    assert_eq!(set.undo(b), Ok(Some(v)));
    assert_eq!(
        set.undo(a),
        Err(HistoryError::Blocked {
            transaction: t,
            buffer: c,
        })
    );
    // `b` is blocked by `c` too, now that `t` is on top of `b`.
    assert_eq!(
        set.undo(b),
        Err(HistoryError::Blocked {
            transaction: t,
            buffer: c,
        })
    );
    assert_eq!(set.undo(c), Ok(Some(u)));
    assert_eq!(set.undo(b), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["a", "b", "c"]);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_later_transaction_blocks_redo() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let v = append(&mut set, &[b], "1");
    let t = append(&mut set, &[a, b], "2");

    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(set.undo(b), Ok(Some(v)));
    assert_eq!(texts(&set, &ids), ["a", "b"]);

    // `t` was made on top of `v`, so `t` can't be redone while `v` is still
    // undone.
    assert_eq!(
        set.redo(a),
        Err(HistoryError::Blocked {
            transaction: t,
            buffer: b,
        })
    );
    assert_eq!(texts(&set, &ids), ["a", "b"]);

    assert_eq!(set.redo(b), Ok(Some(v)));
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(texts(&set, &ids), ["a2", "b12"]);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn unknown_buffer() {
    let (mut set, _) = set_of(&["a"]);
    let (_, other) = set_of(&["x", "y"]);
    let unknown = other[1];
    let err = Err(HistoryError::UnknownBuffer { buffer: unknown });
    assert_eq!(set.undo(unknown), err);
    assert_eq!(set.redo(unknown), err);
}

#[test]
#[ignore = "T-102: not implemented yet"]
fn a_failed_transaction_keeps_redo() {
    let (mut set, ids) = set_of(&["abc"]);
    let a = ids[0];
    let t = append(&mut set, &[a], "d");
    set.undo(a).unwrap();

    assert!(set.apply(vec![Edit::delete(a, 0..99)].into()).is_err());
    assert!(set.apply(Transaction::new()).is_err());
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "abcd");
}
