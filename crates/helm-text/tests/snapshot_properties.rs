//! T-103 acceptance tests: property tests of versions and snapshots. Any
//! random sequence of transactions, undos and redos over several buffers
//! moves each buffer's version exactly as the rules say, and every snapshot
//! taken along the way keeps its text and version, reads like a buffer
//! holding that text, and is stale exactly when its buffer has moved on.

use std::collections::HashMap;

use helm_text::{Buffer, BufferId, BufferSet, Edit, Snapshot, TextBuffer, Transaction};
use proptest::prelude::*;
use proptest::sample::Index;

/// How many buffers each case edits.
const BUFFERS: usize = 3;

/// One piece of random text: ASCII runs, each character width, each line
/// break, and any char.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        6 => "[a-z ]{1,4}",
        1 => Just("\n".to_owned()),
        1 => Just("\r".to_owned()),
        1 => Just("\r\n".to_owned()),
        1 => Just("é".to_owned()),
        1 => Just("中".to_owned()),
        1 => Just("😀".to_owned()),
        1 => any::<char>().prop_map(String::from),
    ]
}

/// Random text of up to `max_pieces` pieces.
fn text(max_pieces: usize) -> impl Strategy<Value = String> {
    prop::collection::vec(piece(), 0..max_pieces).prop_map(|pieces| pieces.concat())
}

/// A starting text: short, or a short text repeated up to a few KiB so the
/// rope has more than one node.
fn start() -> impl Strategy<Value = String> {
    (text(12), prop_oneof![3 => Just(1usize), 1 => 1usize..120])
        .prop_map(|(text, times)| text.repeat(times))
}

/// One random edit: a buffer, two char-boundary picks for the range, and
/// the text to put there.
#[derive(Clone, Debug)]
struct EditSpec {
    buffer: usize,
    start: Index,
    end: Index,
    text: String,
}

/// One random step.
#[derive(Clone, Debug)]
enum Op {
    /// A transaction of these edits. Some overlap or are empty and fail.
    Apply(Vec<EditSpec>),
    Undo(usize),
    Redo(usize),
}

fn edit_spec() -> impl Strategy<Value = EditSpec> {
    (0..BUFFERS, any::<Index>(), any::<Index>(), text(3)).prop_map(|(buffer, start, end, text)| {
        EditSpec {
            buffer,
            start,
            end,
            text,
        }
    })
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => prop::collection::vec(edit_spec(), 0..4).prop_map(Op::Apply),
        3 => (0..BUFFERS).prop_map(Op::Undo),
        2 => (0..BUFFERS).prop_map(Op::Redo),
    ]
}

/// Builds the transaction for `specs` against the set's current texts.
/// Ranges sit on char boundaries, in order; edits may still overlap.
fn build(set: &BufferSet, ids: &[BufferId], specs: &[EditSpec]) -> Transaction {
    specs
        .iter()
        .map(|spec| {
            let text = set.get(ids[spec.buffer]).unwrap().to_string();
            let boundaries: Vec<usize> = text
                .char_indices()
                .map(|(byte, _)| byte)
                .chain([text.len()])
                .collect();
            let a = *spec.start.get(&boundaries);
            let b = *spec.end.get(&boundaries);
            Edit::replace(ids[spec.buffer], a.min(b)..a.max(b), spec.text.clone())
        })
        .collect()
}

/// A snapshot and what it must hold.
struct Kept {
    snapshot: Snapshot,
    buffer: usize,
    version: u64,
    text: String,
}

/// Checks `kept` against its recorded text and version, and its staleness
/// against the buffer's current version in the model.
fn check(kept: &Kept, ids: &[BufferId], current: &[u64]) -> Result<(), TestCaseError> {
    let snap = &kept.snapshot;
    prop_assert_eq!(snap.buffer(), ids[kept.buffer]);
    prop_assert_eq!(snap.version().get(), kept.version);
    prop_assert_eq!(snap.to_string(), kept.text.as_str());
    prop_assert_eq!(snap.is_stale(), kept.version != current[kept.buffer]);
    Ok(())
}

