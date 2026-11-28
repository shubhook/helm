//! T-112 acceptance tests: typing groups on `Editor` alone. Under
//! `UndoGrouping::Typing`, the editor opens and closes undo groups itself,
//! so a run of typing undoes in one step. `Editor`'s "Typing groups" docs
//! hold the rules.

use std::time::Duration;

use helm_editor::{
    CommandRegistry, Editor, EditorError, Invocation, Motion, Selection, Selections, UndoGrouping,
    vim_commands,
};
use helm_text::{Buffer, BufferSet, Edit, FileFormat, LineEndings, Transaction};

const PAUSE: Duration = Editor::TYPING_PAUSE;

/// An editor on `text` with `selections`, under `grouping`.
fn editor_with(
    text: &str,
    selections: &[Selection],
    grouping: UndoGrouping,
) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    editor.set_selections(Selections::new(selections.to_vec(), 0).expect("non-empty"));
    if grouping != UndoGrouping::Explicit {
        editor.set_undo_grouping(grouping);
    }
    (set, editor)
}

/// An editor under `UndoGrouping::Typing`.
fn typing(text: &str, selections: &[Selection]) -> (BufferSet, Editor) {
    editor_with(text, selections, UndoGrouping::Typing)
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

/// Types each char of `chars` as its own `insert`.
fn type_chars(set: &mut BufferSet, editor: &mut Editor, chars: &str) {
    for c in chars.chars() {
        editor
            .insert(set, &c.to_string())
            .expect("the buffer is in the set")
            .expect("typing applies a transaction");
    }
}

/// Undoes until there is nothing left to undo, and returns the text after
/// each undo: one entry per undo step, newest first.
fn steps(set: &mut BufferSet, editor: &mut Editor) -> Vec<String> {
    let mut texts = Vec::new();
    while editor.undo(set).expect("not blocked").is_some() {
        texts.push(text(set, editor));
        assert!(texts.len() < 1000, "undo never ran out");
    }
    texts
}

fn at(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn the_default_grouping_is_explicit_and_unchanged() {
    assert_eq!(UndoGrouping::default(), UndoGrouping::Explicit);
    let (mut set, mut e) = editor_with("", &[Selection::cursor(0)], UndoGrouping::Explicit);
    assert_eq!(e.undo_grouping(), UndoGrouping::Explicit);
    type_chars(&mut set, &mut e, "abc");
    assert!(!e.in_undo_group());
    assert_eq!(steps(&mut set, &mut e), ["ab", "a", ""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_typed_word_undoes_and_redoes_in_one_step() {
    let (mut set, mut e) = typing("x", &[Selection::cursor(1)]);
    assert_eq!(e.undo_grouping(), UndoGrouping::Typing);
    assert!(!e.in_undo_group());
    let first = e.insert(&mut set, "h").unwrap().expect("applied");
    assert!(e.in_undo_group());
    for c in ["e", "l", "l", "o"] {
        assert_eq!(e.insert(&mut set, c).unwrap(), Some(first));
    }
    assert_eq!(text(&set, &e), "xhello");

    assert_eq!(e.undo(&mut set).unwrap(), Some(first));
    assert_eq!(text(&set, &e), "x");
    assert_eq!(cursors(&e), [1]);
    assert!(!e.in_undo_group());
    assert_eq!(e.undo(&mut set).unwrap(), None);

    assert_eq!(e.redo(&mut set).unwrap(), Some(first));
    assert_eq!(text(&set, &e), "xhello");
    assert_eq!(cursors(&e), [6]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn three_cursors_in_crlf_type_as_one_step() {
    let (mut set, mut e) = typing(
        "a\r\nb\r\nc",
        &[
            Selection::cursor(1),
            Selection::cursor(4),
            Selection::cursor(7),
        ],
    );
    type_chars(&mut set, &mut e, "xy");
    assert_eq!(text(&set, &e), "axy\r\nbxy\r\ncxy");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "a\r\nb\r\nc");
    assert_eq!(cursors(&e), [1, 4, 7]);
    assert_eq!(e.undo(&mut set).unwrap(), None);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn cursors_that_merge_during_a_run_keep_the_step() {
    let (mut set, mut e) = typing("xabcd", &[Selection::cursor(3), Selection::cursor(5)]);
    e.delete_backward(&mut set).unwrap().expect("applied");
    assert_eq!(text(&set, &e), "xac");
    e.delete_backward(&mut set).unwrap().expect("applied");
    assert_eq!(text(&set, &e), "x");
    assert_eq!(cursors(&e), [1]);
    e.delete_backward(&mut set).unwrap().expect("applied");
    assert_eq!(text(&set, &e), "");
    assert_eq!(steps(&mut set, &mut e), ["xabcd"]);
}

/// A move, run between typing `x` and typing `y`.
type MoveFn = fn(&mut Editor, &BufferSet);

#[test]
#[ignore = "T-112: not implemented yet"]
fn every_kind_of_move_ends_the_step() {
    let start = "ab cd\nef";
    let cases: [(&str, Vec<Selection>, MoveFn); 11] = [
        ("move left", vec![Selection::cursor(2)], |e, set| {
            assert!(e.move_cursors(set, Motion::Left).unwrap());
        }),
        ("move to the end", vec![Selection::cursor(2)], |e, set| {
            assert!(e.move_cursors(set, Motion::TextEnd).unwrap());
        }),
        ("move down", vec![Selection::cursor(2)], |e, set| {
            assert!(e.move_cursors(set, Motion::Down).unwrap());
        }),
        ("extend right", vec![Selection::cursor(2)], |e, set| {
            assert!(e.extend_selections(set, Motion::Right).unwrap());
        }),
        ("set selections", vec![Selection::cursor(2)], |e, _| {
            e.set_selections(Selections::single(Selection::cursor(0)));
        }),
        (
            "add a cursor below",
            vec![Selection::cursor(2)],
            |e, set| {
                assert!(e.add_cursor_below(set).unwrap());
            },
        ),
        (
            "add a cursor above",
            vec![Selection::cursor(8)],
            |e, set| {
                assert!(e.add_cursor_above(set).unwrap());
            },
        ),
        (
            "add the next match",
            vec![Selection::cursor(2)],
            |e, set| {
                assert!(e.add_next_match(set).unwrap());
            },
        ),
        (
            "select all matches",
            vec![Selection::cursor(2)],
            |e, set| {
                assert!(e.select_all_matches(set).unwrap());
            },
        ),
        ("select all", vec![Selection::cursor(2)], |e, set| {
            assert!(e.select_all(set).unwrap());
        }),
        (
            "collapse",
            vec![Selection::cursor(2), Selection::cursor(8)],
            |e, set| {
                assert!(e.collapse_selections(set).unwrap());
            },
        ),
    ];
    for (name, selections, run) in cases {
        let (mut set, mut e) = typing(start, &selections);
        type_chars(&mut set, &mut e, "x");
        run(&mut e, &set);
        type_chars(&mut set, &mut e, "y");
        let texts = steps(&mut set, &mut e);
        assert_eq!(texts.len(), 2, "{name}: {texts:?}");
        assert_eq!(texts[1], start, "{name}");
    }
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_move_that_changes_nothing_keeps_the_step() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "a");
    assert!(!e.move_cursors(&set, Motion::Right).unwrap());
    assert!(!e.move_cursors(&set, Motion::Down).unwrap());
    assert!(!e.collapse_selections(&set).unwrap());
    e.set_selections(Selections::single(Selection::cursor(1)));
    type_chars(&mut set, &mut e, "b");
    assert_eq!(text(&set, &e), "ab");
    assert_eq!(steps(&mut set, &mut e), [""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_move_and_a_move_back_end_the_step() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "a");
    e.move_cursors(&set, Motion::Left).unwrap();
    e.move_cursors(&set, Motion::Right).unwrap();
    type_chars(&mut set, &mut e, "b");
    e.set_selections(Selections::single(Selection::cursor(0)));
    e.set_selections(Selections::single(Selection::cursor(2)));
    type_chars(&mut set, &mut e, "c");
    assert_eq!(text(&set, &e), "abc");
    assert_eq!(steps(&mut set, &mut e), ["ab", "a", ""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn switching_between_typing_and_deleting_ends_the_step() {
    let (mut set, mut e) = typing("Z", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "abc");
    e.delete_backward(&mut set).unwrap();
    e.delete_backward(&mut set).unwrap();
    type_chars(&mut set, &mut e, "xy");
    e.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &e), "axy");
    assert_eq!(steps(&mut set, &mut e), ["axyZ", "aZ", "abcZ", "Z"]);

    // Deleting backward then forward is two steps too.
    let (mut set, mut e) = typing("abc", &[Selection::cursor(1)]);
    e.delete_backward(&mut set).unwrap();
    e.delete_forward(&mut set).unwrap();
    assert_eq!(text(&set, &e), "c");
    assert_eq!(steps(&mut set, &mut e), ["bc", "abc"]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn word_and_line_deletes_join_the_delete_of_their_direction() {
    let (mut set, mut e) = typing("let foo = bar;", &[Selection::cursor(13)]);
    e.delete_backward(&mut set).unwrap();
    e.delete_to(&mut set, Motion::WordLeft).unwrap();
    assert_eq!(text(&set, &e), "let foo = ;");
    e.delete_to(&mut set, Motion::LineStart).unwrap();
    assert_eq!(text(&set, &e), ";");
    assert_eq!(steps(&mut set, &mut e), ["let foo = bar;"]);

    let (mut set, mut e) = typing("let foo = bar;", &[Selection::cursor(0)]);
    e.delete_forward(&mut set).unwrap();
    e.delete_to(&mut set, Motion::WordRight).unwrap();
    assert_eq!(text(&set, &e), " foo = bar;");
    e.delete_to(&mut set, Motion::LineEnd).unwrap();
    assert_eq!(text(&set, &e), "");
    assert_eq!(steps(&mut set, &mut e), ["let foo = bar;"]);

    // A word delete in the other direction starts a step.
    let (mut set, mut e) = typing("ab cd", &[Selection::cursor(2)]);
    e.delete_backward(&mut set).unwrap();
    assert_eq!(text(&set, &e), "a cd");
    e.delete_to(&mut set, Motion::WordRight).unwrap();
    assert_eq!(text(&set, &e), "a");
    assert_eq!(steps(&mut set, &mut e), ["a cd", "ab cd"]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn enter_starts_a_step_that_typing_after_it_joins() {
    let (mut set, mut e) = typing("\n", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "abc");
    e.newline(&mut set).unwrap();
    type_chars(&mut set, &mut e, "def");
    assert_eq!(text(&set, &e), "abc\ndef\n");
    assert_eq!(steps(&mut set, &mut e), ["abc\n", "\n"]);

    // Two Enters are two steps, and the second takes the typing after it.
    let (mut set, mut e) = typing("\r\n", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "a");
    e.newline(&mut set).unwrap();
    e.newline(&mut set).unwrap();
    type_chars(&mut set, &mut e, "b");
    assert_eq!(text(&set, &e), "a\r\n\r\nb\r\n");
    assert_eq!(steps(&mut set, &mut e), ["a\r\n\r\n", "a\r\n", "\r\n"]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn deleting_a_line_break_starts_a_step() {
    for line_break in ["\n", "\r\n", "\r"] {
        let start = format!("ab{line_break}cd");
        let (mut set, mut e) = typing(&start, &[Selection::cursor(start.len())]);
        e.delete_backward(&mut set).unwrap();
        e.delete_backward(&mut set).unwrap();
        // This one takes the line break.
        e.delete_backward(&mut set).unwrap();
        assert_eq!(text(&set, &e), "ab", "{line_break:?}");
        e.delete_backward(&mut set).unwrap();
        assert_eq!(text(&set, &e), "a", "{line_break:?}");
        assert_eq!(
            steps(&mut set, &mut e),
            [format!("ab{line_break}"), start.clone()],
            "{line_break:?}"
        );

        let (mut set, mut e) = typing(&start, &[Selection::cursor(1)]);
        e.delete_forward(&mut set).unwrap();
        e.delete_forward(&mut set).unwrap();
        assert_eq!(text(&set, &e), "acd", "{line_break:?}");
        e.delete_forward(&mut set).unwrap();
        assert_eq!(text(&set, &e), "ad", "{line_break:?}");
        assert_eq!(
            steps(&mut set, &mut e),
            [format!("a{line_break}cd"), start.clone()],
            "{line_break:?}"
        );
    }
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_word_delete_across_a_line_break_starts_a_step() {
    let (mut set, mut e) = typing("ab\ncd", &[Selection::cursor(5)]);
    e.delete_backward(&mut set).unwrap();
    // From the start of the line, WordLeft crosses the line break.
    e.delete_backward(&mut set).unwrap();
    e.delete_to(&mut set, Motion::WordLeft).unwrap();
    assert_eq!(text(&set, &e), "ab");
    assert_eq!(steps(&mut set, &mut e), ["ab\n", "ab\ncd"]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_pause_of_typing_pause_starts_a_step() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    e.set_time(Duration::ZERO);
    type_chars(&mut set, &mut e, "a");
    e.set_time(PAUSE - Duration::from_nanos(1));
    type_chars(&mut set, &mut e, "b");
    // Measured from the last edit, not the step's first.
    e.set_time(2 * PAUSE - Duration::from_nanos(2));
    type_chars(&mut set, &mut e, "c");
    e.set_time(3 * PAUSE - Duration::from_nanos(2));
    type_chars(&mut set, &mut e, "d");
    assert_eq!(text(&set, &e), "abcd");
    assert_eq!(steps(&mut set, &mut e), ["abc", ""]);

    // A long run of quick keys stays one step.
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    for k in 0..50 {
        e.set_time(at(1000 + 100 * k));
        type_chars(&mut set, &mut e, "x");
    }
    assert_eq!(steps(&mut set, &mut e), [""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_pause_splits_deletes_too() {
    let (mut set, mut e) = typing("abc", &[Selection::cursor(3)]);
    e.set_time(at(0));
    e.delete_backward(&mut set).unwrap();
    e.set_time(at(0) + PAUSE);
    e.delete_backward(&mut set).unwrap();
    assert_eq!(steps(&mut set, &mut e), ["ab", "abc"]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn without_a_time_or_with_time_going_back_nothing_splits() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "abc");
    assert_eq!(steps(&mut set, &mut e), [""]);

    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    e.set_time(at(10_000));
    type_chars(&mut set, &mut e, "a");
    e.set_time(at(5_000));
    type_chars(&mut set, &mut e, "b");
    e.set_time(at(5_500));
    type_chars(&mut set, &mut e, "c");
    assert_eq!(steps(&mut set, &mut e), [""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn under_explicit_grouping_time_changes_nothing() {
    let (mut set, mut e) = editor_with("", &[Selection::cursor(0)], UndoGrouping::Explicit);
    e.set_time(at(0));
    type_chars(&mut set, &mut e, "a");
    e.set_time(at(1));
    type_chars(&mut set, &mut e, "b");
    assert_eq!(steps(&mut set, &mut e), ["a", ""]);

    let (mut set, mut e) = editor_with("", &[Selection::cursor(0)], UndoGrouping::Explicit);
    e.begin_undo_group();
    e.set_time(at(0));
    type_chars(&mut set, &mut e, "a");
    e.set_time(at(0) + 10 * PAUSE);
    type_chars(&mut set, &mut e, "b");
    e.end_undo_group();
    assert_eq!(steps(&mut set, &mut e), [""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn undo_and_redo_end_the_step() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "ab");
    e.undo(&mut set).unwrap();
    assert_eq!(text(&set, &e), "");
    assert!(!e.in_undo_group());
    type_chars(&mut set, &mut e, "cd");
    assert_eq!(text(&set, &e), "cd");
    e.undo(&mut set).unwrap();
    e.redo(&mut set).unwrap();
    assert!(!e.in_undo_group());
    type_chars(&mut set, &mut e, "e");
    assert_eq!(text(&set, &e), "cde");
    assert_eq!(steps(&mut set, &mut e), ["cd", ""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn end_undo_group_ends_a_typing_step_and_makes_a_paste_one_step() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "ab");
    // A paste, as the ED-1 task should make one.
    assert!(e.end_undo_group());
    e.insert(&mut set, "x\ny").unwrap();
    assert!(e.end_undo_group());
    assert!(!e.end_undo_group());
    type_chars(&mut set, &mut e, "cd");
    assert_eq!(text(&set, &e), "abx\nycd");
    assert_eq!(steps(&mut set, &mut e), ["abx\ny", "ab", ""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn an_explicit_group_wins_over_the_typing_rules() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "a");
    e.begin_undo_group();
    assert!(e.in_undo_group());
    e.set_time(at(0));
    type_chars(&mut set, &mut e, "b");
    e.move_cursors(&set, Motion::Left).unwrap();
    e.delete_backward(&mut set).unwrap();
    e.newline(&mut set).unwrap();
    e.set_time(at(0) + 10 * PAUSE);
    type_chars(&mut set, &mut e, "c");
    assert_eq!(text(&set, &e), "\ncb");
    assert!(e.end_undo_group());
    // The next edit starts a typing step of its own.
    type_chars(&mut set, &mut e, "d");
    assert!(e.in_undo_group());
    assert_eq!(text(&set, &e), "\ncdb");
    assert_eq!(steps(&mut set, &mut e), ["\ncb", "a", ""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn changing_the_grouping_ends_any_group() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "a");
    // The same grouping keeps the typing group.
    e.set_undo_grouping(UndoGrouping::Typing);
    assert!(e.in_undo_group());
    type_chars(&mut set, &mut e, "b");
    e.set_undo_grouping(UndoGrouping::Explicit);
    assert_eq!(e.undo_grouping(), UndoGrouping::Explicit);
    assert!(!e.in_undo_group());
    type_chars(&mut set, &mut e, "c");

    // The same for an explicit group.
    e.begin_undo_group();
    e.set_undo_grouping(UndoGrouping::Explicit);
    assert!(e.in_undo_group());
    type_chars(&mut set, &mut e, "d");
    e.set_undo_grouping(UndoGrouping::Typing);
    assert!(!e.in_undo_group());
    type_chars(&mut set, &mut e, "e");
    assert_eq!(text(&set, &e), "abcde");
    assert_eq!(steps(&mut set, &mut e), ["abcd", "abc", "ab", ""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn an_edit_with_nothing_to_do_keeps_the_step() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "ab");
    // Delete at the end of the text deletes nothing.
    assert_eq!(e.delete_forward(&mut set).unwrap(), None);
    assert_eq!(e.delete_to(&mut set, Motion::TextEnd).unwrap(), None);
    assert!(e.in_undo_group());
    type_chars(&mut set, &mut e, "c");
    assert_eq!(steps(&mut set, &mut e), [""]);

    // And backspace at the start, before any typing, opens nothing.
    let (mut set, mut e) = typing("ab", &[Selection::cursor(0)]);
    assert_eq!(e.delete_backward(&mut set).unwrap(), None);
    assert!(!e.in_undo_group());
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn an_unknown_buffer_changes_nothing() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    let mut other = BufferSet::new();
    type_chars(&mut set, &mut e, "a");
    e.set_time(at(10));
    assert!(matches!(
        e.delete_backward(&mut other),
        Err(EditorError::UnknownBuffer { .. })
    ));
    assert!(matches!(
        e.newline(&mut other),
        Err(EditorError::UnknownBuffer { .. })
    ));
    assert!(e.in_undo_group());
    type_chars(&mut set, &mut e, "b");
    assert_eq!(steps(&mut set, &mut e), [""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn an_outside_transaction_splits_the_run() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "ab");
    // An agent appends to the buffer.
    let mut outside = Transaction::new();
    outside.push(Edit::insert(e.buffer(), 2, "Z"));
    set.apply(outside).unwrap();
    type_chars(&mut set, &mut e, "cd");
    assert_eq!(text(&set, &e), "abcdZ");
    // `cd`, then the agent's edit, then `ab`.
    assert_eq!(steps(&mut set, &mut e), ["abZ", "ab", ""]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_vim_command_under_typing_grouping_is_its_own_step() {
    let registry = CommandRegistry::with_vim_commands();
    let (mut set, mut e) = typing("abc", &[Selection::cursor(0)]);
    type_chars(&mut set, &mut e, "x");
    registry
        .run(
            &Invocation::new(vim_commands::DELETE_CHAR),
            &mut e,
            &mut set,
        )
        .unwrap();
    assert_eq!(text(&set, &e), "xbc");
    assert!(!e.in_undo_group());
    type_chars(&mut set, &mut e, "y");
    assert_eq!(text(&set, &e), "xybc");
    assert_eq!(steps(&mut set, &mut e), ["xbc", "xabc", "abc"]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_clone_keeps_the_open_typing_step() {
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    e.set_time(at(100));
    type_chars(&mut set, &mut e, "a");
    let mut copy = e.clone();
    assert_eq!(copy.undo_grouping(), UndoGrouping::Typing);
    assert!(copy.in_undo_group());
    type_chars(&mut set, &mut copy, "b");
    assert_eq!(steps(&mut set, &mut copy), [""]);

    // The copy keeps the time of the step's last edit too.
    let (mut set, mut e) = typing("", &[Selection::cursor(0)]);
    e.set_time(at(100));
    type_chars(&mut set, &mut e, "a");
    let mut copy = e.clone();
    copy.set_time(at(100) + PAUSE);
    type_chars(&mut set, &mut copy, "b");
    assert_eq!(steps(&mut set, &mut copy), ["a", ""]);
}
