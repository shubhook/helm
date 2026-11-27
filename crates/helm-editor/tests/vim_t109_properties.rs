//! T-109 acceptance tests: properties of text objects, registers, `.` and
//! counted inserts over random text (ED-5, ED-1). proptest, 256 cases per
//! property.

mod vim_harness;

use helm_editor::Selection;
use proptest::prelude::*;
use vim_harness::Vim;

/// Random text of words, blanks, punctuation, brackets, quotes, each line
/// break, each char width and any char.
fn text() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        4 => "[a-z]{1,6}",
        2 => Just(" ".to_owned()),
        1 => Just("\t".to_owned()),
        1 => "[.,;!?-]{1,2}",
        1 => prop::sample::select(vec!["(", ")", "[", "]", "{", "}", "<", ">"]).prop_map(str::to_owned),
        1 => prop::sample::select(vec!["\"", "'", "`", "\\\""]).prop_map(str::to_owned),
        2 => prop::sample::select(vec!["\n", "\r\n", "\r", "\n\n"]).prop_map(str::to_owned),
        1 => prop::sample::select(vec!["é", "中", "😀", "_"]).prop_map(str::to_owned),
        1 => any::<char>().prop_map(|c| c.to_string()),
    ];
    prop::collection::vec(piece, 0..40).prop_map(|pieces| pieces.concat())
}

/// A char boundary of `text` near `fraction` of its length.
fn boundary(text: &str, fraction: f64) -> usize {
    let mut at = (text.len() as f64 * fraction) as usize;
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// A Vim editor on `text` with cursors near `fractions`. Each property
/// types `hl` first, which settles every cursor onto a normal-mode
/// position and never edits.
fn vim_at(text: &str, fractions: &[f64]) -> Vim {
    let mut v = Vim::new("|");
    let (set, editor) = vim_harness::editor_on(
        text,
        fractions
            .iter()
            .map(|&f| Selection::cursor(boundary(text, f)))
            .collect(),
    );
    v.set = set;
    v.editor = editor;
    v
}

/// The changes `.` is checked on.
const CHANGES: [&str; 12] = [
    "x",
    "2x",
    "dw",
    "d2e",
    "dd",
    "D",
    "cwab<esc>",
    "ciwQ<esc>",
    "A!<esc>",
    "ox<esc>",
    "3ix<esc>",
    "di(",
];

/// The objects checked under `d`.
const OBJECTS: [&str; 12] = [
    "iw", "aw", "iW", "aW", "i(", "a)", "i[", "a{", "i\"", "a'", "ip", "ap",
];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// `.` after a change and a move does what typing the change again
    /// does, to the text and the cursors.
    #[test]
    #[ignore = "T-109: not implemented yet"]
    fn dot_does_what_typing_the_change_again_does(
        text in text(),
        at in 0.0..1.0f64,
        change in prop::sample::select(CHANGES.to_vec()),
        moves in prop::sample::select(vec!["j", "k", "w", "b", "0", "$", "l", ""]),
    ) {
        let mut dot = vim_at(&text, &[at]);
        dot.keys(&format!("hl{change}{moves}."));
        let mut typed = vim_at(&text, &[at]);
        typed.keys(&format!("hl{change}{moves}{change}"));
        prop_assert_eq!(dot.show(), typed.show());
        prop_assert_eq!(dot.mode(), "normal");
    }

    /// Yanking an object's visual selection and putting it back over the
    /// same selection leaves the text as it was. So does `yy` then `Vp`.
    /// (The visual selection, not `y` with the object: a visual end is
    /// never the line break of a line with text, so the two can differ by
    /// that break. `vim.rs` says when.)
    #[test]
    #[ignore = "T-109: not implemented yet"]
    fn yank_then_put_over_the_same_text_changes_nothing(
        text in text(),
        at in 0.0..1.0f64,
        object in prop::sample::select(vec!["iw", "iW", "i(", "i\"", "ip"]),
    ) {
        let mut v = vim_at(&text, &[at]);
        v.keys("hl");
        let before = v.text();
        v.keys(&format!("v{object}yv{object}p"));
        prop_assert_eq!(v.text(), before.clone());
        let mut v = vim_at(&text, &[at]);
        v.keys("hlyyVp");
        prop_assert_eq!(v.text(), before);
    }

    /// `{n}i` with typed text inserts it `n` times, and one `u` restores
    /// the text.
    #[test]
    #[ignore = "T-109: not implemented yet"]
    fn a_counted_insert_types_n_copies_in_one_undo_step(
        text in text(),
        at in 0.0..1.0f64,
        n in 1usize..6,
        typed in "[a-z ]{1,4}",
    ) {
        let mut v = vim_at(&text, &[at]);
        v.keys("hl");
        let before = v.text();
        let p = v.editor.selections().primary().head;
        v.keys(&format!("{n}i{typed}<esc>"));
        let mut expected = before.clone();
        expected.insert_str(p, &typed.repeat(n));
        prop_assert_eq!(v.text(), expected);
        v.keys("u");
        prop_assert_eq!(v.text(), before);
    }

    /// Any text through the clipboard comes back byte for byte (ED-1).
    #[test]
    #[ignore = "T-109: not implemented yet"]
    fn the_clipboard_keeps_every_byte(text in text()) {
        let mut v = Vim::new("|");
        v.session.set_clipboard(Some(text.clone()));
        v.keys("\"+P");
        prop_assert_eq!(v.text(), text);
    }

    /// Objects under `d` at up to two cursors leave every cursor on a
    /// normal-mode position, and as many `u` as edits restore the text.
    #[test]
    #[ignore = "T-109: not implemented yet"]
    fn objects_under_d_undo_and_keep_cursors_on_chars(
        text in text(),
        at in prop::collection::vec(0.0..1.0f64, 1..3),
        objects in prop::collection::vec(prop::sample::select(OBJECTS.to_vec()), 1..4),
    ) {
        let mut v = vim_at(&text, &at);
        v.keys("hl");
        let before = v.text();
        let mut edits = 0;
        for object in &objects {
            let was = v.text();
            v.keys(&format!("d{object}"));
            if v.text() != was {
                edits += 1;
            }
            let text = v.text();
            for s in v.editor.selections().as_slice() {
                prop_assert!(s.is_empty());
                prop_assert!(on_a_char(&text, s.head), "{:?} at {}", text, s.head);
            }
        }
        for _ in 0..edits {
            v.keys("u");
        }
        prop_assert_eq!(v.text(), before);
    }
}

/// Whether `at` is a normal-mode position in `text`, as T-108's `vim.rs`
/// defines it: a char boundary, not inside a CRLF, and not at the end of
/// the text of a line that has text.
fn on_a_char(text: &str, at: usize) -> bool {
    if !text.is_char_boundary(at) {
        return false;
    }
    let bytes = text.as_bytes();
    if at > 0 && at < bytes.len() && bytes[at - 1] == b'\r' && bytes[at] == b'\n' {
        return false;
    }
    let at_break = at == bytes.len() || bytes[at] == b'\n' || bytes[at] == b'\r';
    let line_start = at == 0 || bytes[at - 1] == b'\n' || bytes[at - 1] == b'\r';
    !at_break || line_start
}
