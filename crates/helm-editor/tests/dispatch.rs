//! T-106 acceptance tests: typed keys dispatched into an `Editor`, checked
//! against the buffer, with a VS Code-like keymap, a Vim-like one, and a
//! switch between them mid-session (ED-6).

use helm_editor::{
    Binding, CommandError, CommandId, CommandRegistry, Dispatcher, Editor, Invocation, KeyEvent,
    KeySequence, KeyState, Keymap, Keystroke, Mode, Resolution, Selection, Selections, Typing,
    editor_commands as ids,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// A binding from `keys` to running `command`.
fn run(keys: &str, command: &str) -> Binding {
    Binding::run(keys, command).expect("valid keys")
}

/// A VS Code-like sample: one mode where typing inserts, and a few of the
/// bindings T-107 will add.
fn vscode_like() -> Keymap {
    let mut keymap =
        Keymap::new("vscode", vec![Mode::new("default", Typing::Insert)]).expect("one mode");
    for binding in [
        run("enter", ids::NEWLINE),
        run("backspace", ids::DELETE_BACKWARD),
        run("delete", ids::DELETE_FORWARD),
        run("cmd-z", ids::UNDO),
        run("shift-cmd-z", ids::REDO),
        run("cmd-d", ids::ADD_NEXT_MATCH),
        run("shift-cmd-l", ids::SELECT_ALL_MATCHES),
        run("alt-cmd-up", ids::ADD_CURSOR_ABOVE),
        run("alt-cmd-down", ids::ADD_CURSOR_BELOW),
        run("cmd-k cmd-z", ids::UNDO),
    ] {
        keymap.bind(binding).expect("valid binding");
    }
    keymap
}

/// A Vim-like sample: normal mode ignores typing, insert mode types.
fn vim_like() -> Keymap {
    let mut keymap = Keymap::new(
        "vim",
        vec![
            Mode::new("normal", Typing::Ignore),
            Mode::new("insert", Typing::Insert),
        ],
    )
    .expect("two modes");
    for binding in [
        Binding::switch_mode("i", "insert")
            .unwrap()
            .in_mode("normal"),
        Binding::switch_mode("escape", "normal")
            .unwrap()
            .in_mode("insert"),
        run("x", ids::DELETE_FORWARD).in_mode("normal"),
        run("X", ids::DELETE_BACKWARD).in_mode("normal"),
        run("u", ids::UNDO).in_mode("normal"),
        run("ctrl-r", ids::REDO).in_mode("normal"),
        run("o", ids::NEWLINE).in_mode("normal").then_mode("insert"),
        run("backspace", ids::DELETE_BACKWARD).in_mode("insert"),
        run("enter", ids::NEWLINE).in_mode("insert"),
    ] {
        keymap.bind(binding).expect("valid binding");
    }
    keymap
}

/// One editor with its buffer set and key state.
struct Session {
    dispatcher: Dispatcher,
    state: KeyState,
    set: BufferSet,
    editor: Editor,
}

impl Session {
    /// An editor on a buffer holding `text`, with `selections` (the first
    /// is the primary), keys going through `keymap` and the editor
    /// commands.
    fn new(keymap: Keymap, text: &str, selections: &[Selection]) -> Self {
        let dispatcher = Dispatcher::new(CommandRegistry::with_editor_commands(), keymap);
        let state = dispatcher.key_state();
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text));
        let format = FileFormat {
            bom: false,
            line_endings: LineEndings::count(text),
        };
        let mut editor = Editor::new(id, format);
        editor.set_selections(Selections::new(selections.to_vec(), 0).expect("non-empty"));
        Self {
            dispatcher,
            state,
            set,
            editor,
        }
    }

    /// Dispatches one event, which must not fail.
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

    /// Dispatches one key event per char of `text`, as typing it would.
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

    /// The buffer's text.
    fn text(&self) -> String {
        self.set
            .get(self.editor.buffer())
            .expect("buffer is in the set")
            .to_string()
    }
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn typing_goes_into_the_buffer() {
    let mut s = Session::new(vscode_like(), "", &[Selection::cursor(0)]);
    s.type_text("hello world");
    assert_eq!(s.text(), "hello world");
    s.keys("enter");
    s.type_text("é中😀");
    assert_eq!(s.text(), "hello world\né中😀");
    assert_eq!(s.editor.selections().as_slice(), &[Selection::cursor(21)]);
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn bound_keys_edit_and_undo() {
    let mut s = Session::new(vscode_like(), "ab\r\ncd", &[Selection::cursor(2)]);
    s.keys("enter");
    assert_eq!(s.text(), "ab\r\n\r\ncd");
    s.keys("backspace backspace");
    assert_eq!(s.text(), "a\r\ncd");
    s.keys("delete");
    assert_eq!(s.text(), "acd");
    s.keys("cmd-z");
    assert_eq!(s.text(), "a\r\ncd");
    s.keys("cmd-k cmd-z");
    assert_eq!(s.text(), "ab\r\ncd");
    s.keys("shift-cmd-z");
    assert_eq!(s.text(), "a\r\ncd");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn typing_at_every_cursor() {
    let mut s = Session::new(vscode_like(), "let a = a + a;", &[Selection::cursor(4)]);
    // cmd-d selects the word, then shift-cmd-l every match of it.
    s.keys("cmd-d shift-cmd-l");
    assert_eq!(s.editor.selections().as_slice().len(), 3);
    s.type_text("total");
    assert_eq!(s.text(), "let total = total + total;");
    s.keys("cmd-z");
    // Each key is one transaction, so one undo takes back the last letter.
    assert_eq!(s.text(), "let tota = tota + tota;");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn adding_cursors_with_keys() {
    let mut s = Session::new(vscode_like(), "ab\nab\nab", &[Selection::cursor(4)]);
    s.keys("alt-cmd-up alt-cmd-down");
    s.type_text("-");
    assert_eq!(s.text(), "a-b\na-b\na-b");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn a_broken_chord_types_nothing() {
    let mut s = Session::new(vscode_like(), "", &[Selection::cursor(0)]);
    s.keys("cmd-k x");
    assert_eq!(s.text(), "");
    s.keys("x");
    assert_eq!(s.text(), "x");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn option_text_is_typed() {
    let mut s = Session::new(vscode_like(), "", &[Selection::cursor(0)]);
    let option_a = KeyEvent::with_text(Keystroke::parse("alt-a").unwrap(), "å");
    assert_eq!(s.event(&option_a), Resolution::Run(Invocation::insert("å")));
    assert_eq!(s.text(), "å");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn vim_like_modes_edit_the_buffer() {
    let mut s = Session::new(vim_like(), "abc", &[Selection::cursor(0)]);
    // Normal mode: `x` deletes, other letters do nothing.
    s.keys("x z q");
    assert_eq!(s.text(), "bc");
    // Insert mode types, escape goes back.
    s.keys("i");
    s.type_text("hey ");
    s.keys("escape");
    assert_eq!(s.text(), "hey bc");
    assert_eq!(s.state.mode().as_str(), "normal");
    s.type_text("q");
    assert_eq!(s.text(), "hey bc");
    // `X` deletes backward, `u` undoes, ctrl-r redoes.
    s.keys("X");
    assert_eq!(s.text(), "heybc");
    s.keys("u");
    assert_eq!(s.text(), "hey bc");
    s.keys("ctrl-r");
    assert_eq!(s.text(), "heybc");
    // `o` runs newline and enters insert mode.
    s.keys("o");
    s.type_text("x");
    assert_eq!(s.text(), "hey\nxbc");
    assert_eq!(s.state.mode().as_str(), "insert");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn switching_keymaps_mid_session() {
    let mut s = Session::new(vscode_like(), "", &[Selection::cursor(0)]);
    s.type_text("abc");
    // Half a chord, then the switch: it is dropped.
    s.keys("cmd-k");

    s.dispatcher.set_keymap(vim_like());
    // Normal mode: `x` now deletes instead of typing.
    s.keys("x");
    assert_eq!(s.text(), "abc");
    s.editor
        .set_selections(Selections::single(Selection::cursor(1)));
    s.keys("x");
    assert_eq!(s.text(), "ac");
    s.keys("i");
    s.type_text("b");
    assert_eq!(s.text(), "abc");

    // And back: typing inserts again, from the first key.
    s.dispatcher.set_keymap(vscode_like());
    s.type_text("x");
    assert_eq!(s.text(), "abxc");
    assert_eq!(s.state.mode().as_str(), "default");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn two_editors_share_one_dispatcher() {
    let mut s = Session::new(vim_like(), "one", &[Selection::cursor(0)]);
    let id = s.set.add(Buffer::from("two"));
    let mut other = Editor::new(id, FileFormat::default());
    let mut other_state = s.dispatcher.key_state();

    // The first editor goes to insert mode; the second stays in normal.
    s.keys("i");
    let x = KeyEvent::parse("x").unwrap();
    s.dispatcher
        .dispatch(&mut other_state, &x, &[], &mut other, &mut s.set)
        .unwrap();
    assert_eq!(s.set.get(id).unwrap().to_string(), "wo");
    s.type_text("x");
    assert_eq!(s.text(), "xone");

    // One switch reaches both.
    s.dispatcher.set_keymap(vscode_like());
    s.dispatcher
        .dispatch(&mut other_state, &x, &[], &mut other, &mut s.set)
        .unwrap();
    assert_eq!(s.set.get(id).unwrap().to_string(), "xwo");
    s.type_text("y");
    assert_eq!(s.text(), "xyone");
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn an_unknown_command_is_an_error_and_edits_nothing() {
    let mut keymap = vscode_like();
    keymap.bind(run("cmd-s", "workspace.save")).unwrap();
    let mut s = Session::new(keymap, "ab", &[Selection::cursor(1)]);
    let save = KeyEvent::parse("cmd-s").unwrap();
    assert_eq!(
        s.dispatcher
            .dispatch(&mut s.state, &save, &[], &mut s.editor, &mut s.set),
        Err(CommandError::UnknownCommand {
            id: CommandId::from("workspace.save")
        })
    );
    assert_eq!(s.text(), "ab");
    // Resolving alone hands the invocation to whoever owns the command.
    assert_eq!(
        s.dispatcher.resolve(&mut s.state, &save, &[]),
        Resolution::Run(Invocation::new("workspace.save"))
    );
}

#[test]
#[ignore = "T-106: not implemented yet"]
fn dispatch_returns_the_resolution() {
    let mut s = Session::new(vscode_like(), "", &[Selection::cursor(0)]);
    let parse = |text: &str| KeyEvent::parse(text).unwrap();
    assert_eq!(s.event(&parse("cmd-k")), Resolution::Pending);
    assert_eq!(s.event(&parse("escape")), Resolution::Cancelled);
    assert_eq!(s.event(&parse("ctrl-q")), Resolution::Unbound);
    assert_eq!(
        s.event(&parse("cmd-z")),
        Resolution::Run(Invocation::new(ids::UNDO))
    );
}
