//! T-105 acceptance tests: the ED-3 commands that add selections. Add next
//! match, select all matches, and add cursor above and below.

use helm_editor::{Editor, Selection, Selections};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// A cursor at `offset`.
fn c(offset: usize) -> Selection {
    Selection::cursor(offset)
}

/// A selection from `anchor` to `head`.
fn s(anchor: usize, head: usize) -> Selection {
    Selection::new(anchor, head)
}

/// A set with one buffer holding `text`, and an editor on it with the
/// file's format and the given selections.
fn setup(text: &str, selections: &[Selection], primary: usize) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    editor.set_selections(
        Selections::new(selections.to_vec(), primary).expect("non-empty, primary in range"),
    );
    (set, editor)
}

/// The text of the editor's buffer.
fn text(set: &BufferSet, editor: &Editor) -> String {
    set.get(editor.buffer())
        .expect("buffer is in the set")
        .to_string()
}

/// Asserts the editor's selections and the primary's index.
fn assert_sels(editor: &Editor, expected: &[Selection], primary: usize) {
    assert_eq!(editor.selections().as_slice(), expected);
    assert_eq!(editor.selections().primary_index(), primary);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_next_match_selects_the_word_first() {
    let (set, mut editor) = setup("foo bar foo", &[c(1)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3)], 0);

    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3), s(8, 11)], 1);

    // Every occurrence is selected: nothing changes.
    assert_eq!(editor.add_next_match(&set), Ok(false));
    assert_sels(&editor, &[s(0, 3), s(8, 11)], 1);
    assert_eq!(text(&set, &editor), "foo bar foo");
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_next_match_wraps_to_the_start() {
    let (set, mut editor) = setup("foo bar foo", &[s(8, 11)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3), s(8, 11)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_next_match_only_touches_the_primary() {
    // A second cursor elsewhere stays a cursor while the primary becomes
    // its word.
    let (set, mut editor) = setup("foo bar foo", &[c(5), c(9)], 1);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[c(5), s(8, 11)], 1);

    // The next match starts after the primary, wraps, and skips nothing
    // else: 0..3 doesn't overlap the cursor at 5.
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3), c(5), s(8, 11)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_next_match_without_a_word() {
    // Between ',' and ' ': no word on either side.
    let (set, mut editor) = setup("foo, bar", &[c(4)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(false));
    assert_sels(&editor, &[c(4)], 0);

    // After "foo", before ',': the word to the left.
    let (set, mut editor) = setup("foo, bar", &[c(3)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3)], 0);

    // Before a_1: the word to the right. `_` and digits are word
    // characters.
    let (set, mut editor) = setup("x = a_1 + a_1", &[c(4)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(4, 7)], 0);

    // The empty buffer.
    let (set, mut editor) = setup("", &[c(0)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(false));
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_next_match_with_multibyte_words() {
    // café is 0..5 (é is 3..5), 中文 6..12, café 13..18, 😀 19..23.
    let text = "café 中文 café 😀";
    let (set, mut editor) = setup(text, &[c(3)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 5)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 5), s(13, 18)], 1);

    // Between 中 and 文.
    let (set, mut editor) = setup(text, &[c(9)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(6, 12)], 0);

    // 😀 isn't a word character.
    let (set, mut editor) = setup(text, &[c(23)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(false));

    // A cursor inside é is clamped to its start first.
    let (set, mut editor) = setup(text, &[c(4)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 5)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_next_match_from_a_selection() {
    // The selection's text is the search, not a word: "o b" here.
    let (set, mut editor) = setup("foo bar, foo bar", &[s(2, 5)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(2, 5), s(11, 14)], 1);

    // A leftwards primary searches from its end, and the new selection
    // points forward.
    let (set, mut editor) = setup("foo foo", &[s(3, 0)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(3, 0), s(4, 7)], 1);

    // Case-sensitive.
    let (set, mut editor) = setup("Foo foo FOO", &[s(4, 7)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(false));

    // Across a CRLF.
    let (set, mut editor) = setup("a\r\nb a\r\nb", &[s(0, 4)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 4), s(5, 9)], 1);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_next_match_skips_overlapping_occurrences() {
    // "aa" occurs at 0, 1, 2 and 3. From 0..2, the next one that doesn't
    // overlap is 2..4, which only touches it.
    let (set, mut editor) = setup("aaaaa", &[s(0, 2)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(0, 2), s(2, 4)], 1);

    // From 2..4 nothing starts at 4 or later, so it wraps: 0..2, 1..3,
    // 2..4 and 3..5 all overlap a selection.
    assert_eq!(editor.add_next_match(&set), Ok(false));

    // Overlapping occurrences count: from 1..3, 3..5 is next.
    let (set, mut editor) = setup("aaaaa", &[s(1, 3)], 0);
    assert_eq!(editor.add_next_match(&set), Ok(true));
    assert_sels(&editor, &[s(1, 3), s(3, 5)], 1);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn select_all_matches_of_the_word() {
    let (set, mut editor) = setup("foo bar foo baz foo", &[c(9)], 0);
    assert_eq!(editor.select_all_matches(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3), s(8, 11), s(16, 19)], 1);

    // Again: nothing changes.
    assert_eq!(editor.select_all_matches(&set), Ok(false));
    assert_sels(&editor, &[s(0, 3), s(8, 11), s(16, 19)], 1);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn select_all_matches_replaces_other_selections() {
    let (set, mut editor) = setup("x foo y foo", &[s(0, 1), s(5, 2)], 1);
    assert_eq!(editor.select_all_matches(&set), Ok(true));
    assert_sels(&editor, &[s(2, 5), s(8, 11)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn select_all_matches_never_overlap() {
    // From the start: 0..2, then 2..4. The primary is the first one that
    // overlaps the old one, 1..3.
    let (set, mut editor) = setup("aaaaa", &[s(1, 3)], 0);
    assert_eq!(editor.select_all_matches(&set), Ok(true));
    assert_sels(&editor, &[s(0, 2), s(2, 4)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn select_all_matches_without_a_word() {
    let (set, mut editor) = setup("a  b", &[c(2)], 0);
    assert_eq!(editor.select_all_matches(&set), Ok(false));
    assert_sels(&editor, &[c(2)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn select_all_matches_then_type() {
    // Plain matches, not whole words: the foo in foo_bar is selected too.
    let (mut set, mut editor) = setup("foo(foo, foo_bar)", &[c(0)], 0);
    assert_eq!(editor.select_all_matches(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3), s(4, 7), s(9, 12)], 0);

    let id = editor.insert(&mut set, "x").unwrap();
    assert_eq!(text(&set, &editor), "x(x, x_bar)");
    assert_sels(&editor, &[c(1), c(3), c(6)], 0);

    assert_eq!(editor.undo(&mut set), Ok(id));
    assert_eq!(text(&set, &editor), "foo(foo, foo_bar)");
    assert_sels(&editor, &[s(0, 3), s(4, 7), s(9, 12)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn select_all_matches_with_multibyte_text() {
    // 中 0..3, 文 3..6, é 6..8, 😀 8..12, then 中文 again at 12..18. é is a
    // word character, so the word around 3 is 中文é, which occurs once.
    let (set, mut editor) = setup("中文é😀中文", &[c(3)], 0);
    assert_eq!(editor.select_all_matches(&set), Ok(true));
    assert_sels(&editor, &[s(0, 8)], 0);

    let (set, mut editor) = setup("中文 😀 中文", &[s(0, 6)], 0);
    assert_eq!(editor.select_all_matches(&set), Ok(true));
    assert_sels(&editor, &[s(0, 6), s(12, 18)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_cursor_below_builds_a_column() {
    let (set, mut editor) = setup("abcd\nabcd\nabcd", &[c(2)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(2), c(7)], 1);

    // The cursor at 2 adds 7 again, which merges; 7 adds 12.
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(2), c(7), c(12)], 2);

    // On the last line nothing is added.
    assert_eq!(editor.add_cursor_below(&set), Ok(false));
    assert_sels(&editor, &[c(2), c(7), c(12)], 2);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_cursor_above_builds_a_column() {
    let (set, mut editor) = setup("abcd\nabcd\nabcd", &[c(12)], 0);
    assert_eq!(editor.add_cursor_above(&set), Ok(true));
    assert_sels(&editor, &[c(7), c(12)], 0);
    assert_eq!(editor.add_cursor_above(&set), Ok(true));
    assert_sels(&editor, &[c(2), c(7), c(12)], 0);
    assert_eq!(editor.add_cursor_above(&set), Ok(false));
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_cursor_counts_columns_in_chars() {
    // Line 0 is a😀bc (a 0, 😀 1..5, b 5, c 6, LF 7). Line 1 is x (x 8,
    // LF 9). Line 2 is é中文字 (é 10..12, 中 12..15, 文 15..18, 字 18..21).
    let text = "a😀bc\nx\né中文字";

    // Column 2 (after 😀) on a one-char line: the end of its text.
    let (set, mut editor) = setup(text, &[c(5)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(5), c(9)], 1);

    // Column 1 on line 1 is after é on line 2.
    let (set, mut editor) = setup(text, &[c(9)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(9), c(12)], 1);

    // Column 3 on line 2 (after 文) is the end of line 1.
    let (set, mut editor) = setup(text, &[c(18)], 0);
    assert_eq!(editor.add_cursor_above(&set), Ok(true));
    assert_sels(&editor, &[c(9), c(18)], 0);

    // Column 1 on line 1 is after a on line 0, not inside 😀.
    let (set, mut editor) = setup(text, &[c(9)], 0);
    assert_eq!(editor.add_cursor_above(&set), Ok(true));
    assert_sels(&editor, &[c(1), c(9)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_cursor_around_crlf_and_cr() {
    // a 0, b 1, CR 2, LF 3, c 4, d 5, CR 6, LF 7, e 8, f 9.
    let text = "ab\r\ncd\r\nef";

    // The end of line 0's text goes to the end of line 1's, before its CR.
    let (set, mut editor) = setup(text, &[c(2)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(2), c(6)], 1);

    let (set, mut editor) = setup(text, &[c(10)], 0);
    assert_eq!(editor.add_cursor_above(&set), Ok(true));
    assert_sels(&editor, &[c(6), c(10)], 0);

    // Between CR and LF the column is capped at the line's text: 2.
    let (set, mut editor) = setup(text, &[c(3)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(3), c(6)], 1);

    // A lone CR breaks lines too.
    let (set, mut editor) = setup("ab\rcd", &[c(1)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(1), c(4)], 1);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_cursor_uses_the_head() {
    // Head at 3: the new cursor is at column 3 of line 1.
    let (set, mut editor) = setup("abcd\nabcd", &[s(0, 3)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[s(0, 3), c(8)], 1);

    // Head at 1.
    let (set, mut editor) = setup("abcd\nabcd", &[s(3, 1)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[s(3, 1), c(6)], 1);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_cursor_merges_into_existing_selections() {
    // The cursor at 1 adds 6, inside 5..9, which absorbs it. The primary is
    // on the last line and adds nothing, so nothing changes.
    let (set, mut editor) = setup("abcd\nabcd", &[c(1), s(5, 9)], 1);
    assert_eq!(editor.add_cursor_below(&set), Ok(false));
    assert_sels(&editor, &[c(1), s(5, 9)], 1);

    // A cursor added at the end of a leftwards selection keeps it
    // leftwards, and the primary is the selection it merged into.
    let (set, mut editor) = setup("abcd\nabcd", &[c(4), s(9, 6)], 0);
    assert_eq!(editor.add_cursor_below(&set), Ok(true));
    assert_sels(&editor, &[c(4), s(9, 6)], 1);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn add_cursor_on_one_line() {
    let (set, mut editor) = setup("abc", &[c(1)], 0);
    assert_eq!(editor.add_cursor_above(&set), Ok(false));
    assert_eq!(editor.add_cursor_below(&set), Ok(false));
    assert_sels(&editor, &[c(1)], 0);
}
