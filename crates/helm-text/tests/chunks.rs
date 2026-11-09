//! `TextBuffer::chunk_at`, added to PR #34 (T-107) under Khakha's scope
//! grant: chunks cover the text in order with no gaps or overlaps, start
//! and end on char boundaries, hold the byte they are asked for, and come
//! back the same from a `Buffer` and from a `Snapshot` of it, for every
//! offset from 0 to the length.

use helm_text::{Buffer, BufferSet, Edit, TextBuffer, TextError, Transaction};
use proptest::prelude::*;

/// Several KB of text with every line break, every char width, a long run
/// of one word, blank and indented lines, so the rope stores it in many
/// chunks.
fn several_kb() -> String {
    let mut text = String::new();
    for i in 0..60 {
        text.push_str(&format!("    let line_{i} = \"é中😀 crème\";\r\n"));
        text.push_str("\tfoo(bar, [1, 2]) // comment\n");
        text.push_str("lone\rcr\r");
        text.push('\n');
        if i % 20 == 0 {
            text.push_str(&"abcdefghij".repeat(150));
            text.push('\n');
        }
    }
    text
}

/// Every chunk of `buffer`, walking from offset 0, with its start.
fn walk(buffer: &impl TextBuffer) -> Vec<(String, usize)> {
    let len = buffer.len_bytes();
    let mut chunks = Vec::new();
    let mut at = 0;
    loop {
        let (chunk, start) = buffer.chunk_at(at).expect("in bounds");
        if at == len {
            assert_eq!(start + chunk.len(), len, "the chunk at the end ends there");
            if len == 0 {
                assert_eq!((chunk, start), ("", 0));
            }
            break;
        }
        assert_eq!(
            start, at,
            "walking from a chunk's end lands on the next chunk's start"
        );
        assert!(!chunk.is_empty(), "a chunk holding a byte isn't empty");
        chunks.push((chunk.to_owned(), start));
        at = start + chunk.len();
    }
    chunks
}

/// Checks every property `chunk_at` documents, for `buffer` holding `text`.
/// Returns how many chunks the text is in.
fn check(buffer: &impl TextBuffer, text: &str) -> usize {
    let len = text.len();
    assert_eq!(buffer.len_bytes(), len);
    let chunks = walk(buffer);

    // In order, no gaps, no overlaps, and joined they are the text.
    let joined: String = chunks.iter().map(|(chunk, _)| chunk.as_str()).collect();
    assert_eq!(joined, text);
    let mut expected_start = 0;
    for (chunk, start) in &chunks {
        assert_eq!(*start, expected_start);
        // Both ends on char boundaries of the whole text.
        assert!(text.is_char_boundary(*start));
        assert!(text.is_char_boundary(start + chunk.len()));
        expected_start = start + chunk.len();
    }
    assert_eq!(expected_start, len);

    // Every offset, inside chars and at chunk edges included, gets the
    // chunk that holds its byte, or the last one at the end.
    for byte in 0..=len {
        let (chunk, start) = buffer.chunk_at(byte).expect("in bounds");
        if byte < len {
            assert!(start <= byte && byte < start + chunk.len(), "byte {byte}");
            let i = chunks.partition_point(|(_, s)| *s <= byte) - 1;
            assert_eq!(
                (chunk, start),
                (chunks[i].0.as_str(), chunks[i].1),
                "byte {byte}"
            );
        } else {
            assert_eq!(start + chunk.len(), len, "the end");
        }
        // Whether the offset is a boundary can be read off the chunk.
        let within = byte - start;
        assert_eq!(
            chunk.is_char_boundary(within),
            text.is_char_boundary(byte),
            "byte {byte}"
        );
        assert_eq!(&text[start..start + chunk.len()], chunk);
    }

    // Past the end is an error.
    assert_eq!(
        buffer.chunk_at(len + 1),
        Err(TextError::ByteOutOfBounds {
            offset: len + 1,
            len
        })
    );
    chunks.len()
}

/// `buffer` and `snapshot` give the same chunk, the same borrowed bytes,
/// for every offset.
fn same_chunks(buffer: &impl TextBuffer, snapshot: &impl TextBuffer) {
    for byte in 0..=buffer.len_bytes() {
        let (a, a_start) = buffer.chunk_at(byte).expect("in bounds");
        let (b, b_start) = snapshot.chunk_at(byte).expect("in bounds");
        assert_eq!((a, a_start), (b, b_start), "byte {byte}");
        // The snapshot shares the buffer's storage: nothing was copied.
        assert_eq!(a.as_ptr(), b.as_ptr(), "byte {byte}");
    }
}

