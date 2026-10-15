//! T-103 acceptance tests: a stress test of concurrent readers. Reader
//! threads hold and re-read snapshots while the main thread applies, undoes
//! and redoes transactions across several buffers. Every snapshot must keep
//! its own text and version the whole time, and report itself stale exactly
//! when its buffer has moved on.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread;

use helm_text::{Buffer, BufferId, BufferSet, Edit, Snapshot, TextBuffer, Transaction};

/// Buffers the main thread edits.
const BUFFERS: usize = 3;

/// Reader threads. Every reader gets every snapshot, so several threads
/// read the same snapshot at once.
const READERS: usize = 4;

/// Steps per seed. A step is one apply, undo or redo.
const STEPS: usize = 1500;

/// Snapshots each reader holds at once. Older ones are checked one last
/// time and dropped.
const HELD: usize = 48;

/// Seeds for the step generator, so a failure replays exactly.
const SEEDS: [u64; 3] = [0x5eed_0001, 0xdead_beef_cafe, 0x0123_4567_89ab_cdef];

/// The lines starting texts repeat: ASCII, é, 中, 😀, LF, CRLF and a lone
/// CR.
const LINES: &str = "fn main() {\n    let café = \"中文\";\r\n    // 😀 done\r}\n";

/// Text an edit inserts, picked at random.
const PIECES: [&str; 9] = ["x", "hello ", "é", "中", "😀", "\n", "\r\n", "\r", ""];

/// splitmix64: a small, fixed random number generator, so the test needs no
/// new dependency and every run with a seed takes the same steps.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A number in `0..n`.
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A snapshot with what it must hold: the version and text its buffer had
/// when the main thread took it.
struct Expected {
    snapshot: Snapshot,
    buffer: usize,
    version: u64,
    text: Arc<str>,
}

/// What the main thread sends readers.
enum Message {
    /// A new snapshot to hold.
    Snapshot(Arc<Expected>),
    /// The main thread is done; these are the buffers' final versions.
    Done(Vec<u64>),
}

/// Checks that `held` still has its id, version and length, plus one slice
/// picked by `probe`. Cheap enough to run on every held snapshot after every
/// message.
fn check_quick(held: &Expected, ids: &[BufferId], probe: usize) {
    let snap = &held.snapshot;
    assert_eq!(snap.buffer(), ids[held.buffer]);
    assert_eq!(snap.version().get(), held.version);
    assert_eq!(snap.len_bytes(), held.text.len());

    // A slice of up to 64 bytes between two char boundaries.
    let text = &*held.text;
    let mut start = probe % (text.len() + 1);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (start + 64).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    assert_eq!(snap.slice(start..end).as_deref(), Ok(&text[start..end]));
}

/// The lines of `text`, each with its line break: LF, CRLF and lone CR end
/// a line, as `TextBuffer` counts them. A text with `k` breaks has `k + 1`
/// lines.
fn lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let (mut start, mut i) = (0, 0);
    while i < bytes.len() {
        let end = match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => i + 2,
            b'\r' | b'\n' => i + 1,
            _ => {
                i += 1;
                continue;
            }
        };
        lines.push(&text[start..end]);
        start = end;
        i = end;
    }
    lines.push(&text[start..]);
    lines
}

/// Checks the whole text of `held`, its lengths and one line.
fn check_full(held: &Expected, ids: &[BufferId], probe: usize) {
    check_quick(held, ids, probe);
    let snap = &held.snapshot;
    let text = &*held.text;
    assert_eq!(snap.to_string(), text);
    assert_eq!(snap.len_chars(), text.chars().count());
    assert_eq!(snap.len_utf16(), text.encode_utf16().count());
    let lines = lines(text);
    assert_eq!(snap.len_lines(), lines.len());
    let line = probe % lines.len();
    assert_eq!(snap.line(line).as_deref(), Ok(lines[line]));
}

