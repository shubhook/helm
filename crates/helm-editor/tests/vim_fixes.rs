//! T-111 acceptance tests: the correctness findings of PR #38's council
//! review that reproduce. Each test failed on main at `11010ce` and
//! passes with T-111's fixes.

use helm_editor::{
    CommandError, CommandId, CommandRegistry, Dispatcher, Editor, EditorError, Invocation, Key,
    KeyEvent, KeyState, Keystroke, Modifiers, Selection, Selections, vim, vim_commands,
};
use helm_text::{Buffer, BufferId, BufferSet, Edit, FileFormat, LineEndings, Transaction};

/// An editor in Vim's keymap, with its own buffer set and key state.
struct Vim {
    dispatcher: Dispatcher,
    state: KeyState,
    set: BufferSet,
    editor: Editor,
}

impl Vim {
    fn new(text: &str, selections: &[Selection]) -> Self {
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text));
        let format = FileFormat {
            bom: false,
            line_endings: LineEndings::count(text),
        };
        Self::on(set, id, format, selections)
    }

    fn on(set: BufferSet, id: BufferId, format: FileFormat, selections: &[Selection]) -> Self {
        let mut editor = Editor::new(id, format);
        editor.set_selections(Selections::new(selections.to_vec(), 0).expect("non-empty"));
        let dispatcher = Dispatcher::new(CommandRegistry::with_vim_commands(), vim::keymap());
        let state = dispatcher.key_state();
        Self {
            dispatcher,
            state,
            set,
            editor,
        }
    }

    /// Dispatches each char of `keys` as a key, `\u{1b}` as Escape.
    fn keys(&mut self, keys: &str) {
        for c in keys.chars() {
            let key = if c == '\u{1b}' {
                Keystroke::new(Key::Escape, Modifiers::NONE)
            } else {
                Keystroke::new(Key::Char(c), Modifiers::NONE)
            };
            self.dispatcher
                .dispatch(
                    &mut self.state,
                    &KeyEvent::new(key),
                    &[],
                    &mut self.editor,
                    &mut self.set,
                )
                .expect("the command runs");
        }
    }

    /// Runs the Vim command `id` on this editor and `set`.
    fn run_in(&mut self, id: &str, set: &mut BufferSet) -> Result<(), CommandError> {
        CommandRegistry::with_vim_commands().run(
            &Invocation::new(CommandId::from(id)),
            &mut self.editor,
            set,
        )
    }

    /// Runs the Vim command `id` on this editor and its own set.
    fn run(&mut self, id: &str) -> Result<(), CommandError> {
        let mut set = std::mem::take(&mut self.set);
        let result = self.run_in(id, &mut set);
        self.set = set;
        result
    }

    fn text(&self) -> String {
        self.set
            .get(self.editor.buffer())
            .expect("in the set")
            .to_string()
    }

    fn selections(&self) -> Vec<Selection> {
        self.editor.selections().as_slice().to_vec()
    }
}

// ---- Finding 1: Escape in insert mode with an unknown buffer ----------

/// `vim.normal` that errors because the editor's buffer isn't in the set
/// changes nothing, as `CommandError` says: the undo group stays open and
/// the cursors stay where they were.
#[test]
fn leaving_insert_mode_with_an_unknown_buffer_keeps_the_group_open() {
    let mut v = Vim::new("abc", &[Selection::cursor(1)]);
    v.keys("ix");
    assert!(v.editor.in_undo_group());
    let before = v.selections();
    let missing = v.editor.buffer();

    let mut other = BufferSet::new();
    assert_eq!(
        v.run_in(vim_commands::NORMAL, &mut other),
        Err(CommandError::Editor(EditorError::UnknownBuffer {
            buffer: missing
        }))
    );
    assert!(v.editor.in_undo_group(), "the error closed the undo group");
    assert_eq!(v.selections(), before);

    // The group is still the one `i` opened, so the next char folds into
    // it and one `u` takes both away.
    v.keys("y\u{1b}u");
    assert_eq!(v.text(), "abc");
}

// ---- Finding 2: a `u` or `ctrl-r` whose first step is blocked ---------