#[test]
fn an_empty_text_is_one_empty_chunk() {
    let buffer = Buffer::new();
    assert_eq!(buffer.chunk_at(0), Ok(("", 0)));
    assert_eq!(
        buffer.chunk_at(1),
        Err(TextError::ByteOutOfBounds { offset: 1, len: 0 })
    );
    assert_eq!(check(&buffer, ""), 0);
}

#[test]
fn a_short_text_is_one_chunk() {
    for text in ["a", "é", "😀", "\r\n", "\r", "a\r\nb\rc\nd", "中文 é😀"] {
        let buffer = Buffer::from(text);
        assert_eq!(check(&buffer, text), 1, "{text:?}");
        // The end of a non-empty text: the last chunk, ending there.
        assert_eq!(buffer.chunk_at(text.len()), Ok((text, 0)));
    }
}

#[test]
fn several_kb_cover_the_text_in_many_chunks() {
    let text = several_kb();
    assert!(text.len() > 8 * 1024, "{}", text.len());
    let buffer = Buffer::from(text.as_str());
    let count = check(&buffer, &text);
    assert!(count >= 4, "{count} chunks");
}

#[test]
fn buffer_and_snapshot_give_the_same_chunks() {
    let text = several_kb();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    let snapshot = set.snapshot(id).expect("in the set");
    check(&snapshot, &text);
    same_chunks(set.get(id).expect("in the set"), &snapshot);
}

#[test]
fn a_snapshot_keeps_its_chunks_after_the_buffer_changes() {
    let text = several_kb();
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text.as_str()));
    let snapshot = set.snapshot(id).expect("in the set");
    let before = walk(&snapshot);

    // Edits that touch several chunks, then an undo of one of them.
    for (at, insert) in [(0, "head "), (3000, "mid é"), (text.len() / 2, "😀\r\n")] {
        let mut transaction = Transaction::new();
        transaction.push(Edit::insert(id, at, insert));
        set.apply(transaction).expect("valid edit");
    }
    let mut transaction = Transaction::new();
    transaction.push(Edit::delete(id, 100..2100));
    set.apply(transaction).expect("valid edit");
    set.undo(id).expect("nothing blocks the undo");

    assert_eq!(walk(&snapshot), before);
    check(&snapshot, &text);
    let buffer = set.get(id).expect("in the set");
    let now = buffer.to_string();
    assert_ne!(now, text);
    check(buffer, &now);
    let later = set.snapshot(id).expect("in the set");
    same_chunks(buffer, &later);
}

/// Pieces of random text: words, blanks, every line break, every width.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-z_]{1,40}",
        2 => "[ \t]{1,4}",
        1 => Just("\n".to_owned()),
        1 => Just("\r".to_owned()),
        1 => Just("\r\n".to_owned()),
        1 => Just("é".to_owned()),
        1 => Just("中".to_owned()),
        1 => Just("😀".to_owned()),
        1 => any::<char>().prop_map(String::from),
    ]
}

/// An edit as fractions of the length, so it is valid for any text.
#[derive(Clone, Debug)]
struct RandomEdit {
    at: f64,
    delete: f64,
    insert: String,
}

fn edit() -> impl Strategy<Value = RandomEdit> {
    (
        0.0..1.0f64,
        0.0..0.2f64,
        prop::collection::vec(piece(), 0..20),
    )
        .prop_map(|(at, delete, pieces)| RandomEdit {
            at,
            delete,
            insert: pieces.concat(),
        })
}

/// The char boundary of `text` at or below `byte`.
fn floor(text: &str, mut byte: usize) -> usize {
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Random texts of up to several KB, then random edits through a set:
    /// the documented properties hold at every offset after each edit, for
    /// the buffer and for a snapshot taken then, and every earlier snapshot
    /// keeps its own text and chunks.
    #[test]
    fn chunks_hold_after_random_edits(
        pieces in prop::collection::vec(piece(), 0..400),
        edits in prop::collection::vec(edit(), 0..6),
    ) {
        let mut text = pieces.concat();
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text.as_str()));
        let mut kept = Vec::new();
        for edit in edits {
            let start = floor(&text, (edit.at * text.len() as f64) as usize);
            let end = floor(&text, (start + (edit.delete * text.len() as f64) as usize).min(text.len()));
            let mut transaction = Transaction::new();
            transaction.push(Edit::replace(id, start..end, edit.insert.as_str()));
            set.apply(transaction).expect("valid edit");
            text.replace_range(start..end, &edit.insert);

            let buffer = set.get(id).expect("in the set");
            check(buffer, &text);
            let snapshot = set.snapshot(id).expect("in the set");
            same_chunks(buffer, &snapshot);
            kept.push((snapshot, text.clone(), walk(buffer)));
        }
        for (snapshot, old, chunks) in &kept {
            check(snapshot, old);
            prop_assert_eq!(&walk(snapshot), chunks);
        }
    }
}