/// One reader: holds up to `HELD` snapshots, re-checks them after every
/// message, and checks staleness against what the channel tells it.
fn reader(receiver: Receiver<Message>, ids: Vec<BufferId>) -> usize {
    let mut held: VecDeque<Arc<Expected>> = VecDeque::new();
    // The newest version this reader has received for each buffer.
    let mut newest = [0u64; BUFFERS];
    let mut probe = 0usize;
    let mut checked = 0;

    while let Ok(message) = receiver.recv() {
        match message {
            Message::Snapshot(expected) => {
                probe = probe.wrapping_mul(31).wrapping_add(17);
                check_full(&expected, &ids, probe);

                // The main thread changed the buffer to this version before
                // sending it, and receiving the message makes that change
                // visible here. So every held snapshot of the same buffer at
                // an older version must read as stale now.
                let b = expected.buffer;
                assert!(expected.version >= newest[b], "versions only grow");
                newest[b] = expected.version;
                for older in &held {
                    if older.buffer == b && older.version < expected.version {
                        assert!(
                            older.snapshot.is_stale(),
                            "buffer {b} version {} not stale after seeing version {}",
                            older.version,
                            expected.version
                        );
                    }
                }

                held.push_back(expected);
                if held.len() > HELD {
                    let oldest = held.pop_front().expect("held is not empty");
                    check_full(&oldest, &ids, probe);
                    checked += 1;
                }
                for (i, h) in held.iter().enumerate() {
                    check_quick(h, &ids, probe.wrapping_add(i * 977));
                }
            }
            Message::Done(versions) => {
                // Every change happened before `Done` was sent, so staleness
                // is now exact.
                for h in &held {
                    check_full(h, &ids, probe);
                    assert_eq!(
                        h.snapshot.is_stale(),
                        h.version != versions[h.buffer],
                        "buffer {} version {} against final version {}",
                        h.buffer,
                        h.version,
                        versions[h.buffer]
                    );
                    checked += 1;
                }
                held.clear();
            }
        }
    }
    checked
}

/// A random transaction: one edit on each of one to three distinct buffers,
/// each replacing up to three chars at a random char offset with a random
/// piece. Returns it with the buffers it touches.
fn random_transaction(
    rng: &mut Rng,
    set: &BufferSet,
    ids: &[BufferId],
) -> (Transaction, Vec<usize>) {
    let mut buffers: Vec<usize> = (0..BUFFERS).collect();
    for i in (1..BUFFERS).rev() {
        buffers.swap(i, rng.below(i + 1));
    }
    buffers.truncate(1 + rng.below(BUFFERS));
    buffers.sort_unstable();

    let edits = buffers
        .iter()
        .map(|&b| {
            let buffer = set.get(ids[b]).expect("buffer is in the set");
            let chars = buffer.len_chars();
            let start = rng.below(chars + 1);
            let end = (start + rng.below(4)).min(chars);
            let range = buffer.char_to_byte(start).unwrap()..buffer.char_to_byte(end).unwrap();
            Edit::replace(ids[b], range, PIECES[rng.below(PIECES.len())])
        })
        .collect();
    (edits, buffers)
}