/// A buffer set where undoing from buffer `a` ("abcd\nx\nabcd" plus a
/// "!" at its end) is blocked by a later transaction in buffer `b`.
fn blocked() -> (BufferSet, BufferId) {
    let mut set = BufferSet::new();
    let a = set.add(Buffer::from("abcd\nx\nabcd"));
    let b = set.add(Buffer::from("b"));
    let mut both = Transaction::new();
    both.push(Edit::insert(a, 11, "!"));
    both.push(Edit::insert(b, 1, "1"));
    set.apply(both).expect("valid edits");
    let mut later = Transaction::new();
    later.push(Edit::insert(b, 0, "2"));
    set.apply(later).expect("valid edit");
    (set, a)
}

/// `vim.undo` whose first step is blocked errors and changes nothing: the
/// selections stay as they were, unsettled, and the goal column `j` set
/// is still there for the next `j`.
#[test]
fn a_blocked_first_undo_step_changes_nothing() {
    let (set, a) = blocked();
    let mut v = Vim::on(set, a, FileFormat::default(), &[Selection::cursor(3)]);

    // `j` from column 3 sets the goal and lands on the short line's `x`.
    v.keys("j");
    assert_eq!(v.selections(), [Selection::cursor(5)]);
    let blocked = v.run(vim_commands::UNDO);
    assert!(
        matches!(
            blocked,
            Err(CommandError::Editor(EditorError::Blocked { .. }))
        ),
        "{blocked:?}"
    );
    assert_eq!(v.selections(), [Selection::cursor(5)]);
    // The goal is still 3, so `j` goes to column 3 of the last line.
    v.keys("j");
    assert_eq!(v.selections(), [Selection::cursor(10)]);

    // A selection that isn't a settled cursor stays as it is too.
    v.editor
        .set_selections(Selections::single(Selection::new(0, 4)));
    assert!(v.run(vim_commands::UNDO).is_err());
    assert_eq!(v.selections(), [Selection::new(0, 4)]);
    assert!(
        v.run(vim_commands::REDO).is_ok(),
        "nothing to redo is not an error"
    );
    assert_eq!(v.text(), "abcd\nx\nabcd!");
}

// ---- Finding 3: undo puts the cursor where the change began -----------

/// After `u`, each cursor is where the change began (the `vim` module
/// docs, "Undo and redo"). A normal-mode command acts at each selection's
/// settled head, so that is where `u` goes back to, even when the
/// selection wasn't a cursor.
#[test]
fn undo_after_an_operator_goes_back_to_the_head() {
    for keys in ["x", "dw", "D", "cwX\u{1b}", "sX\u{1b}"] {
        let mut v = Vim::new("abc def", &[Selection::new(0, 4)]);
        v.keys(keys);
        v.keys("u");
        assert_eq!(v.text(), "abc def", "{keys:?}");
        assert_eq!(v.selections(), [Selection::cursor(4)], "{keys:?}");
    }
}

/// `o` and `O` keep the cursors from before the key, so `u` puts each one
/// back where it was (the `vim` module docs, "Insert mode"): at its
/// settled head.
#[test]
fn undo_after_open_line_goes_back_to_the_head() {
    for keys in ["oX\u{1b}", "OX\u{1b}"] {
        let mut v = Vim::new("ab\ncd", &[Selection::new(0, 4)]);
        v.keys(keys);
        v.keys("u");
        assert_eq!(v.text(), "ab\ncd", "{keys:?}");
        assert_eq!(v.selections(), [Selection::cursor(4)], "{keys:?}");
    }
}

/// Cursors that settle onto one char merge before the edit, and `u` gives
/// back the merged cursor, as it would have without the edit.
#[test]
fn undo_after_an_operator_gives_back_settled_cursors() {
    // Offsets 1 and 2 of "ab\ncd" both settle onto `b`.
    let mut v = Vim::new("ab\ncd", &[Selection::cursor(1), Selection::cursor(2)]);
    v.keys("xu");
    assert_eq!(v.text(), "ab\ncd");
    assert_eq!(v.selections(), [Selection::cursor(1)]);
}

/// A visual edit still records the visual selection, so `u` leaves a
/// cursor where the selection began, as the `vim` module docs say.
#[test]
fn undo_after_a_visual_delete_goes_back_to_the_selection_start() {
    let mut v = Vim::new("abc def", &[Selection::cursor(5)]);
    v.keys("vhhhdu");
    assert_eq!(v.text(), "abc def");
    assert_eq!(v.selections(), [Selection::cursor(2)]);
}
