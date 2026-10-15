//! T-103 acceptance tests: buffer versions and snapshots on hand-picked
//! cases. What changes a version and what doesn't, undo and redo getting
//! new versions, snapshots keeping their text and version, reading like the
//! buffer did, and knowing when they are stale.

use std::fmt::{Debug, Display};
use std::thread;

use helm_text::{
    Buffer, BufferId, BufferSet, Edit, HistoryError, Point, Snapshot, TextBuffer, Transaction,
    TransactionError, TransactionId, Utf16Point,
};

/// Compile-time check: a snapshot can move to another thread, be shared
/// between threads, outlive everything it came from, and be cloned.
const _: () = {
    const fn check<T: Send + Sync + Clone + Debug + Display + TextBuffer + 'static>() {}
    check::<Snapshot>();
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

/// The version of buffer `id` as a number.
fn version(set: &BufferSet, id: BufferId) -> u64 {
    set.version(id).expect("buffer is in the set").get()
}

/// The versions of every buffer in `ids`, in order.
fn versions(set: &BufferSet, ids: &[BufferId]) -> Vec<u64> {
    ids.iter().map(|&id| version(set, id)).collect()
}

/// A snapshot of buffer `id`.
fn snapshot(set: &BufferSet, id: BufferId) -> Snapshot {
    set.snapshot(id).expect("buffer is in the set")
}

/// Applies `edits` as one transaction and returns its id.
fn apply(set: &mut BufferSet, edits: Vec<Edit>) -> TransactionId {
    set.apply(Transaction::from(edits))
        .expect("transaction is valid")
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn new_buffers_start_at_version_zero() {
    let (mut set, ids) = set_of(&["", "hello", "中😀\r\n"]);
    assert_eq!(versions(&set, &ids), [0, 0, 0]);

    // A buffer added after others have changed starts at 0 too.
    apply(&mut set, vec![Edit::insert(ids[0], 0, "x")]);
    let late = set.add(Buffer::from("late"));
    assert_eq!(version(&set, late), 0);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn unknown_buffer_has_no_version_or_snapshot() {
    let (big, big_ids) = set_of(&["a", "b", "c"]);
    let (small, _) = set_of(&["a"]);
    // An id from another set that this set has no buffer for.
    assert_eq!(small.version(big_ids[2]), None);
    assert!(small.snapshot(big_ids[2]).is_none());

    let empty = BufferSet::new();
    assert_eq!(empty.version(big_ids[0]), None);
    assert!(empty.snapshot(big_ids[0]).is_none());
    assert_eq!(version(&big, big_ids[2]), 0);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn apply_adds_one_to_each_touched_buffer() {
    let (mut set, ids) = set_of(&["abcdef", "xyz", "untouched"]);
    let (a, b) = (ids[0], ids[1]);

    // Three edits on `a` and one on `b`: one version each, not one per edit.
    apply(
        &mut set,
        vec![
            Edit::insert(a, 0, "1"),
            Edit::delete(a, 2..3),
            Edit::replace(a, 4..6, "Z"),
            Edit::insert(b, 3, "!"),
        ],
    );
    assert_eq!(versions(&set, &ids), [1, 1, 0]);

    apply(&mut set, vec![Edit::insert(a, 0, "2")]);
    assert_eq!(versions(&set, &ids), [2, 1, 0]);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn undo_and_redo_get_new_versions() {
    let (mut set, ids) = set_of(&["hello"]);
    let a = ids[0];
    let before = snapshot(&set, a);

    let t = apply(&mut set, vec![Edit::replace(a, 0..5, "world")]);
    assert_eq!(version(&set, a), 1);

    assert_eq!(set.undo(a), Ok(Some(t)));
    // The text is back to what it was at version 0, but the version is new.
    assert_eq!(text(&set, a), "hello");
    assert_eq!(version(&set, a), 2);
    assert_ne!(set.version(a), Some(before.version()));
    assert!(before.is_stale());

    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), "world");
    assert_eq!(version(&set, a), 3);

    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(version(&set, a), 5);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn versions_only_grow() {
    let (mut set, ids) = set_of(&["a"]);
    let a = ids[0];
    let mut seen = vec![set.version(a).unwrap()];
    apply(&mut set, vec![Edit::insert(a, 1, "b")]);
    seen.push(set.version(a).unwrap());
    set.undo(a).unwrap();
    seen.push(set.version(a).unwrap());
    apply(&mut set, vec![Edit::insert(a, 1, "c")]);
    seen.push(set.version(a).unwrap());
    set.undo(a).unwrap();
    seen.push(set.version(a).unwrap());
    set.redo(a).unwrap();
    seen.push(set.version(a).unwrap());

    assert!(seen.windows(2).all(|pair| pair[0] < pair[1]), "{seen:?}");
    assert_eq!(
        seen.iter().map(|v| v.get()).collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 5]
    );
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn new_transaction_after_undo_gets_a_fresh_version() {
    let (mut set, ids) = set_of(&["base"]);
    let a = ids[0];

    apply(&mut set, vec![Edit::insert(a, 4, " one")]);
    let one = snapshot(&set, a);
    set.undo(a).unwrap();
    apply(&mut set, vec![Edit::insert(a, 4, " two")]);
    let two = snapshot(&set, a);

    // Version 1 and the version after the undo and new edit name different
    // texts, so they must differ.
    assert_eq!(one.version().get(), 1);
    assert_eq!(two.version().get(), 3);
    assert_eq!(one.to_string(), "base one");
    assert_eq!(two.to_string(), "base two");
    assert!(one.is_stale());
    assert!(!two.is_stale());
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn multi_buffer_undo_and_redo_change_every_touched_buffer() {
    let (mut set, ids) = set_of(&["a", "b", "c"]);
    let (a, b, c) = (ids[0], ids[1], ids[2]);
    let t = apply(
        &mut set,
        vec![Edit::insert(a, 1, "1"), Edit::insert(c, 1, "1")],
    );
    assert_eq!(versions(&set, &ids), [1, 0, 1]);

    // Undoing from `c` reverts `a` too, so both get a new version.
    assert_eq!(set.undo(c), Ok(Some(t)));
    assert_eq!(versions(&set, &ids), [2, 0, 2]);
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(versions(&set, &ids), [3, 0, 3]);
    assert_eq!(version(&set, b), 0);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn no_op_transaction_still_counts() {
    let (mut set, ids) = set_of(&["same"]);
    let a = ids[0];
    let before = snapshot(&set, a);
    let t = apply(&mut set, vec![Edit::insert(a, 2, "")]);
    assert_eq!(text(&set, a), "same");
    assert_eq!(version(&set, a), 1);
    assert!(before.is_stale());

    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(version(&set, a), 2);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn failed_calls_leave_versions_alone() {
    let (mut set, ids) = set_of(&["héllo", "world", "third"]);
    let (a, b) = (ids[0], ids[1]);
    let (other, other_ids) = set_of(&["1", "2", "3", "4"]);
    let unknown = other_ids[3];
    drop(other);

    let snaps: Vec<Snapshot> = ids.iter().map(|&id| snapshot(&set, id)).collect();
    let check = |set: &BufferSet, expected: [u64; 3]| {
        assert_eq!(versions(set, &ids), expected);
    };

    // Nothing to undo or redo yet.
    assert_eq!(set.undo(a), Ok(None));
    assert_eq!(set.redo(a), Ok(None));
    check(&set, [0, 0, 0]);

    // Every way `apply` can fail.
    let failures: Vec<(Transaction, TransactionError)> = vec![
        (Transaction::new(), TransactionError::Empty),
        (
            Transaction::from(vec![Edit::insert(a, 0, "x"), Edit::insert(unknown, 0, "x")]),
            TransactionError::UnknownBuffer {
                edit: 1,
                buffer: unknown,
            },
        ),
        (
            Transaction::from(vec![Edit::insert(b, 0, "x"), Edit::delete(a, 2..3)]),
            TransactionError::InvalidRange {
                edit: 1,
                error: helm_text::TextError::NotCharBoundary { offset: 2 },
            },
        ),
        (
            Transaction::from(vec![Edit::delete(b, 0..3), Edit::delete(b, 2..4)]),
            TransactionError::Overlap { edit: 1, other: 0 },
        ),
    ];
    for (transaction, error) in failures {
        assert_eq!(set.apply(transaction), Err(error));
        check(&set, [0, 0, 0]);
    }

    // Undo and redo of an unknown buffer.
    assert_eq!(
        set.undo(unknown),
        Err(HistoryError::UnknownBuffer { buffer: unknown })
    );
    assert_eq!(
        set.redo(unknown),
        Err(HistoryError::UnknownBuffer { buffer: unknown })
    );
    check(&set, [0, 0, 0]);

    // A blocked undo: `t` touched `a` and `b`, and a later one touched `b`.
    let t = apply(
        &mut set,
        vec![Edit::insert(a, 0, "1"), Edit::insert(b, 0, "1")],
    );
    apply(&mut set, vec![Edit::insert(b, 0, "2")]);
    check(&set, [1, 2, 0]);
    assert_eq!(
        set.undo(a),
        Err(HistoryError::Blocked {
            transaction: t,
            buffer: b,
        })
    );
    check(&set, [1, 2, 0]);

    // Getting a buffer, its version or a snapshot changes nothing.
    let _ = set.get(a);
    let _ = set.version(a);
    let _ = set.snapshot(a);
    check(&set, [1, 2, 0]);

    // `a` and `b` changed since the snapshots; `c` never did.
    assert!(snaps[0].is_stale());
    assert!(snaps[1].is_stale());
    assert!(!snaps[2].is_stale());
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn blocked_redo_leaves_versions_alone() {
    let (mut set, ids) = set_of(&["x", "y"]);
    let (x, y) = (ids[0], ids[1]);
    let p = apply(&mut set, vec![Edit::insert(x, 1, "1")]);
    let q = apply(
        &mut set,
        vec![Edit::insert(x, 2, "2"), Edit::insert(y, 1, "2")],
    );
    assert_eq!(set.undo(y), Ok(Some(q)));
    assert_eq!(set.undo(x), Ok(Some(p)));
    assert_eq!(versions(&set, &ids), [4, 2]);
    let snaps = [snapshot(&set, x), snapshot(&set, y)];

    // `x`'s redo history has `p` on top of `q`, so redoing `q` from `y` is
    // blocked by `x`.
    assert_eq!(
        set.redo(y),
        Err(HistoryError::Blocked {
            transaction: q,
            buffer: x,
        })
    );
    assert_eq!(versions(&set, &ids), [4, 2]);
    assert!(snaps.iter().all(|s| !s.is_stale()));
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_carries_its_buffer_and_version() {
    let (mut set, ids) = set_of(&["one", "two"]);
    let (a, b) = (ids[0], ids[1]);
    apply(&mut set, vec![Edit::insert(b, 3, "!")]);
    apply(&mut set, vec![Edit::insert(b, 4, "!")]);

    let sa = snapshot(&set, a);
    let sb = snapshot(&set, b);
    assert_eq!(sa.buffer(), a);
    assert_eq!(sb.buffer(), b);
    assert_eq!(sa.version(), set.version(a).unwrap());
    assert_eq!(sb.version(), set.version(b).unwrap());
    assert_eq!(sa.version().get(), 0);
    assert_eq!(sb.version().get(), 2);
    assert_eq!(sb.to_string(), "two!!");
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_keeps_its_text_through_apply_undo_and_redo() {
    let (mut set, ids) = set_of(&["fn main() {}\r\n"]);
    let a = ids[0];
    let s0 = snapshot(&set, a);

    let t = apply(
        &mut set,
        vec![Edit::insert(a, 11, " 😀 "), Edit::delete(a, 0..3)],
    );
    let s1 = snapshot(&set, a);
    assert_eq!(set.undo(a), Ok(Some(t)));
    let s2 = snapshot(&set, a);
    assert_eq!(set.redo(a), Ok(Some(t)));
    let s3 = snapshot(&set, a);
    let len = text(&set, a).len();
    apply(&mut set, vec![Edit::delete(a, 0..len)]);

    let expected = [
        (&s0, 0, "fn main() {}\r\n"),
        (&s1, 1, "main() { 😀 }\r\n"),
        (&s2, 2, "fn main() {}\r\n"),
        (&s3, 3, "main() { 😀 }\r\n"),
    ];
    for (snap, version, text) in expected {
        assert_eq!(snap.version().get(), version);
        assert_eq!(snap.to_string(), text);
        assert_eq!(snap.len_bytes(), text.len());
        assert!(snap.is_stale());
    }
    assert_eq!(text(&set, a), "");
}

/// Every `TextBuffer` method of `snap` returns what it returns on a fresh
/// buffer holding `text`, for every offset, line and column up to a little
/// past the end, valid or not.
fn assert_reads_like(snap: &Snapshot, text: &str) {
    let buffer = Buffer::from(text);
    assert_eq!(snap.len_bytes(), buffer.len_bytes());
    assert_eq!(snap.len_chars(), buffer.len_chars());
    assert_eq!(snap.len_utf16(), buffer.len_utf16());
    assert_eq!(snap.len_lines(), buffer.len_lines());

    for i in (0..text.len() + 3).chain([usize::MAX]) {
        assert_eq!(snap.byte_to_char(i), buffer.byte_to_char(i), "byte {i}");
        assert_eq!(snap.char_to_byte(i), buffer.char_to_byte(i), "char {i}");
        assert_eq!(snap.byte_to_utf16(i), buffer.byte_to_utf16(i), "byte {i}");
        assert_eq!(snap.utf16_to_byte(i), buffer.utf16_to_byte(i), "utf16 {i}");
        assert_eq!(snap.byte_to_line(i), buffer.byte_to_line(i), "byte {i}");
        assert_eq!(snap.byte_to_point(i), buffer.byte_to_point(i), "byte {i}");
        assert_eq!(
            snap.byte_to_utf16_point(i),
            buffer.byte_to_utf16_point(i),
            "byte {i}"
        );
        assert_eq!(snap.slice(i..text.len()), buffer.slice(i..text.len()));
        assert_eq!(snap.slice(0..i), buffer.slice(0..i));
    }
    // A reversed range.
    let (start, end) = (3, 1);
    assert_eq!(snap.slice(start..end), buffer.slice(start..end));

    for line in (0..buffer.len_lines() + 2).chain([usize::MAX]) {
        assert_eq!(snap.line(line), buffer.line(line), "line {line}");
        assert_eq!(snap.line_to_byte(line), buffer.line_to_byte(line));
        for column in (0..8).chain([usize::MAX]) {
            assert_eq!(
                snap.point_to_byte(Point::new(line, column)),
                buffer.point_to_byte(Point::new(line, column)),
                "point {line}:{column}"
            );
            assert_eq!(
                snap.utf16_point_to_byte(Utf16Point::new(line, column)),
                buffer.utf16_point_to_byte(Utf16Point::new(line, column)),
                "utf16 point {line}:{column}"
            );
        }
    }
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_reads_like_the_buffer_did() {
    let start = "é中😀\r\nab\rc\n\u{2028}x😀";
    let (mut set, ids) = set_of(&[start, ""]);
    let (a, empty) = (ids[0], ids[1]);
    let s0 = snapshot(&set, a);
    let s_empty = snapshot(&set, empty);

    apply(
        &mut set,
        vec![Edit::replace(a, 0..2, "\n"), Edit::insert(a, 9, "\r")],
    );
    let after = text(&set, a);
    let s1 = snapshot(&set, a);
    apply(&mut set, vec![Edit::insert(empty, 0, "now full")]);

    assert_reads_like(&s0, start);
    assert_reads_like(&s1, &after);
    assert_reads_like(&s_empty, "");
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_display_writes_the_whole_text() {
    let text_in = "line one\r\nline two 中\rline three 😀\n";
    let (set, ids) = set_of(&[text_in]);
    let snap = snapshot(&set, ids[0]);
    assert_eq!(snap.to_string(), text_in);
    assert_eq!(format!("{snap}"), text_in);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_clones_share_text_and_version() {
    let (mut set, ids) = set_of(&["abc"]);
    let a = ids[0];
    apply(&mut set, vec![Edit::insert(a, 3, "d")]);
    let snap = snapshot(&set, a);
    let clone = snap.clone();
    apply(&mut set, vec![Edit::insert(a, 4, "e")]);

    assert_eq!(clone.to_string(), "abcd");
    assert_eq!(clone.version(), snap.version());
    assert_eq!(clone.buffer(), snap.buffer());
    assert!(clone.is_stale());
    drop(snap);
    assert_eq!(clone.to_string(), "abcd");
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn two_snapshots_at_one_version_agree() {
    let (mut set, ids) = set_of(&["abc", "other"]);
    let (a, b) = (ids[0], ids[1]);
    apply(&mut set, vec![Edit::insert(a, 0, ">")]);
    let first = snapshot(&set, a);
    // A change to another buffer doesn't change `a`'s version.
    apply(&mut set, vec![Edit::insert(b, 0, ">")]);
    let second = snapshot(&set, a);

    assert_eq!(first.version(), second.version());
    assert_eq!(first.to_string(), second.to_string());
    assert!(!first.is_stale());
    assert!(!second.is_stale());
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_goes_stale_when_its_own_buffer_changes() {
    let (mut set, ids) = set_of(&["a", "b"]);
    let (a, b) = (ids[0], ids[1]);
    let sa = snapshot(&set, a);
    let sb = snapshot(&set, b);
    assert!(!sa.is_stale());
    assert!(!sb.is_stale());

    apply(&mut set, vec![Edit::insert(a, 1, "!")]);
    assert!(sa.is_stale());
    assert!(!sb.is_stale());

    let sa1 = snapshot(&set, a);
    assert!(!sa1.is_stale());
    apply(&mut set, vec![Edit::insert(b, 1, "!")]);
    assert!(!sa1.is_stale());
    assert!(sb.is_stale());
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn stale_stays_stale_when_undo_restores_the_text() {
    let (mut set, ids) = set_of(&["text"]);
    let a = ids[0];
    let snap = snapshot(&set, a);
    let t = apply(&mut set, vec![Edit::insert(a, 4, "!")]);
    assert!(snap.is_stale());

    assert_eq!(set.undo(a), Ok(Some(t)));
    assert_eq!(text(&set, a), snap.to_string());
    assert!(snap.is_stale());

    let after_undo = snapshot(&set, a);
    assert_eq!(set.redo(a), Ok(Some(t)));
    assert_eq!(set.undo(a), Ok(Some(t)));
    assert!(after_undo.is_stale());
    assert!(snap.is_stale());
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn failed_calls_leave_snapshots_current() {
    let (mut set, ids) = set_of(&["abc", "def"]);
    let (a, b) = (ids[0], ids[1]);
    let t = apply(
        &mut set,
        vec![Edit::insert(a, 0, "1"), Edit::insert(b, 0, "1")],
    );
    apply(&mut set, vec![Edit::insert(b, 0, "2")]);
    let snaps = [snapshot(&set, a), snapshot(&set, b)];

    assert_eq!(set.apply(Transaction::new()), Err(TransactionError::Empty));
    assert!(
        set.apply(Transaction::from(vec![Edit::delete(a, 9..10)]))
            .is_err()
    );
    assert_eq!(set.redo(a), Ok(None));
    assert_eq!(
        set.undo(a),
        Err(HistoryError::Blocked {
            transaction: t,
            buffer: b,
        })
    );
    assert!(snaps.iter().all(|s| !s.is_stale()));
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_outlives_its_set() {
    let (mut set, ids) = set_of(&["first", "second"]);
    let (a, b) = (ids[0], ids[1]);
    let old = snapshot(&set, a);
    apply(&mut set, vec![Edit::insert(a, 5, "!")]);
    let latest_a = snapshot(&set, a);
    let latest_b = snapshot(&set, b);
    drop(set);

    assert_eq!(old.to_string(), "first");
    assert_eq!(latest_a.to_string(), "first!");
    assert_eq!(latest_b.to_string(), "second");
    assert_eq!(latest_a.line(0), Ok("first!".to_owned()));
    // Staleness keeps its answer as of the set's last change.
    assert!(old.is_stale());
    assert!(!latest_a.is_stale());
    assert!(!latest_b.is_stale());
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_reads_on_another_thread() {
    let (mut set, ids) = set_of(&["shared 中 text\n"]);
    let a = ids[0];
    let snap = snapshot(&set, a);

    // One snapshot read by several threads at once, while it is current.
    thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                assert_eq!(snap.to_string(), "shared 中 text\n");
                assert_eq!(snap.len_lines(), 2);
                assert!(!snap.is_stale());
            });
        }
    });

    // Moved to a thread that sees a change made before it started.
    apply(&mut set, vec![Edit::insert(a, 0, "edited ")]);
    let moved = snap.clone();
    let handle = thread::spawn(move || (moved.to_string(), moved.is_stale(), moved.version()));
    let (text_seen, stale, seen_version) = handle.join().expect("reader thread");
    assert_eq!(text_seen, "shared 中 text\n");
    assert!(stale);
    assert_eq!(seen_version.get(), 0);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshot_of_a_large_buffer_keeps_its_text() {
    // About 1 MiB, so the rope has many nodes for edits to copy.
    let line = "let café = \"中😀\";\r\n";
    let start = line.repeat((1 << 20) / line.len());
    let (mut set, ids) = set_of(&[&start]);
    let a = ids[0];
    let snaps: Vec<Snapshot> = (0..1000).map(|_| snapshot(&set, a)).collect();

    let middle = (start.len() / 2 / line.len()) * line.len();
    apply(
        &mut set,
        vec![Edit::replace(a, middle..middle + line.len(), "X")],
    );
    apply(&mut set, vec![Edit::insert(a, 0, "head\n")]);

    for snap in &snaps {
        assert_eq!(snap.len_bytes(), start.len());
        assert!(snap.is_stale());
    }
    assert_eq!(snaps[0].to_string(), start);
    assert_eq!(snaps[999].to_string(), start);
    assert_eq!(
        snaps[500].slice(middle..middle + line.len()),
        Ok(line.to_owned())
    );
    assert_eq!(text(&set, a).len(), start.len() - line.len() + 1 + 5);
}