/// Runs `STEPS` random steps with `READERS` readers for one seed.
fn stress(seed: u64) {
    let mut rng = Rng(seed);
    let mut set = BufferSet::new();
    // 8 to 16 KiB each, so every rope has many nodes for edits to copy.
    let ids: Vec<BufferId> = (0..BUFFERS)
        .map(|i| set.add(Buffer::from(LINES.repeat(160 + 80 * i).as_str())))
        .collect();

    // The model: each buffer's version, and the buffers each transaction
    // touched, to know which versions an undo or redo changes.
    let mut versions = vec![0u64; BUFFERS];
    let mut touched_by = HashMap::new();
    // The text of each buffer at its current version, shared by every
    // snapshot taken at that version.
    let mut texts: Vec<Arc<str>> = ids
        .iter()
        .map(|&id| Arc::from(set.get(id).unwrap().to_string()))
        .collect();
    // The last snapshot the main thread took of each buffer.
    let mut last: Vec<Snapshot> = ids.iter().map(|&id| set.snapshot(id).unwrap()).collect();

    thread::scope(|scope| {
        let mut senders: Vec<SyncSender<Message>> = Vec::new();
        let mut readers = Vec::new();
        for _ in 0..READERS {
            let (sender, receiver) = sync_channel(64);
            let ids = ids.clone();
            senders.push(sender);
            readers.push(scope.spawn(move || reader(receiver, ids)));
        }
        let send = |senders: &[SyncSender<Message>], expected: Expected| {
            let expected = Arc::new(expected);
            for sender in senders {
                sender
                    .send(Message::Snapshot(Arc::clone(&expected)))
                    .expect("reader is running");
            }
        };

        for step in 0..STEPS {
            let changed: Vec<usize> = match rng.below(4) {
                0 | 1 => {
                    let (transaction, buffers) = random_transaction(&mut rng, &set, &ids);
                    let id = set.apply(transaction).expect("transaction is valid");
                    touched_by.insert(id, buffers.clone());
                    buffers
                }
                2 => match set.undo(ids[rng.below(BUFFERS)]) {
                    Ok(Some(id)) => touched_by[&id].clone(),
                    Ok(None) | Err(_) => Vec::new(),
                },
                _ => match set.redo(ids[rng.below(BUFFERS)]) {
                    Ok(Some(id)) => touched_by[&id].clone(),
                    Ok(None) | Err(_) => Vec::new(),
                },
            };

            for &b in &changed {
                versions[b] += 1;
                texts[b] = Arc::from(set.get(ids[b]).unwrap().to_string());
                assert!(last[b].is_stale(), "step {step}: buffer {b} changed");
            }
            for b in 0..BUFFERS {
                assert_eq!(
                    set.version(ids[b]).map(|v| v.get()),
                    Some(versions[b]),
                    "step {step}: buffer {b}"
                );
                if !changed.contains(&b) {
                    assert!(!last[b].is_stale(), "step {step}: buffer {b} unchanged");
                }
            }

            // Snapshot every changed buffer, plus one random buffer, which
            // may be unchanged and so a second snapshot at one version.
            let mut to_snapshot = changed;
            to_snapshot.push(rng.below(BUFFERS));
            for b in to_snapshot {
                let snapshot = set.snapshot(ids[b]).unwrap();
                assert_eq!(snapshot.version().get(), versions[b]);
                assert_eq!(snapshot.buffer(), ids[b]);
                last[b] = snapshot.clone();
                send(
                    &senders,
                    Expected {
                        snapshot,
                        buffer: b,
                        version: versions[b],
                        text: Arc::clone(&texts[b]),
                    },
                );
            }
        }

        for sender in &senders {
            sender
                .send(Message::Done(versions.clone()))
                .expect("reader is running");
        }
        drop(senders);
        for reader in readers {
            let checked = reader.join().expect("reader thread");
            assert!(
                checked > STEPS / 2,
                "reader checked only {checked} snapshots"
            );
        }
    });

    // Every buffer still matches the last text the readers were told about.
    for b in 0..BUFFERS {
        assert_eq!(set.get(ids[b]).unwrap().to_string(), *texts[b]);
        assert_eq!(last[b].to_string(), *texts[b]);
    }
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn readers_keep_their_snapshots_while_the_main_thread_edits_seed_0() {
    stress(SEEDS[0]);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn readers_keep_their_snapshots_while_the_main_thread_edits_seed_1() {
    stress(SEEDS[1]);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn readers_keep_their_snapshots_while_the_main_thread_edits_seed_2() {
    stress(SEEDS[2]);
}

#[test]
#[ignore = "T-103: not implemented yet"]
fn snapshots_dropped_on_reader_threads_after_the_set_is_gone() {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(LINES.repeat(400).as_str()));
    let start = set.get(id).unwrap().to_string();
    let (sender, receiver) = sync_channel::<Snapshot>(0);

    thread::scope(|scope| {
        let start = &start;
        let reader = scope.spawn(move || {
            let mut kept = Vec::new();
            while let Ok(snapshot) = receiver.recv() {
                kept.push(snapshot);
            }
            // The set and every later version are gone; the snapshots still
            // read, and are freed here, on this thread.
            for (i, snapshot) in kept.iter().enumerate() {
                assert_eq!(snapshot.version().get(), i as u64 * 2);
                assert_eq!(snapshot.to_string(), *start);
            }
            kept.len()
        });

        for _ in 0..200 {
            sender
                .send(set.snapshot(id).unwrap())
                .expect("reader is running");
            set.apply(Transaction::from(vec![Edit::insert(id, 0, "😀\r\n")]))
                .unwrap();
            set.undo(id).unwrap();
        }
        drop(sender);
        drop(set);
        assert_eq!(reader.join().expect("reader thread"), 200);
    });
}
