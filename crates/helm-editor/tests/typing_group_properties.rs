//! T-112 acceptance tests: properties of typing groups. Grouping changes
//! only how edits undo, never the edits themselves, and the undo steps
//! start exactly where the "Typing groups" rules on `Editor` say.

use std::time::Duration;

use helm_editor::{Editor, Motion, Selection, Selections, UndoGrouping};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings, TextBuffer};
use proptest::prelude::*;

const PAUSE: Duration = Editor::TYPING_PAUSE;

/// One call on the editor.
#[derive(Clone, Debug)]
enum Op {
    Type(char),
    Enter,
    Backspace,
    Delete,
    DeleteTo(Motion),
    Move(Motion),
    /// Moves time on by this much.
    Wait(Duration),
    /// Sets the time, which may go backwards.
    SetTime(Duration),
    Begin,
    End,
}

/// What an edit is, for the oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Typing,
    Backward,
    Forward,
}

/// Text pieces. `lone_cr` adds a CR with no LF after it.
fn text_strategy(lone_cr: bool) -> impl Strategy<Value = String> {
    let mut pieces = vec![
        "ab", "x", " ", "  ", ".", "_", "é", "中", "😀", "\n", "\r\n",
    ];
    if lone_cr {
        pieces.push("\r");
    }
    prop::collection::vec(prop::sample::select(pieces), 0..12).prop_map(|p| p.concat())
}

fn wait_strategy() -> impl Strategy<Value = Duration> {
    prop_oneof![
        Just(Duration::ZERO),
        Just(Duration::from_millis(1)),
        Just(Duration::from_millis(500)),
        Just(PAUSE - Duration::from_nanos(1)),
        Just(PAUSE),
        Just(3 * PAUSE),
    ]
}

fn op_strategy(groups: bool) -> BoxedStrategy<Op> {
    let motion = prop::sample::select(vec![
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
    ]);
    let delete = prop::sample::select(vec![
        Motion::WordLeft,
        Motion::WordRight,
        Motion::LineStart,
        Motion::LineEnd,
    ]);
    let typed = prop::sample::select(vec!['a', 'b', ' ', '.', 'é', '中', '😀']);
    let ops = prop_oneof![
        8 => typed.prop_map(Op::Type),
        2 => Just(Op::Enter),
        4 => Just(Op::Backspace),
        3 => Just(Op::Delete),
        2 => delete.prop_map(Op::DeleteTo),
        3 => motion.prop_map(Op::Move),
        3 => wait_strategy().prop_map(Op::Wait),
        1 => (0u64..5_000).prop_map(|ms| Op::SetTime(Duration::from_millis(ms))),
        1 => Just(Op::End),
    ];
    if groups {
        prop_oneof![27 => ops, 1 => Just(Op::Begin)].boxed()
    } else {
        ops.boxed()
    }
}

/// Char boundaries of `text` that aren't inside a CRLF.
fn positions(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    (0..=text.len())
        .filter(|&i| text.is_char_boundary(i))
        .filter(|&i| !(i > 0 && i < bytes.len() && bytes[i - 1] == b'\r' && bytes[i] == b'\n'))
        .collect()
}

/// A text, up to three cursor positions in it (as indexes into its
/// `positions`), and a list of ops.
fn case(
    lone_cr: bool,
    groups: bool,
) -> impl Strategy<Value = (String, Vec<prop::sample::Index>, Vec<Op>)> {
    (
        text_strategy(lone_cr),
        prop::collection::vec(any::<prop::sample::Index>(), 1..=3),
        prop::collection::vec(op_strategy(groups), 0..40),
    )
}

fn editor(
    text: &str,
    cursors: &[prop::sample::Index],
    grouping: UndoGrouping,
) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    let at = positions(text);
    let selections = cursors
        .iter()
        .map(|index| Selection::cursor(at[index.index(at.len())]))
        .collect();
    editor.set_selections(Selections::new(selections, 0).expect("non-empty"));
    if grouping != UndoGrouping::Explicit {
        editor.set_undo_grouping(grouping);
    }
    (set, editor)
}

