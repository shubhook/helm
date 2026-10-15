//! T-107 acceptance tests: property tests for motions on random text with
//! every line break and character width. Every motion lands on a char
//! boundary outside any CRLF, Left and Right undo each other, word motions
//! always make progress, many cursors move as each would alone, and
//! `delete_to` matches a plain `String` oracle.

use helm_editor::{Editor, Motion, Selection, Selections};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};
use proptest::prelude::*;
use proptest::sample::Index;

/// Every motion.
const MOTIONS: [Motion; 10] = [
    Motion::Left,
    Motion::Right,
    Motion::Up,
    Motion::Down,
    Motion::WordLeft,
    Motion::WordRight,
    Motion::LineStart,
    Motion::LineEnd,
    Motion::TextStart,
    Motion::TextEnd,
];

/// One piece of random text: words, blanks, punctuation, each line break
/// and each character width.
fn piece() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-c_]{1,4}",
        2 => "[ \t]{1,3}",
        2 => "[.(){}]{1,2}",
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

fn motion() -> impl Strategy<Value = Motion> {
    prop::sample::select(MOTIONS.to_vec())
}

/// Every char boundary of `text` that isn't between a CR and an LF,
/// including its length.
fn positions(text: &str) -> Vec<usize> {
    text.char_indices()
        .map(|(byte, _)| byte)
        .chain([text.len()])
        .filter(|&at| !inside_crlf(text, at))
        .collect()
}

/// Whether `at` sits between the CR and the LF of a CRLF.
fn inside_crlf(text: &str, at: usize) -> bool {
    at > 0 && text[..at].ends_with('\r') && text[at..].starts_with('\n')
}

fn format_of(text: &str) -> FileFormat {
    FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    }
}

/// An editor on `text` with `selections`.
fn setup(text: &str, selections: Selections) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let mut editor = Editor::new(id, format_of(text));
    editor.set_selections(selections);
    (set, editor)
}

/// Where `motion` takes a single cursor at `at`, by moving it.
fn target(text: &str, at: usize, motion: Motion) -> usize {
    let (set, mut editor) = setup(text, Selections::single(Selection::cursor(at)));
    editor
        .move_cursors(&set, motion)
        .expect("buffer is in the set");
    let selections = editor.selections().as_slice();
    assert_eq!(selections.len(), 1);
    assert!(selections[0].is_empty());
    selections[0].head
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// From any position, every motion lands on a char boundary that isn't
    /// inside a CRLF, within the text.
    #[test]
    #[ignore = "T-107: not implemented yet"]
    fn every_motion_lands_on_a_valid_position(
        text in text(24),
        at in any::<Index>(),
        motion in motion(),
    ) {
        let at = *at.get(&positions(&text));
        let to = target(&text, at, motion);
        prop_assert!(to <= text.len());
        prop_assert!(text.is_char_boundary(to));
        prop_assert!(!inside_crlf(&text, to), "{motion:?} from {at} in {text:?} gave {to}");
    }

    /// Left then Right, and Right then Left, come back to where they
    /// started, away from the ends of the text.
    #[test]
    #[ignore = "T-107: not implemented yet"]
    fn left_and_right_undo_each_other(text in text(24), at in any::<Index>()) {
        let at = *at.get(&positions(&text));
        if at < text.len() {
            let right = target(&text, at, Motion::Right);
            prop_assert!(right > at);
            prop_assert_eq!(target(&text, right, Motion::Left), at);
        }
        if at > 0 {
            let left = target(&text, at, Motion::Left);
            prop_assert!(left < at);
            prop_assert_eq!(target(&text, left, Motion::Right), at);
        }
    }

    /// Word motions always move, until the end they head for, so pressing
    /// one repeatedly crosses the whole text.
    #[test]
    #[ignore = "T-107: not implemented yet"]
    fn word_motions_always_make_progress(text in text(24)) {
        let mut at = 0;
        let mut steps = 0;
        while at < text.len() {
            let next = target(&text, at, Motion::WordRight);
            prop_assert!(next > at, "WordRight stuck at {at} in {text:?}");
            at = next;
            steps += 1;
        }
        prop_assert!(steps <= text.len());
        while at > 0 {
            let next = target(&text, at, Motion::WordLeft);
            prop_assert!(next < at, "WordLeft stuck at {at} in {text:?}");
            at = next;
        }
    }

    /// Moving many cursors at once gives what moving each alone gives, put
    /// in normal form, with the primary the one the old primary became.
    #[test]
    #[ignore = "T-107: not implemented yet"]
    fn many_cursors_move_as_each_would_alone(
        text in text(24),
        picks in prop::collection::vec(any::<Index>(), 1..=12),
        primary in any::<Index>(),
        motion in motion(),
    ) {
        let positions = positions(&text);
        let cursors: Vec<Selection> = picks
            .iter()
            .map(|pick| Selection::cursor(*pick.get(&positions)))
            .collect();
        let selections = Selections::new(cursors, primary.index(picks.len()))
            .expect("non-empty, primary in range");
        let (set, mut editor) = setup(&text, selections.clone());
        editor.move_cursors(&set, motion).expect("buffer is in the set");

        let alone: Vec<Selection> = selections
            .as_slice()
            .iter()
            .map(|cursor| Selection::cursor(target(&text, cursor.head, motion)))
            .collect();
        let expected = Selections::new(alone, selections.primary_index())
            .expect("non-empty, primary in range");
        prop_assert_eq!(editor.selections(), &expected);
    }

    /// Extending a cursor puts its head where moving it would, and keeps
    /// its anchor.
    #[test]
    #[ignore = "T-107: not implemented yet"]
    fn extending_keeps_the_anchor(
        text in text(24),
        at in any::<Index>(),
        motion in motion(),
    ) {
        let at = *at.get(&positions(&text));
        let (set, mut editor) = setup(&text, Selections::single(Selection::cursor(at)));
        editor.extend_selections(&set, motion).expect("buffer is in the set");
        let expected = Selection::new(at, target(&text, at, motion));
        prop_assert_eq!(editor.selections().as_slice(), &[expected]);
    }

    /// `delete_to` from one cursor removes exactly the text between the
    /// cursor and where the motion takes it, leaves the cursor at the
    /// start of that range, and one undo restores the text.
    #[test]
    #[ignore = "T-107: not implemented yet"]
    fn delete_to_matches_a_string_oracle(
        text in text(24),
        at in any::<Index>(),
        motion in motion(),
    ) {
        let at = *at.get(&positions(&text));
        let to = target(&text, at, motion);
        let range = at.min(to)..at.max(to);
        let mut expected = text.clone();
        expected.replace_range(range.clone(), "");

        let (mut set, mut editor) = setup(&text, Selections::single(Selection::cursor(at)));
        let id = editor.delete_to(&mut set, motion).expect("buffer is in the set");
        prop_assert_eq!(id.is_some(), !range.is_empty());
        let after = set.get(editor.buffer()).expect("buffer is in the set").to_string();
        prop_assert_eq!(&after, &expected);
        prop_assert_eq!(editor.selections().as_slice(), &[Selection::cursor(range.start)]);

        editor.undo(&mut set).expect("buffer is in the set");
        let undone = set.get(editor.buffer()).expect("buffer is in the set").to_string();
        prop_assert_eq!(undone, text);
    }
}
