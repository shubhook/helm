//! T-112 acceptance tests: VS Code's typing groups through the dispatcher.
//! `vscode::keymap()` carries `UndoGrouping::Typing`, and `dispatch` hands
//! the active keymap's grouping to the editor before each command.

use std::time::Duration;

use helm_editor::{
    Binding, CommandRegistry, Dispatcher, Editor, Key, KeyEvent, KeySequence, KeyState, Keymap,
    Keystroke, Mode, Modifiers, Resolution, Selection, Selections, Session, Typing, UndoGrouping,
    editor_commands, vim, vscode,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

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

/// The key events of `keys`, in `KeySequence` notation.
fn events(keys: &str) -> Vec<KeyEvent> {
    KeySequence::parse(keys)
        .expect("valid keys")
        .as_slice()
        .iter()
        .map(|&keystroke| KeyEvent::new(keystroke))
        .collect()
}

/// One key event per char of `text`, each typing its char.
fn chars(text: &str) -> Vec<KeyEvent> {
    text.chars()
        .map(|c| KeyEvent::new(Keystroke::new(Key::Char(c), Modifiers::NONE)))
        .collect()
}

/// One editor on a dispatcher that holds every command, so it can switch
/// between VS Code's and Vim's keymaps. Keys go through `dispatch_in` with
/// one `Session`, as in `helm-ui`.
struct Window {
    dispatcher: Dispatcher,
    state: KeyState,
    session: Session,
    set: BufferSet,
    editor: Editor,
}

impl Window {
    fn new(keymap: Keymap, text: &str, selections: &[Selection]) -> Self {
        let dispatcher = Dispatcher::new(CommandRegistry::with_vim_commands(), keymap);
        let state = dispatcher.key_state();
        let (set, editor) = setup(text, selections);
        Self {
            dispatcher,
            state,
            session: Session::new(),
            set,
            editor,
        }
    }

    fn send(&mut self, events: Vec<KeyEvent>) {
        for event in events {
            self.dispatcher
                .dispatch_in(
                    &mut self.state,
                    &event,
                    &[],
                    &mut self.editor,
                    &mut self.set,
                    &mut self.session,
                )
                .expect("the command runs");
        }
    }

    fn keys(&mut self, keys: &str) {
        self.send(events(keys));
    }

    fn type_text(&mut self, text: &str) {
        self.send(chars(text));
    }

    /// Switches keymaps the way a settings toggle does, without the
    /// `end_undo_group` call T-108 asks `helm-ui` to make.
    fn switch(&mut self, keymap: Keymap) {
        self.dispatcher.set_keymap(keymap);
        self.dispatcher.sync(&mut self.state);
    }

    fn text(&self) -> String {
        self.set
            .get(self.editor.buffer())
            .expect("in the set")
            .to_string()
    }

    fn selections(&self) -> &[Selection] {
        self.editor.selections().as_slice()
    }
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn each_keymap_carries_its_grouping() {
    assert_eq!(vscode::keymap().undo_grouping(), UndoGrouping::Typing);
    assert_eq!(vim::keymap().undo_grouping(), UndoGrouping::Explicit);
    let plain = Keymap::new("plain", vec![Mode::new("m", Typing::Insert)]).unwrap();
    assert_eq!(plain.undo_grouping(), UndoGrouping::Explicit);
    let typing = plain.with_undo_grouping(UndoGrouping::Typing);
    assert_eq!(typing.undo_grouping(), UndoGrouping::Typing);
    assert_eq!(typing.name(), "plain");
    // The VS Code keymap keeps its 39 bindings.
    assert_eq!(vscode::keymap().bindings().len(), 39);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn dispatch_gives_the_editor_the_keymaps_grouping_on_a_run() {
    let mut keymap = Keymap::new("chords", vec![Mode::new("m", Typing::Insert)])
        .unwrap()
        .with_undo_grouping(UndoGrouping::Typing);
    keymap
        .bind(Binding::run("cmd-k cmd-z", editor_commands::UNDO).unwrap())
        .unwrap();
    let dispatcher = Dispatcher::new(CommandRegistry::with_all_commands(), keymap);
    let mut state = dispatcher.key_state();
    let (mut set, mut editor) = setup("", &[Selection::cursor(0)]);
    assert_eq!(editor.undo_grouping(), UndoGrouping::Explicit);

    let mut press = |editor: &mut Editor, set: &mut BufferSet, keys: &str| {
        let event = &events(keys)[0];
        dispatcher
            .dispatch(&mut state, event, &[], editor, set)
            .unwrap()
    };
    // An unbound key and a pending chord run nothing, so nothing changes.
    assert_eq!(press(&mut editor, &mut set, "cmd-x"), Resolution::Unbound);
    assert_eq!(press(&mut editor, &mut set, "cmd-k"), Resolution::Pending);
    assert_eq!(editor.undo_grouping(), UndoGrouping::Explicit);
    assert!(matches!(
        press(&mut editor, &mut set, "cmd-z"),
        Resolution::Run(_)
    ));
    assert_eq!(editor.undo_grouping(), UndoGrouping::Typing);

    // `dispatch_in` does the same, with Vim's keymap giving `Explicit`.
    let vim = Dispatcher::new(CommandRegistry::with_vim_commands(), vim::keymap());
    let mut state = vim.key_state();
    let mut session = Session::new();
    vim.dispatch_in(
        &mut state,
        &chars("l")[0],
        &[],
        &mut editor,
        &mut set,
        &mut session,
    )
    .unwrap();
    assert_eq!(editor.undo_grouping(), UndoGrouping::Explicit);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn a_vscode_session_undoes_step_by_step() {
    let mut w = Window::new(vscode::keymap(), "", &[Selection::cursor(0)]);
    w.type_text("hello world");
    w.keys("enter");
    w.type_text("x");
    w.keys("left");
    w.type_text("y");
    w.keys("backspace");
    assert_eq!(w.text(), "hello world\nx");

    let undone = [
        ("hello world\nyx", 13),
        ("hello world\nx", 12),
        ("hello world", 11),
        ("", 0),
        ("", 0),
    ];
    for (text, cursor) in undone {
        w.keys("cmd-z");
        assert_eq!(w.text(), text);
        assert_eq!(w.selections(), [Selection::cursor(cursor)], "{text:?}");
    }
    let redone = [
        ("hello world", 11),
        ("hello world\nx", 13),
        ("hello world\nyx", 13),
        ("hello world\nx", 12),
    ];
    for (text, cursor) in redone {
        w.keys("shift-cmd-z");
        assert_eq!(w.text(), text);
        assert_eq!(w.selections(), [Selection::cursor(cursor)], "{text:?}");
    }
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn backspace_and_word_and_line_deletes_undo_together() {
    let mut w = Window::new(vscode::keymap(), "let foo = bar;", &[Selection::cursor(13)]);
    w.keys("backspace alt-backspace cmd-backspace");
    assert_eq!(w.text(), ";");
    w.keys("cmd-z");
    assert_eq!(w.text(), "let foo = bar;");
    assert_eq!(w.selections(), [Selection::cursor(13)]);
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn typing_after_undo_starts_a_new_step() {
    let mut w = Window::new(vscode::keymap(), "", &[Selection::cursor(0)]);
    w.type_text("ab");
    w.keys("cmd-z");
    w.type_text("cd");
    w.keys("cmd-z");
    assert_eq!(w.text(), "");
    w.keys("shift-cmd-z");
    assert_eq!(w.text(), "cd");
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn typing_at_three_cursors_undoes_in_one_step() {
    let mut w = Window::new(vscode::keymap(), "let a = a + a;", &[Selection::cursor(4)]);
    w.keys("cmd-d shift-cmd-l");
    w.type_text("total");
    assert_eq!(w.text(), "let total = total + total;");
    w.keys("cmd-z");
    assert_eq!(w.text(), "let a = a + a;");
    assert_eq!(
        w.selections(),
        [
            Selection::new(4, 5),
            Selection::new(8, 9),
            Selection::new(12, 13)
        ]
    );
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn set_time_between_keys_splits_a_run_at_a_pause() {
    let mut w = Window::new(vscode::keymap(), "", &[Selection::cursor(0)]);
    let pause = Editor::TYPING_PAUSE;
    let times = [
        Duration::ZERO,
        Duration::from_millis(100),
        Duration::from_millis(100) + pause,
    ];
    for (c, time) in "abc".chars().zip(times) {
        w.editor.set_time(time);
        w.type_text(&c.to_string());
    }
    w.keys("cmd-z");
    assert_eq!(w.text(), "ab");
    w.keys("cmd-z");
    assert_eq!(w.text(), "");
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn switching_to_vim_ends_the_typing_step() {
    let mut w = Window::new(vscode::keymap(), "abc", &[Selection::cursor(0)]);
    w.type_text("xy");
    assert!(w.editor.in_undo_group());
    w.switch(vim::keymap());
    // Vim's `x` deletes the char under the cursor, as its own step.
    w.type_text("x");
    assert_eq!(w.text(), "xybc");
    assert_eq!(w.editor.undo_grouping(), UndoGrouping::Explicit);
    w.type_text("u");
    assert_eq!(w.text(), "xyabc");
    w.type_text("u");
    assert_eq!(w.text(), "abc");
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn switching_to_vscode_mid_insert_ends_the_vim_group_and_its_record() {
    let mut w = Window::new(vim::keymap(), "abc", &[Selection::cursor(0)]);
    w.type_text("ixy");
    assert_eq!(w.state.mode().as_str(), vim::INSERT);
    assert!(w.session.last_change().is_none());
    w.switch(vscode::keymap());
    w.type_text("z");
    // The switch ended the insert session before `z` ran, so `.` has the
    // whole session and `z` isn't part of it.
    let change = w.session.last_change().expect("the insert is recorded");
    assert_eq!(change.len(), 3);
    assert_eq!(change[0].command.as_str(), "vim.insert");
    w.type_text("w");
    assert_eq!(w.text(), "xyzwabc");
    w.switch(vim::keymap());
    assert_eq!(w.state.mode().as_str(), vim::NORMAL);
    w.type_text("u");
    assert_eq!(w.text(), "xyabc");
    w.type_text("u");
    assert_eq!(w.text(), "abc");
}

#[test]
#[ignore = "T-112: not implemented yet"]
fn with_helm_uis_close_on_each_switch_there_are_still_three_steps() {
    let mut w = Window::new(vim::keymap(), "abc", &[Selection::cursor(0)]);
    w.type_text("iy");
    w.switch(vscode::keymap());
    w.editor.end_undo_group();
    w.type_text("zw");
    w.switch(vim::keymap());
    w.editor.end_undo_group();
    w.type_text("x");
    assert_eq!(w.text(), "yzwbc");
    for expected in ["yzwabc", "yabc", "abc"] {
        w.type_text("u");
        assert_eq!(w.text(), expected);
    }
}
