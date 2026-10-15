//! T-107 acceptance tests: the cursor command ids, the registry with every
//! command, VS Code's keymap as data, and typing through it into an
//! `Editor` (ED-4).

use helm_editor::{
    Action, CommandRegistry, Dispatcher, Editor, Invocation, KeyEvent, KeySequence, KeyState,
    Keystroke, Mode, Motion, Resolution, Selection, Selections, Typing, cursor_commands,
    editor_commands, vscode,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// VS Code's bindings as the spec lists them: keys and command id.
const BINDINGS: [(&str, &str); 39] = [
    ("enter", "editor.newline"),
    ("backspace", "editor.delete_backward"),
    ("delete", "editor.delete_forward"),
    ("cmd-z", "editor.undo"),
    ("shift-cmd-z", "editor.redo"),
    ("cmd-d", "editor.add_next_match"),
    ("shift-cmd-l", "editor.select_all_matches"),
    ("alt-cmd-up", "editor.add_cursor_above"),
    ("alt-cmd-down", "editor.add_cursor_below"),
    ("left", "cursor.left"),
    ("right", "cursor.right"),
    ("up", "cursor.up"),
    ("down", "cursor.down"),
    ("alt-left", "cursor.word_left"),
    ("alt-right", "cursor.word_right"),
    ("home", "cursor.line_start"),
    ("cmd-left", "cursor.line_start"),
    ("end", "cursor.line_end"),
    ("cmd-right", "cursor.line_end"),
    ("cmd-up", "cursor.text_start"),
    ("cmd-down", "cursor.text_end"),
    ("shift-left", "cursor.select_left"),
    ("shift-right", "cursor.select_right"),
    ("shift-up", "cursor.select_up"),
    ("shift-down", "cursor.select_down"),
    ("alt-shift-left", "cursor.select_word_left"),
    ("alt-shift-right", "cursor.select_word_right"),
    ("shift-home", "cursor.select_line_start"),
    ("shift-cmd-left", "cursor.select_line_start"),
    ("shift-end", "cursor.select_line_end"),
    ("shift-cmd-right", "cursor.select_line_end"),
    ("shift-cmd-up", "cursor.select_text_start"),
    ("shift-cmd-down", "cursor.select_text_end"),
    ("cmd-a", "cursor.select_all"),
    ("escape", "cursor.collapse"),
    ("alt-backspace", "cursor.delete_word_left"),
    ("alt-delete", "cursor.delete_word_right"),
    ("cmd-backspace", "cursor.delete_line_start"),
    ("cmd-delete", "cursor.delete_line_end"),
];

/// An editor on `text` with `selections`, the first primary.
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
    set.get(editor.buffer())
        .expect("buffer is in the set")
        .to_string()
}

/// One editor typing through VS Code's keymap and every command.
struct Session {
    dispatcher: Dispatcher,
    state: KeyState,
    set: BufferSet,
    editor: Editor,
}

impl Session {
    fn new(text: &str, selections: &[Selection]) -> Self {
        let dispatcher = Dispatcher::new(CommandRegistry::with_all_commands(), vscode::keymap());
        let state = dispatcher.key_state();
        let (set, editor) = setup(text, selections);
        Self {
            dispatcher,
            state,
            set,
            editor,
        }
    }

    fn event(&mut self, event: &KeyEvent) -> Resolution {
        self.dispatcher
            .dispatch(&mut self.state, event, &[], &mut self.editor, &mut self.set)
            .expect("the command runs")
    }

    /// Dispatches each keystroke of `keys`, in `KeySequence` notation.
    fn keys(&mut self, keys: &str) {
        for &keystroke in KeySequence::parse(keys).expect("valid keys").as_slice() {
            self.event(&KeyEvent::new(keystroke));
        }
    }

    /// Dispatches one key event per char of `text`.
    fn type_text(&mut self, text: &str) {
        for c in text.chars() {
            let keystroke = if c == ' ' {
                Keystroke::parse("space").unwrap()
            } else {
                Keystroke::parse(&c.to_string()).unwrap()
            };
            self.event(&KeyEvent::new(keystroke));
        }
    }

