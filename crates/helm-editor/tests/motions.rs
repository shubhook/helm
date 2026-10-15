//! T-107 acceptance tests: moving, extending and deleting by a `Motion`,
//! goal columns, `select_all` and `collapse_selections`.
//!
//! Texts are written with markers: `|` is a head, and `^` is the anchor of
//! the selection whose head is the marker right next to it. A `|` with no
//! `^` beside it is a cursor. The first selection in the text is the
//! primary unless a test says otherwise.

use helm_editor::{Editor, EditorError, Motion, Selection, Selections};
use helm_text::{Buffer, BufferId, BufferSet, FileFormat, LineEndings, TextBuffer};

/// The text with its markers removed, and the selections they mark, in
/// the order they appear.
fn parse(marked: &str) -> (String, Vec<Selection>) {
    let mut text = String::new();
    let mut markers: Vec<(char, usize)> = Vec::new();
    for c in marked.chars() {
        match c {
            '|' | '^' => markers.push((c, text.len())),
            _ => text.push(c),
        }
    }
    // Pair each `^` with the `|` right after it, or else right before it.
    let mut paired = vec![false; markers.len()];
    let mut selections: Vec<(usize, Selection)> = Vec::new();
    for j in 0..markers.len() {
        if markers[j].0 != '^' {
            continue;
        }
        let head = if j + 1 < markers.len() && markers[j + 1].0 == '|' && !paired[j + 1] {
            j + 1
        } else if j > 0 && markers[j - 1].0 == '|' && !paired[j - 1] {
            j - 1
        } else {
            panic!(
                "`^` at {} has no `|` next to it in {marked:?}",
                markers[j].1
            );
        };
        paired[j] = true;
        paired[head] = true;
        selections.push((j.min(head), Selection::new(markers[j].1, markers[head].1)));
    }
    for (j, &(c, at)) in markers.iter().enumerate() {
        if c == '|' && !paired[j] {
            selections.push((j, Selection::cursor(at)));
        }
    }
    selections.sort_by_key(|&(j, _)| j);
    assert!(!selections.is_empty(), "no selection in {marked:?}");
    (text, selections.into_iter().map(|(_, s)| s).collect())
}

/// `text` with the markers for `selections`.
fn show(text: &str, selections: &Selections) -> String {
    let mut markers: Vec<(usize, usize, char)> = Vec::new();
    for (i, s) in selections.as_slice().iter().enumerate() {
        if s.is_empty() {
            markers.push((s.head, i, '|'));
        } else if s.is_reversed() {
            markers.push((s.head, i, '|'));
            markers.push((s.anchor, i, '^'));
        } else {
            markers.push((s.anchor, i, '^'));
            markers.push((s.head, i, '|'));
        }
    }
    markers.sort();
    let mut out = String::new();
    let mut at = 0;
    for (offset, _, c) in markers {
        out.push_str(&text[at..offset]);
        out.push(c);
        at = offset;
    }
    out.push_str(&text[at..]);
    out
}

/// One editor on one buffer.
struct Session {
    set: BufferSet,
    editor: Editor,
}

impl Session {
    /// An editor on the marked text, the first selection primary.
    fn new(marked: &str) -> Self {
        Self::with_primary(marked, 0)
    }

    /// An editor on the marked text, with selection `primary` (in text
    /// order) as the primary.
    fn with_primary(marked: &str, primary: usize) -> Self {
        let (text, selections) = parse(marked);
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text.as_str()));
        let format = FileFormat {
            bom: false,
            line_endings: LineEndings::count(&text),
        };
        let mut editor = Editor::new(id, format);
        editor.set_selections(Selections::new(selections, primary).expect("valid selections"));
        Self { set, editor }
    }

    fn text(&self) -> String {
        self.set
            .get(self.editor.buffer())
            .expect("buffer is in the set")
            .to_string()
    }

    /// The text with the editor's selections marked.
    fn show(&self) -> String {
        show(&self.text(), self.editor.selections())
    }

    fn move_by(&mut self, motion: Motion) -> bool {
        self.editor
            .move_cursors(&self.set, motion)
            .expect("buffer is in the set")
    }

    fn extend_by(&mut self, motion: Motion) -> bool {
        self.editor
            .extend_selections(&self.set, motion)
            .expect("buffer is in the set")
    }

    fn delete_to(&mut self, motion: Motion) -> bool {
        self.editor
            .delete_to(&mut self.set, motion)
            .expect("buffer is in the set")
            .is_some()
    }
}

