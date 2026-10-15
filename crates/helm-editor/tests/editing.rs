//! T-105 acceptance tests: typing, newline and deletion at every cursor as
//! one transaction, where each selection lands afterwards, clamping stale
//! selections, and restoring selections on undo and redo.

use helm_editor::{Editor, EditorError, Selection, Selections};
use helm_text::{Buffer, BufferSet, Edit, FileFormat, LineEndings, Transaction};

/// A cursor at `offset`.
fn c(offset: usize) -> Selection {
    Selection::cursor(offset)
}

/// A selection from `anchor` to `head`.
fn s(anchor: usize, head: usize) -> Selection {
    Selection::new(anchor, head)
}

/// `Selections::new`, which must accept the input.
fn sels(selections: &[Selection], primary: usize) -> Selections {
    Selections::new(selections.to_vec(), primary).expect("non-empty, primary in range")
}

/// The format of a file whose text is `text`, without a BOM.
fn format_of(text: &str) -> FileFormat {
    FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    }
}

/// A set with one buffer holding `text`, and an editor on it with the
/// file's format and the given selections.
fn setup(text: &str, selections: &[Selection], primary: usize) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let mut editor = Editor::new(id, format_of(text));
    editor.set_selections(sels(selections, primary));
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
fn typing_at_three_cursors_is_one_transaction() {
    let (mut set, mut editor) = setup("ab\ncd\nef", &[c(1), c(4), c(7)], 1);
    let id = editor.insert(&mut set, "X").unwrap();
    assert!(id.is_some());
    assert_eq!(text(&set, &editor), "aXb\ncXd\neXf");
    assert_sels(&editor, &[c(2), c(6), c(10)], 1);

    // One undo in the buffer set reverts all three.
    assert_eq!(set.undo(editor.buffer()), Ok(id));
    assert_eq!(text(&set, &editor), "ab\ncd\nef");
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn typing_replaces_every_selection() {
    // "one", "two" (selected leftwards) and "three".
    let (mut set, mut editor) = setup("one two three", &[s(0, 3), s(7, 4), s(8, 13)], 2);
    editor.insert(&mut set, "1").unwrap();
    assert_eq!(text(&set, &editor), "1 1 1");
    assert_sels(&editor, &[c(1), c(3), c(5)], 2);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn typing_moves_cursors_by_bytes() {
    // é is 2 bytes, 中 3 and 😀 4. Cursors after each on the first line, and
    // at the end of the text.
    let (mut set, mut editor) = setup("é中😀\né中😀", &[c(2), c(5), c(9), c(19)], 0);
    editor.insert(&mut set, "ü").unwrap();
    assert_eq!(text(&set, &editor), "éü中ü😀ü\né中😀ü");
    assert_sels(&editor, &[c(4), c(9), c(15), c(27)], 0);

    let (mut set, mut editor) = setup("a😀b", &[s(1, 5)], 0);
    editor.insert(&mut set, "中中").unwrap();
    assert_eq!(text(&set, &editor), "a中中b");
    assert_sels(&editor, &[c(7)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn typing_over_touching_selections() {
    // Two non-empty selections that touch stay apart, and both get the text.
    let (mut set, mut editor) = setup("abc", &[s(0, 1), s(1, 2)], 0);
    editor.insert(&mut set, "xy").unwrap();
    assert_eq!(text(&set, &editor), "xyxyc");
    assert_sels(&editor, &[c(2), c(4)], 0);

    // Typing nothing over them deletes both, and the two cursors merge.
    let (mut set, mut editor) = setup("abc", &[s(0, 1), s(1, 2)], 1);
    editor.insert(&mut set, "").unwrap();
    assert_eq!(text(&set, &editor), "c");
    assert_sels(&editor, &[c(0)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn typing_nothing_at_cursors_is_no_transaction() {
    let (mut set, mut editor) = setup("ab", &[c(1), c(2)], 0);
    assert_eq!(editor.insert(&mut set, ""), Ok(None));
    assert_eq!(text(&set, &editor), "ab");
    assert_sels(&editor, &[c(1), c(2)], 0);
    assert_eq!(editor.undo(&mut set), Ok(None));
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn typed_line_breaks_go_in_as_given() {
    // No conversion to the file's line endings: that is paste's job, later.
    let (mut set, mut editor) = setup("a\r\nb", &[c(1)], 0);
    editor.insert(&mut set, "x\ny").unwrap();
    assert_eq!(text(&set, &editor), "ax\ny\r\nb");
    assert_sels(&editor, &[c(4)], 0);

    // Between the CR and LF of a CRLF is a valid cursor position.
    let (mut set, mut editor) = setup("a\r\nb", &[c(2)], 0);
    editor.insert(&mut set, "x").unwrap();
    assert_eq!(text(&set, &editor), "a\rx\nb");
    assert_sels(&editor, &[c(3)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn newline_in_a_crlf_file_is_crlf() {
    let (mut set, mut editor) = setup("a\r\nb", &[c(1), c(4)], 0);
    editor.newline(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a\r\n\r\nb\r\n");
    assert_sels(&editor, &[c(3), c(8)], 0);

    // Mixed, with CRLF the most common.
    let (mut set, mut editor) = setup("a\r\nb\r\nc\nd", &[c(9)], 0);
    editor.newline(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a\r\nb\r\nc\nd\r\n");
    assert_sels(&editor, &[c(11)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn newline_in_lf_cr_and_new_files() {
    let (mut set, mut editor) = setup("a\nb", &[c(1), c(3)], 1);
    editor.newline(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a\n\nb\n");
    assert_sels(&editor, &[c(2), c(5)], 1);

    let (mut set, mut editor) = setup("a\rb", &[c(3)], 0);
    editor.newline(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a\rb\r");

    // No line breaks yet: LF.
    let (mut set, mut editor) = setup("ab", &[c(1)], 0);
    editor.newline(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a\nb");

    // The editor's format decides, not the text: a new editor with the
    // default format on CRLF text still types LF.
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from("a\r\nb"));
    let mut editor = Editor::new(id, FileFormat::default());
    editor.newline(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "\na\r\nb");
    assert_sels(&editor, &[c(1)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn newline_replaces_selections() {
    let (mut set, mut editor) = setup("abc\r\nxyz", &[s(1, 2), s(8, 6)], 0);
    editor.newline(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a\r\nc\r\nx\r\n");
    assert_sels(&editor, &[c(3), c(9)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn delete_backward_removes_one_character_per_cursor() {
    // Cursors after é, 中 and 😀.
    let (mut set, mut editor) = setup("aé-中-😀-", &[c(3), c(7), c(12)], 2);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a---");
    assert_sels(&editor, &[c(1), c(2), c(3)], 2);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn delete_backward_and_crlf() {
    // Right after a CRLF: both bytes go.
    let (mut set, mut editor) = setup("a\r\nb", &[c(3)], 0);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "ab");
    assert_sels(&editor, &[c(1)], 0);

    // Between the CR and the LF: only the CR goes.
    let (mut set, mut editor) = setup("a\r\nb", &[c(2)], 0);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a\nb");
    assert_sels(&editor, &[c(1)], 0);

    // Both: the ranges 1..2 and 1..3 overlap and are joined.
    let (mut set, mut editor) = setup("a\r\nb", &[c(2), c(3)], 1);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "ab");
    assert_sels(&editor, &[c(1)], 0);

    // A lone CR is one character.
    let (mut set, mut editor) = setup("a\rb", &[c(2)], 0);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "ab");
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn delete_backward_at_the_start() {
    // Only the cursor that has something before it deletes.
    let (mut set, mut editor) = setup("ab", &[c(0), c(2)], 1);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a");
    assert_sels(&editor, &[c(0), c(1)], 1);

    // Nothing to delete anywhere: no transaction.
    let (mut set, mut editor) = setup("ab", &[c(0)], 0);
    assert_eq!(editor.delete_backward(&mut set), Ok(None));
    assert_eq!(text(&set, &editor), "ab");
    assert_eq!(editor.undo(&mut set), Ok(None));
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn delete_removes_selected_text() {
    let (mut set, mut editor) = setup("one two three", &[s(3, 0), c(5), s(8, 13)], 1);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), " wo ");
    assert_sels(&editor, &[c(0), c(1), c(4)], 1);

    let (mut set, mut editor) = setup("one two three", &[s(3, 0), c(5), s(8, 13)], 1);
    editor.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), " to ");
    assert_sels(&editor, &[c(0), c(2), c(4)], 1);

    // Touching selections both go, and their cursors merge.
    let (mut set, mut editor) = setup("abc", &[s(0, 1), s(1, 2)], 0);
    editor.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "c");
    assert_sels(&editor, &[c(0)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn delete_forward_removes_one_character_per_cursor() {
    // é is 0..2, CR 2, LF 3, 😀 4..8.
    let (mut set, mut editor) = setup("é\r\n😀", &[c(0), c(4)], 1);
    editor.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "\r\n");
    assert_sels(&editor, &[c(0), c(2)], 1);

    // Right before a CRLF: both bytes go.
    let (mut set, mut editor) = setup("é\r\n😀", &[c(2)], 0);
    editor.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "é😀");
    assert_sels(&editor, &[c(2)], 0);

    // Between the CR and the LF: only the LF goes.
    let (mut set, mut editor) = setup("é\r\n😀", &[c(3)], 0);
    editor.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "é\r😀");
    assert_sels(&editor, &[c(3)], 0);

    // Both, plus a cursor at the end: 2..4 and 3..4 are joined, and the end
    // deletes nothing.
    let (mut set, mut editor) = setup("é\r\n😀", &[c(2), c(3), c(8)], 2);
    editor.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "é😀");
    assert_sels(&editor, &[c(2), c(6)], 1);

    // Nothing to delete anywhere: no transaction.
    let (mut set, mut editor) = setup("é\r\n😀", &[c(8)], 0);
    assert_eq!(editor.delete_forward(&mut set), Ok(None));
    assert_eq!(text(&set, &editor), "é\r\n😀");
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn stale_selections_are_clamped() {
    // a is 0, 😀 1..5, b 5. 3 is inside 😀 and 100 is past the end.
    let (mut set, mut editor) = setup("a😀b", &[c(3), c(100)], 1);
    editor.insert(&mut set, "x").unwrap();
    assert_eq!(text(&set, &editor), "ax😀bx");
    assert_sels(&editor, &[c(2), c(8)], 1);

    // Clamped ends keep their direction.
    let (mut set, mut editor) = setup("a😀b", &[s(100, 3)], 0);
    editor.insert(&mut set, "x").unwrap();
    assert_eq!(text(&set, &editor), "ax");
    assert_sels(&editor, &[c(2)], 0);

    // Clamping can merge: 2, 3 and 4 all clamp to 1.
    let (mut set, mut editor) = setup("a😀b", &[c(2), c(3), c(4)], 2);
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "😀b");
    assert_sels(&editor, &[c(0)], 0);

    // A command with nothing to do still leaves the clamped selections.
    let (mut set, mut editor) = setup("ab", &[c(7)], 0);
    assert_eq!(editor.delete_forward(&mut set), Ok(None));
    assert_sels(&editor, &[c(2)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn every_command_checks_the_buffer() {
    let mut other = BufferSet::new();
    other.add(Buffer::from("x"));
    let id = other.add(Buffer::from("y"));
    let mut set = BufferSet::new();
    let abc = set.add(Buffer::from("abc"));
    let mut editor = Editor::new(id, FileFormat::default());
    editor.set_selections(sels(&[c(1)], 0));
    let unknown = Err(EditorError::UnknownBuffer { buffer: id });

    assert_eq!(editor.insert(&mut set, "x"), unknown);
    assert_eq!(editor.newline(&mut set), unknown);
    assert_eq!(editor.delete_backward(&mut set), unknown);
    assert_eq!(editor.delete_forward(&mut set), unknown);
    assert_eq!(editor.undo(&mut set), unknown);
    assert_eq!(editor.redo(&mut set), unknown);
    let unknown = Err(EditorError::UnknownBuffer { buffer: id });
    assert_eq!(editor.add_next_match(&set), unknown);
    assert_eq!(editor.select_all_matches(&set), unknown);
    assert_eq!(editor.add_cursor_above(&set), unknown);
    assert_eq!(editor.add_cursor_below(&set), unknown);

    assert_sels(&editor, &[c(1)], 0);
    assert_eq!(set.get(abc).unwrap().to_string(), "abc");
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn undo_and_redo_restore_selections() {
    let (mut set, mut editor) = setup("abc", &[s(1, 0), c(3)], 1);
    let id = editor.insert(&mut set, "X").unwrap();
    assert_eq!(text(&set, &editor), "XbcX");
    assert_sels(&editor, &[c(1), c(4)], 1);

    // Back to exactly the selections before typing, direction and primary
    // included.
    assert_eq!(editor.undo(&mut set), Ok(id));
    assert_eq!(text(&set, &editor), "abc");
    assert_sels(&editor, &[s(1, 0), c(3)], 1);

    assert_eq!(editor.redo(&mut set), Ok(id));
    assert_eq!(text(&set, &editor), "XbcX");
    assert_sels(&editor, &[c(1), c(4)], 1);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn undo_and_redo_walk_back_through_each_command() {
    let (mut set, mut editor) = setup("", &[c(0)], 0);
    editor.insert(&mut set, "a").unwrap();
    editor.set_selections(sels(&[c(0), c(1)], 0));
    editor.insert(&mut set, "b").unwrap();
    assert_eq!(text(&set, &editor), "bab");
    editor.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a");
    assert_sels(&editor, &[c(0), c(1)], 0);

    editor.undo(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "bab");
    assert_sels(&editor, &[c(1), c(3)], 0);
    editor.undo(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a");
    assert_sels(&editor, &[c(0), c(1)], 0);
    editor.undo(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "");
    assert_sels(&editor, &[c(0)], 0);
    assert_eq!(editor.undo(&mut set), Ok(None));
    assert_sels(&editor, &[c(0)], 0);

    editor.redo(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a");
    assert_sels(&editor, &[c(1)], 0);
    editor.redo(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "bab");
    assert_sels(&editor, &[c(1), c(3)], 0);
    editor.redo(&mut set).unwrap();
    assert_eq!(text(&set, &editor), "a");
    assert_sels(&editor, &[c(0), c(1)], 0);
    assert_eq!(editor.redo(&mut set), Ok(None));
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn undo_restores_the_clamped_selections() {
    let (mut set, mut editor) = setup("ab", &[c(50)], 0);
    editor.insert(&mut set, "x").unwrap();
    assert_eq!(text(&set, &editor), "abx");
    editor.undo(&mut set).unwrap();
    assert_sels(&editor, &[c(2)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn moving_selections_is_not_undone() {
    let (mut set, mut editor) = setup("ab\ncd", &[c(0)], 0);
    let id = editor.insert(&mut set, "x").unwrap();
    editor.set_selections(sels(&[c(5)], 0));
    editor.add_cursor_above(&set).unwrap();
    assert_sels(&editor, &[c(1), c(5)], 0);

    // The first undo reverts the typing, and puts back its selections.
    assert_eq!(editor.undo(&mut set), Ok(id));
    assert_eq!(text(&set, &editor), "ab\ncd");
    assert_sels(&editor, &[c(0)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn undoing_an_outside_transaction_keeps_the_selections() {
    let (mut set, mut editor) = setup("ab", &[c(2)], 0);
    let id = editor.buffer();
    let outside = set
        .apply(Transaction::from(vec![Edit::insert(id, 0, "zz")]))
        .unwrap();
    assert_eq!(text(&set, &editor), "zzab");

    assert_eq!(editor.undo(&mut set), Ok(Some(outside)));
    assert_eq!(text(&set, &editor), "ab");
    assert_sels(&editor, &[c(2)], 0);
    assert_eq!(editor.redo(&mut set), Ok(Some(outside)));
    assert_sels(&editor, &[c(2)], 0);

    // An outside edit that shortens the text leaves the cursor past the
    // end, and the next command clamps it.
    let (mut set, mut editor) = setup("abcd", &[c(4)], 0);
    let id = editor.buffer();
    set.apply(Transaction::from(vec![Edit::delete(id, 0..2)]))
        .unwrap();
    editor.insert(&mut set, "x").unwrap();
    assert_eq!(text(&set, &editor), "cdx");
    assert_sels(&editor, &[c(3)], 0);
}

#[test]
#[ignore = "T-105: not implemented yet"]
fn blocked_undo_changes_nothing() {
    let mut set = BufferSet::new();
    let a = set.add(Buffer::from("a"));
    let b = set.add(Buffer::from("b"));
    let mut editor = Editor::new(a, FileFormat::default());
    editor.set_selections(sels(&[c(1)], 0));

    // A rename across both buffers, then a later edit in b only.
    let both = set
        .apply(Transaction::from(vec![
            Edit::insert(a, 0, "1"),
            Edit::insert(b, 0, "1"),
        ]))
        .unwrap();
    set.apply(Transaction::from(vec![Edit::insert(b, 0, "2")]))
        .unwrap();

    assert_eq!(
        editor.undo(&mut set),
        Err(EditorError::Blocked {
            transaction: both,
            buffer: b,
        })
    );
    assert_eq!(text(&set, &editor), "1a");
    assert_sels(&editor, &[c(1)], 0);
}