    fn text(&self) -> String {
        text(&self.set, &self.editor)
    }

    fn selections(&self) -> &[Selection] {
        self.editor.selections().as_slice()
    }
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn cursor_command_ids_are_the_documented_ones() {
    assert_eq!(
        cursor_commands::ALL,
        [
            "cursor.collapse",
            "cursor.delete_line_end",
            "cursor.delete_line_start",
            "cursor.delete_word_left",
            "cursor.delete_word_right",
            "cursor.down",
            "cursor.left",
            "cursor.line_end",
            "cursor.line_start",
            "cursor.right",
            "cursor.select_all",
            "cursor.select_down",
            "cursor.select_left",
            "cursor.select_line_end",
            "cursor.select_line_start",
            "cursor.select_right",
            "cursor.select_text_end",
            "cursor.select_text_start",
            "cursor.select_up",
            "cursor.select_word_left",
            "cursor.select_word_right",
            "cursor.text_end",
            "cursor.text_start",
            "cursor.up",
            "cursor.word_left",
            "cursor.word_right",
        ]
    );
    // Calls into the registry so the test runs against the implementation.
    assert!(
        CommandRegistry::with_all_commands()
            .get("cursor.left")
            .is_some()
    );
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn with_all_commands_registers_both_lists_sorted_with_titles() {
    let registry = CommandRegistry::with_all_commands();
    let ids: Vec<&str> = registry.commands().map(|c| c.id.as_str()).collect();
    let mut expected: Vec<&str> = editor_commands::ALL
        .iter()
        .chain(cursor_commands::ALL.iter())
        .copied()
        .collect();
    expected.sort_unstable();
    assert_eq!(ids, expected);
    assert_eq!(ids.len(), 36);
    for id in expected {
        let command = registry.get(id).expect("registered");
        assert!(!command.title.is_empty(), "{id} has a title");
    }
    // `with_editor_commands` is unchanged.
    let editor_registry = CommandRegistry::with_editor_commands();
    let editor_only: Vec<&str> = editor_registry.commands().map(|c| c.id.as_str()).collect();
    assert_eq!(editor_only, editor_commands::ALL);
}

/// Runs `id` through a registry with every command on a fresh editor, and
/// `direct` on another fresh editor, and checks both end the same.
fn same_as_direct(id: &str, direct: fn(&mut Editor, &mut BufferSet)) {
    let text = "foo.bar  baz\r\n  qux é中😀\nend";
    let selections = [
        Selection::cursor(5),
        Selection::new(16, 19),
        Selection::cursor(27),
    ];
    let registry = CommandRegistry::with_all_commands();
    let (mut set, mut editor) = setup(text, &selections);
    registry
        .run(&Invocation::new(id), &mut editor, &mut set)
        .expect("the command runs");
    let (mut direct_set, mut direct_editor) = setup(text, &selections);
    direct(&mut direct_editor, &mut direct_set);
    assert_eq!(
        text_of(&set, &editor),
        text_of(&direct_set, &direct_editor),
        "{id} text"
    );
    assert_eq!(
        editor.selections(),
        direct_editor.selections(),
        "{id} selections"
    );
}

fn text_of(set: &BufferSet, editor: &Editor) -> String {
    text(set, editor)
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn each_cursor_command_runs_its_editor_method() {
    let motions = [
        ("left", Motion::Left),
        ("right", Motion::Right),
        ("up", Motion::Up),
        ("down", Motion::Down),
        ("word_left", Motion::WordLeft),
        ("word_right", Motion::WordRight),
        ("line_start", Motion::LineStart),
        ("line_end", Motion::LineEnd),
        ("text_start", Motion::TextStart),
        ("text_end", Motion::TextEnd),
    ];
    // Function pointers can't capture the motion, so each case names it.
    macro_rules! check {
        ($prefix:literal, $name:literal, $method:ident, $motion:expr) => {
            same_as_direct(concat!("cursor.", $prefix, $name), |e, s| {
                e.$method(s, $motion).expect("buffer is in the set");
            })
        };
    }
    check!("", "left", move_cursors, Motion::Left);
    check!("", "right", move_cursors, Motion::Right);
    check!("", "up", move_cursors, Motion::Up);
    check!("", "down", move_cursors, Motion::Down);
    check!("", "word_left", move_cursors, Motion::WordLeft);
    check!("", "word_right", move_cursors, Motion::WordRight);
    check!("", "line_start", move_cursors, Motion::LineStart);
    check!("", "line_end", move_cursors, Motion::LineEnd);
    check!("", "text_start", move_cursors, Motion::TextStart);
    check!("", "text_end", move_cursors, Motion::TextEnd);
    check!("select_", "left", extend_selections, Motion::Left);
    check!("select_", "right", extend_selections, Motion::Right);
    check!("select_", "up", extend_selections, Motion::Up);
    check!("select_", "down", extend_selections, Motion::Down);
    check!("select_", "word_left", extend_selections, Motion::WordLeft);
    check!(
        "select_",
        "word_right",
        extend_selections,
        Motion::WordRight
    );
    check!(
        "select_",
        "line_start",
        extend_selections,
        Motion::LineStart
    );
    check!("select_", "line_end", extend_selections, Motion::LineEnd);
    check!(
        "select_",
        "text_start",
        extend_selections,
        Motion::TextStart
    );
    check!("select_", "text_end", extend_selections, Motion::TextEnd);
    check!("delete_", "word_left", delete_to, Motion::WordLeft);
    check!("delete_", "word_right", delete_to, Motion::WordRight);
    check!("delete_", "line_start", delete_to, Motion::LineStart);
    check!("delete_", "line_end", delete_to, Motion::LineEnd);
    same_as_direct(cursor_commands::SELECT_ALL, |e, s| {
        e.select_all(s).expect("buffer is in the set");
    });
    same_as_direct(cursor_commands::COLLAPSE, |e, s| {
        e.collapse_selections(s).expect("buffer is in the set");
    });
    // Every motion name above is one of the cursor command ids.
    for (name, _) in motions {
        assert!(cursor_commands::ALL.contains(&format!("cursor.{name}").as_str()));
    }
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn the_keymap_is_one_typing_mode() {
    let keymap = vscode::keymap();
    assert_eq!(vscode::NAME, "vscode");
    assert_eq!(vscode::MODE, "default");
    assert_eq!(keymap.name(), "vscode");
    assert_eq!(keymap.modes(), &[Mode::new("default", Typing::Insert)]);
    assert_eq!(keymap.initial_mode().as_str(), "default");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn the_keymap_holds_exactly_the_documented_bindings() {
    let keymap = vscode::keymap();
    let mut actual: Vec<(String, String)> = keymap
        .bindings()
        .iter()
        .map(|binding| {
            assert_eq!(binding.mode, None, "{}", binding.keys);
            assert_eq!(binding.context, None, "{}", binding.keys);
            assert_eq!(binding.then_mode, None, "{}", binding.keys);
            let Some(Action::Run(command)) = &binding.action else {
                panic!("{} should run a command", binding.keys);
            };
            (binding.keys.to_string(), command.as_str().to_owned())
        })
        .collect();
    let mut expected: Vec<(String, String)> = BINDINGS
        .iter()
        .map(|&(keys, command)| {
            let keys = KeySequence::parse(keys).expect("valid keys").to_string();
            (keys, command.to_owned())
        })
        .collect();
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn every_bound_command_is_registered() {
    let registry = CommandRegistry::with_all_commands();
    for binding in vscode::keymap().bindings() {
        let Some(Action::Run(command)) = &binding.action else {
            panic!("{} should run a command", binding.keys);
        };
        assert!(
            registry.get(command.as_str()).is_some(),
            "{} runs {command}, which isn't registered",
            binding.keys
        );
    }
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn each_binding_resolves_to_its_command() {
    let dispatcher = Dispatcher::new(CommandRegistry::with_all_commands(), vscode::keymap());
    for (keys, command) in BINDINGS {
        let mut state = dispatcher.key_state();
        let event = KeyEvent::parse(keys).expect("valid key");
        assert_eq!(
            dispatcher.resolve(&mut state, &event, &[]),
            Resolution::Run(Invocation::new(command)),
            "{keys}"
        );
        assert!(state.pending().is_empty());
    }
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn typing_and_arrows() {
    let mut s = Session::new("", &[Selection::cursor(0)]);
    s.type_text("abc");
    s.keys("left left");
    s.type_text("X");
    assert_eq!(s.text(), "aXbc");
    s.keys("end");
    s.type_text("!");
    assert_eq!(s.text(), "aXbc!");
    s.keys("cmd-left");
    s.type_text(">");
    assert_eq!(s.text(), ">aXbc!");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn up_and_down_keep_the_goal_through_keys() {
    let mut s = Session::new("abcdef\nab\nabcdef", &[Selection::cursor(5)]);
    s.keys("down down");
    s.type_text("X");
    assert_eq!(s.text(), "abcdef\nab\nabcdeXf");
    s.keys("cmd-up");
    s.type_text("^");
    assert_eq!(s.text(), "^abcdef\nab\nabcdeXf");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn shift_selects_and_typing_replaces_the_selection() {
    let mut s = Session::new("say hello", &[Selection::cursor(9)]);
    s.keys("alt-shift-left");
    assert_eq!(s.selections(), &[Selection::new(9, 4)]);
    s.type_text("bye");
    assert_eq!(s.text(), "say bye");
    s.keys("shift-home");
    s.keys("backspace");
    assert_eq!(s.text(), "");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn select_all_then_delete_and_undo() {
    let mut s = Session::new("one\r\ntwo", &[Selection::cursor(2)]);
    s.keys("cmd-a");
    assert_eq!(s.selections(), &[Selection::new(0, 8)]);
    s.keys("delete");
    assert_eq!(s.text(), "");
    s.keys("cmd-z");
    assert_eq!(s.text(), "one\r\ntwo");
    assert_eq!(s.selections(), &[Selection::new(0, 8)]);
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn word_and_line_deletes() {
    let mut s = Session::new("let foo = bar;", &[Selection::cursor(13)]);
    s.keys("alt-backspace");
    assert_eq!(s.text(), "let foo = ;");
    s.keys("cmd-backspace");
    assert_eq!(s.text(), ";");
    s.keys("cmd-z cmd-z");
    assert_eq!(s.text(), "let foo = bar;");
    s.keys("cmd-left alt-delete");
    assert_eq!(s.text(), " foo = bar;");
    s.keys("cmd-delete");
    assert_eq!(s.text(), "");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn escape_drops_extra_cursors_then_the_selection() {
    let mut s = Session::new("ab ab ab", &[Selection::cursor(1)]);
    s.keys("cmd-d cmd-d cmd-d");
    assert_eq!(s.selections().len(), 3);
    s.type_text("X");
    assert_eq!(s.text(), "X X X");
    s.keys("shift-left escape");
    assert_eq!(s.selections().len(), 1);
    assert!(!s.selections()[0].is_empty());
    s.keys("escape");
    assert!(s.selections()[0].is_empty());
    s.type_text("Y");
    assert_eq!(s.text(), "X X YX");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn multi_cursor_movement_through_keys() {
    let mut s = Session::new("a1\nb2\nc3", &[Selection::cursor(0)]);
    s.keys("alt-cmd-down alt-cmd-down");
    assert_eq!(s.selections().len(), 3);
    s.keys("end");
    s.type_text(";");
    assert_eq!(s.text(), "a1;\nb2;\nc3;");
    s.keys("alt-left");
    s.type_text("_");
    assert_eq!(s.text(), "a1_;\nb2_;\nc3_;");
}

#[test]
#[ignore = "T-107: not implemented yet"]
fn option_text_still_types() {
    let mut s = Session::new("", &[Selection::cursor(0)]);
    let alt_e = Keystroke::parse("alt-e").expect("valid key");
    assert_eq!(
        s.event(&KeyEvent::with_text(alt_e, "´")),
        Resolution::Run(Invocation::insert("´"))
    );
    assert_eq!(s.text(), "´");
}