/// The marked text after moving every selection in `marked` by `motion`.
fn moved(motion: Motion, marked: &str) -> String {
    let mut s = Session::new(marked);
    s.move_by(motion);
    s.show()
}

/// The marked text after extending every selection in `marked` by
/// `motion`.
fn extended(motion: Motion, marked: &str) -> String {
    let mut s = Session::new(marked);
    s.extend_by(motion);
    s.show()
}

/// The marked text after `delete_to(motion)` on `marked`.
fn deleted(motion: Motion, marked: &str) -> String {
    let mut s = Session::new(marked);
    s.delete_to(motion);
    s.show()
}

/// Checks each `(before, after)` pair for one function and motion.
fn table(f: fn(Motion, &str) -> String, motion: Motion, cases: &[(&str, &str)]) {
    for &(before, after) in cases {
        assert_eq!(
            f(motion, before),
            after,
            "{motion:?} from {before:?} should give {after:?}"
        );
    }
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn the_markers_round_trip() {
    // Checks the helpers, and calls into the implementation once so the
    // test runs against it.
    for marked in ["ab|c", "a^bc|d", "a|bc^d", "|a ^b|", "^a|^b|", "a\r\n|b"] {
        let (text, selections) = parse(marked);
        let selections = Selections::new(selections, 0).expect("valid");
        assert_eq!(show(&text, &selections), marked);
    }
    assert_eq!(moved(Motion::TextStart, "ab|c"), "|abc");
}

// Left and Right.

#[test]
#[ignore = "T-107: not implemented yet"]
fn left_and_right_move_one_character() {
    table(
        moved,
        Motion::Left,
        &[
            ("ab|c", "a|bc"),
            ("|abc", "|abc"),
            ("a\r\n|b", "a|\r\nb"),
            ("a\n|b", "a|\nb"),
            ("a\r|b", "a|\rb"),
            ("é|中😀", "|é中😀"),
            ("é中😀|", "é中|😀"),
        ],
    );
    table(
        moved,
        Motion::Right,
        &[
            ("ab|c", "abc|"),
            ("abc|", "abc|"),
            ("a|\r\nb", "a\r\n|b"),
            ("a|\nb", "a\n|b"),
            ("a|\rb", "a\r|b"),
            ("|é中😀", "é|中😀"),
            ("é中|😀", "é中😀|"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn a_head_inside_a_crlf_moves_out_of_it() {
    // A stale cursor between the CR and the LF.
    assert_eq!(moved(Motion::Left, "a\r|\nb"), "a|\r\nb");
    assert_eq!(moved(Motion::Right, "a\r|\nb"), "a\r\n|b");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn moving_left_or_right_collapses_a_selection_to_its_side() {
    table(
        moved,
        Motion::Left,
        &[("a^bc|d", "a|bcd"), ("a|bc^d", "a|bcd")],
    );
    table(
        moved,
        Motion::Right,
        &[("a^bc|d", "abc|d"), ("a|bc^d", "abc|d")],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn other_motions_move_a_selection_from_its_head() {
    assert_eq!(moved(Motion::WordRight, "^ab| cd"), "ab cd|");
    assert_eq!(moved(Motion::LineEnd, "a|b^c\nd"), "abc|\nd");
    assert_eq!(moved(Motion::Down, "a^b|\ncd"), "ab\ncd|");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn extending_moves_only_the_head() {
    table(
        extended,
        Motion::Left,
        &[("ab|c", "a|b^c"), ("a^bc|d", "a^b|cd"), ("|abc", "|abc")],
    );
    table(
        extended,
        Motion::Right,
        &[
            ("ab|c", "ab^c|"),
            ("a|bc^d", "ab|c^d"),
            ("a^b|\r\nc", "a^b\r\n|c"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn a_head_that_reaches_its_anchor_leaves_a_cursor() {
    assert_eq!(extended(Motion::Left, "a^b|c"), "a|bc");
    assert_eq!(extended(Motion::Right, "a|b^c"), "ab|c");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn moving_reports_whether_anything_changed() {
    let mut s = Session::new("|abc");
    assert!(!s.move_by(Motion::Left));
    assert!(!s.move_by(Motion::TextStart));
    assert!(s.move_by(Motion::Right));
    assert!(s.extend_by(Motion::LineStart));
    assert_eq!(s.show(), "|a^bc");
    assert!(!s.extend_by(Motion::TextStart));
    let mut s = Session::new("a^b|c");
    assert!(s.extend_by(Motion::Left));
    assert_eq!(s.show(), "a|bc");
}

// Up and Down.

#[test]
#[ignore = "T-107: not implemented yet"]
fn up_and_down_keep_the_column() {
    table(
        moved,
        Motion::Up,
        &[
            ("abc\nd|ef\nghi", "a|bc\ndef\nghi"),
            ("abc\r\nd|ef", "a|bc\r\ndef"),
            ("a\rb|c", "a|\rbc"),
        ],
    );
    table(
        moved,
        Motion::Down,
        &[
            ("abc\nd|ef\nghi", "abc\ndef\ng|hi"),
            ("a|bc\r\ndef", "abc\r\nd|ef"),
            ("a|bc\n", "abc\n|"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn up_on_the_first_line_and_down_on_the_last_go_to_the_ends() {
    assert_eq!(moved(Motion::Up, "ab|c\ndef"), "|abc\ndef");
    assert_eq!(moved(Motion::Down, "abc\nd|ef"), "abc\ndef|");
    assert_eq!(moved(Motion::Up, "a|bc"), "|abc");
    assert_eq!(moved(Motion::Down, "a|bc"), "abc|");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn the_column_is_capped_before_the_line_break() {
    assert_eq!(moved(Motion::Down, "abcd|\nab\nx"), "abcd\nab|\nx");
    assert_eq!(moved(Motion::Down, "abcd|\r\nab\r\nx"), "abcd\r\nab|\r\nx");
    assert_eq!(moved(Motion::Up, "\nabc|"), "|\nabc");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn columns_count_chars_not_bytes() {
    assert_eq!(moved(Motion::Down, "中文字|\nabcdef"), "中文字\nabc|def");
    assert_eq!(moved(Motion::Up, "é😀ab\nab|c"), "é😀|ab\nabc");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn the_goal_column_survives_a_short_line() {
    let mut s = Session::new("abcde|f\nab\nabcdef");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab|\nabcdef");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab\nabcde|f");
    s.move_by(Motion::Up);
    assert_eq!(s.show(), "abcdef\nab|\nabcdef");
    s.move_by(Motion::Up);
    assert_eq!(s.show(), "abcde|f\nab\nabcdef");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn the_goal_column_survives_the_first_and_last_lines() {
    let mut s = Session::new("ab|c\nabc");
    s.move_by(Motion::Up);
    assert_eq!(s.show(), "|abc\nabc");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abc\nab|c");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abc\nabc|");
    s.move_by(Motion::Up);
    assert_eq!(s.show(), "ab|c\nabc");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn extending_up_and_down_shares_the_goal_with_moving() {
    let mut s = Session::new("abcde|f\nab\nabcdef");
    s.extend_by(Motion::Down);
    assert_eq!(s.show(), "abcde^f\nab|\nabcdef");
    s.extend_by(Motion::Down);
    assert_eq!(s.show(), "abcde^f\nab\nabcde|f");
    s.move_by(Motion::Up);
    assert_eq!(s.show(), "abcdef\nab|\nabcdef");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab\nabcde|f");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn any_other_motion_drops_the_goal() {
    let mut s = Session::new("abcde|f\nab\nabcdef");
    s.move_by(Motion::Down);
    s.move_by(Motion::Left);
    assert_eq!(s.show(), "abcdef\na|b\nabcdef");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab\na|bcdef");

    let mut s = Session::new("abcde|f\nab\nabcdef");
    s.move_by(Motion::Down);
    s.extend_by(Motion::LineEnd);
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab\nab|cdef");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn editing_and_set_selections_drop_the_goal() {
    let mut s = Session::new("abcde|f\nab\nabcdef");
    s.move_by(Motion::Down);
    s.editor
        .insert(&mut s.set, "x")
        .expect("buffer is in the set");
    assert_eq!(s.show(), "abcdef\nabx|\nabcdef");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nabx\nabc|def");

    let mut s = Session::new("abcde|f\nab\nabcdef");
    s.move_by(Motion::Down);
    s.editor
        .set_selections(Selections::single(Selection::cursor(8)));
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab\na|bcdef");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn undo_drops_the_goal() {
    let mut s = Session::new("abcde|f\nab\nabcdef");
    s.editor
        .insert(&mut s.set, "x")
        .expect("buffer is in the set");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdexf\nab|\nabcdef");
    s.editor.undo(&mut s.set).expect("buffer is in the set");
    assert_eq!(s.show(), "abcde|f\nab\nabcdef");
    s.move_by(Motion::Down);
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab\nabcde|f");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn each_cursor_keeps_its_own_goal() {
    let mut s = Session::new("a|bcde|f\nab\nabcdef");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\na|b|\nabcdef");
    s.move_by(Motion::Down);
    assert_eq!(s.show(), "abcdef\nab\na|bcde|f");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn cursors_that_meet_merge_and_keep_the_primary() {
    let mut s = Session::with_primary("a|b|c\nx", 1);
    assert!(s.move_by(Motion::Up));
    assert_eq!(s.show(), "|abc\nx");
    assert_eq!(s.editor.selections().as_slice().len(), 1);

    let mut s = Session::with_primary("a|b\nc|d\ne|f", 2);
    s.move_by(Motion::TextEnd);
    assert_eq!(s.show(), "ab\ncd\nef|");
    let mut s = Session::with_primary("a|b\nc|d\ne|f", 1);
    s.move_by(Motion::Right);
    assert_eq!(s.show(), "ab|\ncd|\nef|");
    assert_eq!(s.editor.selections().primary_index(), 1);
}

// Words.

#[test]
#[ignore = "T-107: not implemented yet"]
fn word_right_stops_at_the_end_of_each_run() {
    table(
        moved,
        Motion::WordRight,
        &[
            ("|foo.bar  baz\nqux", "foo|.bar  baz\nqux"),
            ("foo|.bar  baz\nqux", "foo.|bar  baz\nqux"),
            ("foo.|bar  baz\nqux", "foo.bar|  baz\nqux"),
            ("foo.bar|  baz\nqux", "foo.bar  baz|\nqux"),
            ("foo.bar  baz|\nqux", "foo.bar  baz\n|qux"),
            ("foo.bar  baz\n|qux", "foo.bar  baz\nqux|"),
            ("qux|", "qux|"),
            ("a|  \t  \nb", "a  \t  |\nb"),
            ("a|  ", "a  |"),
            ("a|\r\nb", "a\r\n|b"),
            ("a|\rb", "a\r|b"),
            ("|__init__()", "__init__|()"),
            ("|café 中文", "café| 中文"),
            ("café| 中文", "café 中文|"),
            ("|😀😀a", "😀😀|a"),
            ("|(a)", "(|a)"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn word_left_stops_at_the_start_of_each_run() {
    table(
        moved,
        Motion::WordLeft,
        &[
            ("foo\nfoo.bar  baz|", "foo\nfoo.bar  |baz"),
            ("foo\nfoo.bar  |baz", "foo\nfoo.|bar  baz"),
            ("foo\nfoo.|bar  baz", "foo\nfoo|.bar  baz"),
            ("foo\nfoo|.bar  baz", "foo\n|foo.bar  baz"),
            ("foo\n|foo.bar  baz", "foo|\nfoo.bar  baz"),
            ("foo|\nfoo.bar  baz", "|foo\nfoo.bar  baz"),
            ("|foo", "|foo"),
            ("a\n  \t |b", "a\n|  \t b"),
            ("  |b", "|  b"),
            ("a\r\n|b", "a|\r\nb"),
            ("a\r|b", "a|\rb"),
            ("x.__init__|", "x.|__init__"),
            ("café 中文|", "café |中文"),
            ("a😀😀|", "a|😀😀"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn extending_by_words_selects_them() {
    assert_eq!(extended(Motion::WordRight, "|foo bar"), "^foo| bar");
    assert_eq!(extended(Motion::WordLeft, "foo bar|"), "foo |bar^");
    let mut s = Session::new("|foo bar");
    s.extend_by(Motion::WordRight);
    s.extend_by(Motion::WordRight);
    assert_eq!(s.show(), "^foo bar|");
}

// Lines and the whole text.

#[test]
#[ignore = "T-107: not implemented yet"]
fn line_start_goes_to_the_first_non_blank_then_the_line_start() {
    let mut s = Session::new("x\n  foo|bar");
    s.move_by(Motion::LineStart);
    assert_eq!(s.show(), "x\n  |foobar");
    s.move_by(Motion::LineStart);
    assert_eq!(s.show(), "x\n|  foobar");
    s.move_by(Motion::LineStart);
    assert_eq!(s.show(), "x\n  |foobar");
    table(
        moved,
        Motion::LineStart,
        &[
            ("  | foo", "   |foo"),
            ("a\r\n\tb|c", "a\r\n\t|bc"),
            ("foo|", "|foo"),
            ("   |", "|   "),
            ("|   ", "   |"),
            ("a\n|\nb", "a\n|\nb"),
            ("a\r\r\n|x", "a\r\r\n|x"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn line_end_goes_before_the_line_break() {
    table(
        moved,
        Motion::LineEnd,
        &[
            ("a|bc\r\nd", "abc|\r\nd"),
            ("a|bc\nd", "abc|\nd"),
            ("a|b\rc", "ab|\rc"),
            ("a\nb|c", "a\nbc|"),
            ("abc|\nd", "abc|\nd"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn text_start_and_end() {
    assert_eq!(moved(Motion::TextStart, "ab\nc|d"), "|ab\ncd");
    assert_eq!(moved(Motion::TextEnd, "a|b\nc|d"), "ab\ncd|");
    assert_eq!(extended(Motion::TextStart, "ab\nc|d"), "|ab\nc^d");
    assert_eq!(extended(Motion::TextEnd, "a|b\r\ncd"), "a^b\r\ncd|");
    assert_eq!(moved(Motion::TextEnd, "|"), "|");
}

// Deleting by a motion.

#[test]
#[ignore = "T-107: not implemented yet"]
fn delete_to_deletes_from_each_cursor_to_where_the_motion_goes() {
    table(
        deleted,
        Motion::WordLeft,
        &[
            ("foo bar|", "foo |"),
            ("foo |bar", "|bar"),
            ("a\r\n|b", "a|b"),
            ("|abc", "|abc"),
        ],
    );
    table(
        deleted,
        Motion::WordRight,
        &[
            ("|foo bar", "| bar"),
            ("foo| bar", "foo|"),
            ("a|\nb", "a|b"),
        ],
    );
    table(
        deleted,
        Motion::LineStart,
        &[
            ("  foo|bar", "  |bar"),
            ("  |bar", "|bar"),
            ("a\n|b", "a\n|b"),
        ],
    );
    table(
        deleted,
        Motion::LineEnd,
        &[
            ("fo|o\nbar", "fo|\nbar"),
            ("foo|\nbar", "foo|\nbar"),
            ("a|b\r\nc", "a|\r\nc"),
        ],
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn delete_to_deletes_a_selection_instead() {
    assert_eq!(deleted(Motion::WordLeft, "a^bc|d"), "a|d");
    assert_eq!(deleted(Motion::LineEnd, "a|bc^d"), "a|d");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn delete_to_with_nothing_to_delete_applies_nothing() {
    let mut s = Session::new("|abc");
    assert!(!s.delete_to(Motion::WordLeft));
    assert!(!s.delete_to(Motion::LineStart));
    assert_eq!(s.show(), "|abc");
    assert_eq!(s.editor.undo(&mut s.set), Ok(None));
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn delete_to_at_every_cursor_is_one_undo_step() {
    let mut s = Session::new("foo bar|\nbaz qux|");
    assert!(s.delete_to(Motion::WordLeft));
    assert_eq!(s.show(), "foo |\nbaz |");
    s.editor.undo(&mut s.set).expect("buffer is in the set");
    assert_eq!(s.show(), "foo bar|\nbaz qux|");
    s.editor.redo(&mut s.set).expect("buffer is in the set");
    assert_eq!(s.show(), "foo |\nbaz |");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn delete_to_joins_overlapping_ranges() {
    assert_eq!(deleted(Motion::WordLeft, "ab|c|"), "|");
    assert_eq!(deleted(Motion::LineStart, "x\nab|c|d"), "x\n|d");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn delete_to_keeps_cursors_that_delete_nothing() {
    assert_eq!(deleted(Motion::WordLeft, "|ab cd|"), "|ab |");
    assert_eq!(deleted(Motion::LineEnd, "ab|\ncd|\nef|gh"), "ab|\ncd|\nef|");
}

// Selecting all and collapsing.

#[test]
#[ignore = "T-107: not implemented yet"]
fn select_all_selects_the_whole_text() {
    let mut s = Session::new("a|b\nc|d");
    assert!(s.editor.select_all(&s.set).expect("buffer is in the set"));
    assert_eq!(s.show(), "^ab\ncd|");
    assert_eq!(s.editor.selections().as_slice().len(), 1);
    assert!(!s.editor.select_all(&s.set).expect("buffer is in the set"));

    let mut s = Session::new("|");
    assert!(!s.editor.select_all(&s.set).expect("buffer is in the set"));
    assert_eq!(s.show(), "|");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn collapse_keeps_the_primary_then_makes_it_a_cursor() {
    let mut s = Session::with_primary("a|b^c|d^e|", 1);
    assert!(
        s.editor
            .collapse_selections(&s.set)
            .expect("buffer is in the set")
    );
    assert_eq!(s.show(), "ab^c|de");
    assert!(
        s.editor
            .collapse_selections(&s.set)
            .expect("buffer is in the set")
    );
    assert_eq!(s.show(), "abc|de");
    assert!(
        !s.editor
            .collapse_selections(&s.set)
            .expect("buffer is in the set")
    );

    assert_eq!(
        {
            let mut s = Session::new("a|bc^d");
            s.editor
                .collapse_selections(&s.set)
                .expect("buffer is in the set");
            s.show()
        },
        "a|bcd"
    );
}

// Clamping and errors.

#[test]
#[ignore = "T-107: not implemented yet"]
fn stale_selections_are_clamped_first() {
    let mut s = Session::new("|abc");
    s.editor
        .set_selections(Selections::single(Selection::cursor(100)));
    s.move_by(Motion::Left);
    assert_eq!(s.show(), "ab|c");

    // Offset 2 is inside é.
    let mut s = Session::new("|aéb");
    s.editor
        .set_selections(Selections::single(Selection::cursor(2)));
    s.move_by(Motion::Right);
    assert_eq!(s.show(), "aé|b");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn an_unknown_buffer_is_an_error_and_changes_nothing() {
    let set = BufferSet::new();
    let mut other = BufferSet::new();
    let id: BufferId = other.add(Buffer::from("abc"));
    let mut editor = Editor::new(
        id,
        FileFormat {
            bom: false,
            line_endings: LineEndings::count(""),
        },
    );
    editor.set_selections(Selections::single(Selection::new(1, 2)));
    let before = editor.selections().clone();
    let unknown = Err(EditorError::UnknownBuffer { buffer: id });
    assert_eq!(editor.move_cursors(&set, Motion::Left), unknown);
    assert_eq!(editor.extend_selections(&set, Motion::Down), unknown);
    assert_eq!(editor.select_all(&set), unknown);
    assert_eq!(editor.collapse_selections(&set), unknown);
    let mut empty = BufferSet::new();
    assert_eq!(
        editor.delete_to(&mut empty, Motion::WordLeft),
        Err(EditorError::UnknownBuffer { buffer: id })
    );
    assert_eq!(editor.selections(), &before);
    assert_eq!(other.get(id).expect("in the set").len_bytes(), 3);
}