fn text(set: &BufferSet, editor: &Editor) -> String {
    set.get(editor.buffer()).expect("in the set").to_string()
}

fn lines(set: &BufferSet, editor: &Editor) -> usize {
    set.get(editor.buffer()).expect("in the set").len_lines()
}

/// Runs `op` on `editor`. Returns whether an edit applied a transaction
/// and, for a move, whether it changed the selections.
fn run(op: &Op, set: &mut BufferSet, editor: &mut Editor, now: &mut Duration) -> bool {
    match op {
        Op::Type(c) => editor.insert(set, &c.to_string()).unwrap().is_some(),
        Op::Enter => editor.newline(set).unwrap().is_some(),
        Op::Backspace => editor.delete_backward(set).unwrap().is_some(),
        Op::Delete => editor.delete_forward(set).unwrap().is_some(),
        Op::DeleteTo(motion) => editor.delete_to(set, *motion).unwrap().is_some(),
        Op::Move(motion) => editor.move_cursors(set, *motion).unwrap(),
        Op::Wait(d) => {
            *now += *d;
            editor.set_time(*now);
            false
        }
        Op::SetTime(t) => {
            *now = *t;
            editor.set_time(*now);
            false
        }
        Op::Begin => {
            editor.begin_undo_group();
            false
        }
        Op::End => {
            editor.end_undo_group();
            false
        }
    }
}

fn kind(op: &Op) -> Option<Kind> {
    match op {
        Op::Type(_) | Op::Enter => Some(Kind::Typing),
        Op::Backspace => Some(Kind::Backward),
        Op::Delete => Some(Kind::Forward),
        Op::DeleteTo(Motion::WordLeft | Motion::LineStart) => Some(Kind::Backward),
        Op::DeleteTo(_) => Some(Kind::Forward),
        _ => None,
    }
}

/// The oracle: runs `ops` on an editor under `Explicit` grouping, with no
/// explicit groups of its own, and returns the text just before each edit
/// that starts an undo step under `Typing` grouping, oldest first, and the
/// final text. The twin shows whether each edit applied a transaction,
/// whether each move changed the selections, and, through its line count,
/// whether a delete took a line break. The texts have no lone CR, so a
/// delete can never join a CR and a LF into a new line break.
fn oracle(text0: &str, cursors: &[prop::sample::Index], ops: &[Op]) -> (Vec<String>, String) {
    let (mut set, mut twin) = editor(text0, cursors, UndoGrouping::Explicit);
    let mut now = Duration::ZERO;
    let mut marks = Vec::new();
    // The open typing group: its last kind and the time of its last edit.
    let mut open: Option<(Kind, Duration)> = None;
    let mut moved = false;
    // An explicit group, and whether it has an edit yet.
    let mut explicit: Option<bool> = None;
    for op in ops {
        let before = text(&set, &twin);
        let lines_before = lines(&set, &twin);
        match op {
            Op::Begin => {
                open = None;
                explicit = Some(false);
                continue;
            }
            Op::End => {
                open = None;
                explicit = None;
                continue;
            }
            _ => {}
        }
        let changed = run(op, &mut set, &mut twin, &mut now);
        let Some(kind) = kind(op) else {
            if matches!(op, Op::Move(_)) && changed {
                moved = true;
            }
            continue;
        };
        if !changed {
            continue;
        }
        if let Some(has_edit) = explicit.as_mut() {
            if !*has_edit {
                marks.push(before);
                *has_edit = true;
            }
            continue;
        }
        let took_break = kind != Kind::Typing && lines(&set, &twin) < lines_before;
        let joins = match open {
            Some((last, at)) => {
                last == kind
                    && !moved
                    && !matches!(op, Op::Enter)
                    && !took_break
                    && now.checked_sub(at).is_none_or(|gap| gap < PAUSE)
            }
            None => false,
        };
        if !joins {
            marks.push(before);
        }
        open = Some((kind, now));
        moved = false;
    }
    (marks, text(&set, &twin))
}

