//! T-108 acceptance tests: undo groups. Between `Editor::begin_undo_group`
//! and `Editor::end_undo_group`, every editing command folds its edits into
//! one transaction with `BufferSet::amend`, so one undo reverts them all.

use helm_editor::{Editor, EditorError, Motion, Selection, Selections};
use helm_text::{Buffer, BufferId, BufferSet, Edit, FileFormat, LineEndings, Transaction};

fn setup(text: &str, selections: &[Selection]) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    editor.set_selections(Selections::new(selections.to_vec(), 0).expect("non-empty"));
    (set, editor)
}

fn text(set: &BufferSet, editor: &Editor) -> String {
    set.get(editor.buffer()).expect("in the set").to_string()
}

fn cursors(editor: &Editor) -> Vec<usize> {
    editor
        .selections()
        .as_slice()
        .iter()
        .map(|s| {
            assert!(s.is_empty(), "{s:?}");
            s.head
        })
        .collect()
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn a_group_undoes_and_redoes_in_one_step() {
    let (mut set, mut e) = setup("ab", &[Selection::cursor(1)]);
    assert!(!e.in_undo_group());
    e.begin_undo_group();
    assert!(e.in_undo_group());
    e.insert(&mut set, "x").unwrap();
    e.insert(&mut set, "yz").unwrap();
    e.newline(&mut set).unwrap();
    e.insert(&mut set, "é").unwrap();
    e.delete_backward(&mut set).unwrap();
    e.delete_backward(&mut set).unwrap();
    e.insert(&mut set, "中").unwrap();
    assert!(e.end_undo_group());
    assert!(!e.in_undo_group());
    assert_eq!(text(&set, &e), "axyz中b");
    assert_eq!(cursors(&e), [7]);

    assert!(e.undo(&mut set).unwrap().is_some());
    assert_eq!(text(&set, &e), "ab");
    assert_eq!(cursors(&e), [1]);
    assert_eq!(e.undo(&mut set).unwrap(), None);
    assert!(e.redo(&mut set).unwrap().is_some());
    assert_eq!(text(&set, &e), "axyz中b");
    assert_eq!(cursors(&e), [7]);
    assert_eq!(e.redo(&mut set).unwrap(), None);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn every_edit_in_a_group_returns_the_groups_transaction() {
    let (mut set, mut e) = setup("", &[Selection::cursor(0)]);
    e.begin_undo_group();
    let first = e.insert(&mut set, "a").unwrap().expect("edits");
    assert_eq!(e.insert(&mut set, "b").unwrap(), Some(first));
    assert_eq!(e.newline(&mut set).unwrap(), Some(first));
    assert_eq!(e.delete_backward(&mut set).unwrap(), Some(first));
    assert_eq!(
        e.delete_to(&mut set, Motion::WordLeft).unwrap(),
        Some(first)
    );
    // A command with nothing to do still applies nothing.
    assert_eq!(e.delete_backward(&mut set).unwrap(), None);
    assert_eq!(e.insert(&mut set, "c").unwrap(), Some(first));
    e.end_undo_group();
    assert_eq!(text(&set, &e), "c");
    assert_eq!(e.undo(&mut set).unwrap(), Some(first));
    assert_eq!(text(&set, &e), "");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn edits_outside_a_group_are_their_own_steps() {
    let (mut set, mut e) = setup("", &[Selection::cursor(0)]);
    e.insert(&mut set, "a").unwrap();
    e.begin_undo_group();
    e.insert(&mut set, "b").unwrap();
    e.insert(&mut set, "c").unwrap();
    e.end_undo_group();
    e.insert(&mut set, "d").unwrap();
    e.insert(&mut set, "e").unwrap();
    for expected in ["abcd", "abc", "a", ""] {
        e.undo(&mut set).unwrap();
        assert_eq!(text(&set, &e), expected);
    }
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn an_empty_group_changes_nothing() {
    let (mut set, mut e) = setup("", &[Selection::cursor(0)]);
    e.insert(&mut set, "a").unwrap();
    e.begin_undo_group();
    assert!(e.end_undo_group());
    assert!(!e.end_undo_group());
    e.insert(&mut set, "b").unwrap();
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "a");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn moving_and_setting_selections_keep_the_group_open() {
    let (mut set, mut e) = setup("ab", &[Selection::cursor(2)]);
    e.begin_undo_group();
    e.insert(&mut set, "x").unwrap();
    e.move_cursors(&set, Motion::TextStart).unwrap();
    e.insert(&mut set, "y").unwrap();
    e.set_selections(Selections::single(Selection::new(1, 3)));
    e.insert(&mut set, "z").unwrap();
    e.add_cursor_below(&set).unwrap();
    assert!(e.in_undo_group());
    e.end_undo_group();
    assert_eq!(text(&set, &e), "yzx");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "ab");
    // The selections from before the group's first edit.
    assert_eq!(cursors(&e), [2]);
    e.redo(&mut set).unwrap();
    // The selections after its last edit.
    assert_eq!(cursors(&e), [2]);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn undo_and_redo_end_the_group() {
    let (mut set, mut e) = setup("", &[Selection::cursor(0)]);
    e.insert(&mut set, "a").unwrap();
    e.begin_undo_group();
    e.insert(&mut set, "b").unwrap();
    e.undo(&mut set).unwrap();
    assert!(!e.in_undo_group());
    assert_eq!(text(&set, &e), "a");
    e.insert(&mut set, "c").unwrap();
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "a");

    e.begin_undo_group();
    e.redo(&mut set).unwrap();
    assert!(!e.in_undo_group());
    assert_eq!(text(&set, &e), "ac");
    e.insert(&mut set, "d").unwrap();
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "ac");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn begin_while_open_starts_a_new_group() {
    let (mut set, mut e) = setup("", &[Selection::cursor(0)]);
    e.begin_undo_group();
    e.insert(&mut set, "a").unwrap();
    e.insert(&mut set, "b").unwrap();
    e.begin_undo_group();
    assert!(e.in_undo_group());
    e.insert(&mut set, "c").unwrap();
    e.insert(&mut set, "d").unwrap();
    e.end_undo_group();
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "ab");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn an_edit_from_outside_starts_a_new_step_in_the_group() {
    let (mut set, mut e) = setup("ab", &[Selection::cursor(0)]);
    e.begin_undo_group();
    e.insert(&mut set, "x").unwrap();
    // Another writer, such as an agent, edits the buffer.
    let mut outside = Transaction::new();
    outside.push(Edit::insert(e.buffer(), 3, "!"));
    set.apply(outside).unwrap();
    // The group's transaction is no longer on top, so this one can't be
    // amended into it: it starts a new step, and the group goes on from
    // there.
    let second = e.insert(&mut set, "y").unwrap().expect("edits");
    assert_eq!(e.insert(&mut set, "z").unwrap(), Some(second));
    e.end_undo_group();
    assert_eq!(text(&set, &e), "xyzab!");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "xab!");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "xab");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "ab");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn a_group_covers_every_cursor() {
    let (mut set, mut e) = setup(
        "a\r\nb\r\nc",
        &[
            Selection::cursor(1),
            Selection::cursor(4),
            Selection::cursor(7),
        ],
    );
    e.begin_undo_group();
    e.insert(&mut set, "1").unwrap();
    e.newline(&mut set).unwrap();
    e.insert(&mut set, "2").unwrap();
    e.end_undo_group();
    assert_eq!(text(&set, &e), "a1\r\n2\r\nb1\r\n2\r\nc1\r\n2");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "a\r\nb\r\nc");
    assert_eq!(cursors(&e), [1, 4, 7]);
    assert_eq!(e.undo(&mut set).unwrap(), None);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn a_group_drops_redo_like_any_edit() {
    let (mut set, mut e) = setup("", &[Selection::cursor(0)]);
    e.insert(&mut set, "a").unwrap();
    e.insert(&mut set, "b").unwrap();
    e.undo(&mut set).unwrap();
    e.begin_undo_group();
    e.insert(&mut set, "c").unwrap();
    e.insert(&mut set, "d").unwrap();
    e.end_undo_group();
    assert_eq!(e.redo(&mut set).unwrap(), None);
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "a");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn an_error_in_a_group_changes_nothing() {
    let (mut set, mut e) = setup("ab", &[Selection::cursor(1)]);
    e.begin_undo_group();
    e.insert(&mut set, "x").unwrap();
    let mut other = BufferSet::new();
    let missing: BufferId = e.buffer();
    assert_eq!(
        e.insert(&mut other, "y"),
        Err(EditorError::UnknownBuffer { buffer: missing })
    );
    assert!(e.in_undo_group());
    e.insert(&mut set, "z").unwrap();
    e.end_undo_group();
    assert_eq!(text(&set, &e), "axzb");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "ab");
}