/// Checks every `TextBuffer` method of `snap` against `buffer`, a clone of
/// the buffer taken with the snapshot, at up to 16 sampled lines and 16
/// sampled offsets, plus both ends and one past the end of each.
fn check_reads(snap: &Snapshot, buffer: &Buffer) -> Result<(), TestCaseError> {
    prop_assert_eq!(snap.to_string(), buffer.to_string());
    prop_assert_eq!(snap.len_bytes(), buffer.len_bytes());
    prop_assert_eq!(snap.len_chars(), buffer.len_chars());
    prop_assert_eq!(snap.len_utf16(), buffer.len_utf16());
    prop_assert_eq!(snap.len_lines(), buffer.len_lines());
    let lines = buffer.len_lines();
    let line_step = (lines / 16).max(1);
    for line in (0..=lines).step_by(line_step).chain([lines - 1, lines]) {
        prop_assert_eq!(snap.line(line), buffer.line(line));
        prop_assert_eq!(snap.line_to_byte(line), buffer.line_to_byte(line));
    }
    let len = buffer.len_bytes();
    let step = (len / 16).max(1);
    for i in (0..=len + 1).step_by(step).chain([len, len + 1]) {
        prop_assert_eq!(snap.byte_to_char(i), buffer.byte_to_char(i));
        prop_assert_eq!(snap.char_to_byte(i), buffer.char_to_byte(i));
        prop_assert_eq!(snap.byte_to_utf16(i), buffer.byte_to_utf16(i));
        prop_assert_eq!(snap.utf16_to_byte(i), buffer.utf16_to_byte(i));
        prop_assert_eq!(snap.byte_to_line(i), buffer.byte_to_line(i));
        prop_assert_eq!(snap.byte_to_point(i), buffer.byte_to_point(i));
        prop_assert_eq!(snap.byte_to_utf16_point(i), buffer.byte_to_utf16_point(i));
        prop_assert_eq!(snap.slice(i / 2..i), buffer.slice(i / 2..i));
        if let Ok(point) = buffer.byte_to_point(i) {
            prop_assert_eq!(snap.point_to_byte(point), Ok(i));
        }
        if let Ok(point) = buffer.byte_to_utf16_point(i) {
            prop_assert_eq!(snap.utf16_point_to_byte(point), Ok(i));
        }
    }
    Ok(())
}

/// Runs `ops`, snapshotting every buffer after every step, and checks the
/// model's versions and every snapshot kept so far after each step.
fn run(starts: &[String], ops: &[Op]) -> Result<(), TestCaseError> {
    let mut set = BufferSet::new();
    let ids: Vec<BufferId> = starts
        .iter()
        .map(|text| set.add(Buffer::from(text.as_str())))
        .collect();
    let mut versions = vec![0u64; BUFFERS];
    let mut touched_by = HashMap::new();
    let mut kept: Vec<Kept> = Vec::new();

    let take = |set: &BufferSet, versions: &[u64], kept: &mut Vec<Kept>| {
        for (b, &id) in ids.iter().enumerate() {
            let snapshot = set.snapshot(id).unwrap();
            kept.push(Kept {
                snapshot,
                buffer: b,
                version: versions[b],
                text: set.get(id).unwrap().to_string(),
            });
        }
    };
    take(&set, &versions, &mut kept);

    for op in ops {
        let changed: Vec<usize> = match op {
            Op::Apply(specs) => match set.apply(build(&set, &ids, specs)) {
                Ok(id) => {
                    let mut buffers: Vec<usize> = specs.iter().map(|s| s.buffer).collect();
                    buffers.sort_unstable();
                    buffers.dedup();
                    touched_by.insert(id, buffers.clone());
                    buffers
                }
                Err(_) => Vec::new(),
            },
            Op::Undo(b) => match set.undo(ids[*b]) {
                Ok(Some(id)) => touched_by[&id].clone(),
                _ => Vec::new(),
            },
            Op::Redo(b) => match set.redo(ids[*b]) {
                Ok(Some(id)) => touched_by[&id].clone(),
                _ => Vec::new(),
            },
        };
        for b in changed {
            versions[b] += 1;
        }
        for (b, &id) in ids.iter().enumerate() {
            prop_assert_eq!(set.version(id).map(|v| v.get()), Some(versions[b]));
        }
        for k in &kept {
            check(k, &ids, &versions)?;
        }
        take(&set, &versions, &mut kept);
    }

    // The snapshots outlive the set and keep their answers.
    drop(set);
    for k in &kept {
        check(k, &ids, &versions)?;
    }
    Ok(())
}

fn starts() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(start(), BUFFERS)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn versions_and_snapshots_follow_the_model(
        starts in starts(),
        ops in prop::collection::vec(op(), 1..40),
    ) {
        run(&starts, &ops)?;
    }
}

proptest! {
    // Fewer cases and steps: every snapshot is checked method by method, and
    // ropey is slow in a debug build. `snapshots.rs` checks every offset.
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn snapshots_read_like_a_buffer_of_their_text(
        starts in starts(),
        ops in prop::collection::vec(op(), 1..8),
    ) {
        let mut set = BufferSet::new();
        let ids: Vec<BufferId> = starts
            .iter()
            .map(|text| set.add(Buffer::from(text.as_str())))
            .collect();
        let mut kept = Vec::new();
        for op in &ops {
            for &id in &ids {
                kept.push((set.snapshot(id).unwrap(), set.get(id).unwrap().clone()));
            }
            match op {
                Op::Apply(specs) => {
                    let _ = set.apply(build(&set, &ids, specs));
                }
                Op::Undo(b) => {
                    let _ = set.undo(ids[*b]);
                }
                Op::Redo(b) => {
                    let _ = set.redo(ids[*b]);
                }
            }
        }
        for (snap, buffer) in &kept {
            check_reads(snap, buffer)?;
        }
    }
}