/// Checks that undoing `editor` goes back through `marks`, newest first,
/// then finds nothing, and that redoing comes forward to `last`.
fn check_steps(
    set: &mut BufferSet,
    editor: &mut Editor,
    marks: &[String],
    last: &str,
) -> Result<(), TestCaseError> {
    prop_assert_eq!(text(set, editor), last);
    for mark in marks.iter().rev() {
        prop_assert!(editor.undo(set).unwrap().is_some());
        prop_assert_eq!(&text(set, editor), mark);
    }
    prop_assert_eq!(editor.undo(set).unwrap(), None);
    // Redoing step `i` brings back the text from before step `i + 1`, or
    // the final text after the last step.
    let after: Vec<&str> = (1..=marks.len())
        .map(|i| marks.get(i).map_or(last, String::as_str))
        .collect();
    for expected in after {
        prop_assert!(editor.redo(set).unwrap().is_some());
        prop_assert_eq!(text(set, editor), expected);
    }
    prop_assert_eq!(editor.redo(set).unwrap(), None);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Typing grouping changes only undo: the texts and selections after
    /// every op are the same as under explicit grouping.
    #[test]
    #[ignore = "T-112: not implemented yet"]
    fn grouping_never_changes_the_edits((text0, cursors, ops) in case(true, true)) {
        let (mut set_a, mut typing) = editor(&text0, &cursors, UndoGrouping::Typing);
        let (mut set_b, mut explicit) = editor(&text0, &cursors, UndoGrouping::Explicit);
        let (mut now_a, mut now_b) = (Duration::ZERO, Duration::ZERO);
        for op in &ops {
            let a = run(op, &mut set_a, &mut typing, &mut now_a);
            let b = run(op, &mut set_b, &mut explicit, &mut now_b);
            prop_assert_eq!(a, b, "{:?}", op);
            prop_assert_eq!(text(&set_a, &typing), text(&set_b, &explicit), "{:?}", op);
            prop_assert_eq!(typing.selections(), explicit.selections(), "{:?}", op);
        }
    }

    /// Undo steps start where the rules say, and undo and redo walk them.
    #[test]
    #[ignore = "T-112: not implemented yet"]
    fn steps_start_where_the_rules_say((text0, cursors, ops) in case(false, false)) {
        let (marks, last) = oracle(&text0, &cursors, &ops);
        let (mut set, mut e) = editor(&text0, &cursors, UndoGrouping::Typing);
        let mut now = Duration::ZERO;
        for op in &ops {
            run(op, &mut set, &mut e, &mut now);
        }
        check_steps(&mut set, &mut e, &marks, &last)?;
    }

    /// Under explicit grouping with no groups, every edit that applies a
    /// transaction is one step, whatever the time says.
    #[test]
    #[ignore = "T-112: not implemented yet"]
    fn explicit_grouping_makes_each_edit_a_step((text0, cursors, ops) in case(true, false)) {
        let (mut set, mut e) = editor(&text0, &cursors, UndoGrouping::Explicit);
        let mut now = Duration::ZERO;
        let mut marks = Vec::new();
        for op in ops.iter().filter(|op| !matches!(op, Op::End)) {
            let before = text(&set, &e);
            if run(op, &mut set, &mut e, &mut now) && kind(op).is_some() {
                marks.push(before);
            }
        }
        let last = text(&set, &e);
        check_steps(&mut set, &mut e, &marks, &last)?;
    }

    /// The same as `steps_start_where_the_rules_say`, with explicit groups
    /// opened and closed in between: each is one step, and typing groups
    /// start again after it.
    #[test]
    #[ignore = "T-112: not implemented yet"]
    fn explicit_groups_inside_typing_grouping((text0, cursors, ops) in case(false, true)) {
        let (marks, last) = oracle(&text0, &cursors, &ops);
        let (mut set, mut e) = editor(&text0, &cursors, UndoGrouping::Typing);
        let mut now = Duration::ZERO;
        for op in &ops {
            run(op, &mut set, &mut e, &mut now);
        }
        check_steps(&mut set, &mut e, &marks, &last)?;
    }
}
