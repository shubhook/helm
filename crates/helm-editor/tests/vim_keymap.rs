//! T-108 acceptance tests: the Vim command ids, the registry that holds
//! every command, Vim's keymap as data, and switching between it and VS
//! Code's keymap at runtime (ED-5, ED-6).

use helm_editor::{
    Action, Binding, CommandRegistry, Dispatcher, Editor, Key, KeyEvent, KeySequence, Keystroke,
    Mode, ModeId, Modifiers, Resolution, Selection, Selections, Typing, cursor_commands,
    editor_commands, vim, vim_commands, vscode,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// The fifteen motion keys, the command each runs in normal and
/// operator-pending modes, and the one it runs in the visual modes.
const MOTIONS: [(&str, &str, &str); 15] = [
    ("h", "vim.left", "vim.select_left"),
    ("l", "vim.right", "vim.select_right"),
    ("j", "vim.down", "vim.select_down"),
    ("k", "vim.up", "vim.select_up"),
    ("w", "vim.next_word", "vim.select_next_word"),
    ("b", "vim.prev_word", "vim.select_prev_word"),
    ("e", "vim.word_end", "vim.select_word_end"),
    ("W", "vim.next_big_word", "vim.select_next_big_word"),
    ("B", "vim.prev_big_word", "vim.select_prev_big_word"),
    ("E", "vim.big_word_end", "vim.select_big_word_end"),
    ("0", "vim.line_start", "vim.select_line_start"),
    ("^", "vim.first_non_blank", "vim.select_first_non_blank"),
    ("$", "vim.line_end", "vim.select_line_end"),
    ("g g", "vim.first_line", "vim.select_first_line"),
    ("G", "vim.last_line", "vim.select_last_line"),
];

/// One documented binding: keys, mode, what it does, and its mode switch.
#[derive(Debug, PartialEq, Eq)]
struct Row {
    keys: String,
    mode: String,
    action: Option<Action>,
    then_mode: Option<String>,
}

fn row(keys: &str, mode: &str, action: Option<Action>, then_mode: Option<&str>) -> Row {
    Row {
        keys: KeySequence::parse(keys).expect("valid keys").to_string(),
        mode: mode.to_owned(),
        action,
        then_mode: then_mode.map(str::to_owned),
    }
}

fn runs(command: &str) -> Option<Action> {
    Some(Action::Run(command.into()))
}

/// Every binding the spec lists for `vim::keymap()`.
fn documented() -> Vec<Row> {
    let mut rows = Vec::new();
    for (keys, motion, _) in MOTIONS {
        rows.push(row(keys, "normal", runs(motion), None));
    }
    rows.push(row(
        "d",
        "normal",
        Some(Action::Operator("vim.delete".into())),
        Some("delete_pending"),
    ));
    rows.push(row(
        "c",
        "normal",
        Some(Action::Operator("vim.change".into())),
        Some("change_pending"),
    ));
    for (keys, command) in [
        ("x", "vim.delete_char"),
        ("X", "vim.delete_char_before"),
        ("D", "vim.delete_to_line_end"),
        ("u", "vim.undo"),
        ("ctrl-r", "vim.redo"),
    ] {
        rows.push(row(keys, "normal", runs(command), None));
    }
    for (keys, command) in [
        ("C", "vim.change_to_line_end"),
        ("s", "vim.substitute"),
        ("S", "vim.substitute_line"),
        ("i", "vim.insert"),
        ("a", "vim.append"),
        ("I", "vim.insert_line_start"),
        ("A", "vim.append_line_end"),
        ("o", "vim.open_below"),
        ("O", "vim.open_above"),
    ] {
        rows.push(row(keys, "normal", runs(command), Some("insert")));
    }
    rows.push(row("v", "normal", runs("vim.visual"), Some("visual")));
    rows.push(row("V", "normal", runs("vim.visual"), Some("visual_line")));
    for (keys, motion, _) in MOTIONS {
        rows.push(row(keys, "delete_pending", runs(motion), None));
    }
    rows.push(row("d", "delete_pending", runs("vim.line"), None));
    for (keys, motion, _) in MOTIONS {
        rows.push(row(keys, "change_pending", runs(motion), Some("insert")));
    }
    rows.push(row("c", "change_pending", runs("vim.line"), Some("insert")));
    for (mode, own, other_key, other, delete, change) in [
        (
            "visual",
            "v",
            "V",
            "visual_line",
            "vim.delete_selection",
            "vim.change_selection",
        ),
        (
            "visual_line",
            "V",
            "v",
            "visual",
            "vim.delete_lines",
            "vim.change_lines",
        ),
    ] {
        for (keys, _, select) in MOTIONS {
            rows.push(row(keys, mode, runs(select), None));
        }
        rows.push(row("escape", mode, runs("vim.exit_visual"), Some("normal")));
        rows.push(row(own, mode, runs("vim.exit_visual"), Some("normal")));
        rows.push(row(other_key, mode, None, Some(other)));
        rows.push(row("o", mode, runs("vim.swap_ends"), None));
        rows.push(row("d", mode, runs(delete), Some("normal")));
        rows.push(row("x", mode, runs(delete), Some("normal")));
        rows.push(row("c", mode, runs(change), Some("insert")));
        rows.push(row("s", mode, runs(change), Some("insert")));
    }
    rows.push(row("escape", "insert", runs("vim.normal"), Some("normal")));
    rows.push(row("enter", "insert", runs("editor.newline"), None));
    rows.push(row(
        "backspace",
        "insert",
        runs("editor.delete_backward"),
        None,
    ));
    rows.push(row("delete", "insert", runs("editor.delete_forward"), None));
    rows
}

fn as_row(binding: &Binding) -> Row {
    assert_eq!(binding.context, None, "{binding:?}");
    Row {
        keys: binding.keys.to_string(),
        mode: binding
            .mode
            .as_ref()
            .unwrap_or_else(|| panic!("{binding:?} has a mode"))
            .as_str()
            .to_owned(),
        action: binding.action.clone(),
        then_mode: binding.then_mode.as_ref().map(|m| m.as_str().to_owned()),
    }
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn vim_command_ids_are_the_documented_ones() {
    let mut expected: Vec<&str> = MOTIONS
        .iter()
        .flat_map(|&(_, motion, select)| [motion, select])
        .chain([
            "vim.line",
            "vim.delete",
            "vim.change",
            "vim.delete_char",
            "vim.delete_char_before",
            "vim.delete_to_line_end",
            "vim.change_to_line_end",
            "vim.substitute",
            "vim.substitute_line",
            "vim.insert",
            "vim.append",
            "vim.insert_line_start",
            "vim.append_line_end",
            "vim.open_below",
            "vim.open_above",
            "vim.normal",
            "vim.visual",
            "vim.exit_visual",
            "vim.swap_ends",
            "vim.delete_selection",
            "vim.change_selection",
            "vim.delete_lines",
            "vim.change_lines",
            "vim.undo",
            "vim.redo",
        ])
        .collect();
    expected.sort_unstable();
    assert_eq!(vim_commands::ALL.len(), 55);
    assert_eq!(vim_commands::ALL.to_vec(), expected);
    assert!(vim_commands::ALL.is_sorted());
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn with_vim_commands_holds_every_command() {
    let registry = CommandRegistry::with_vim_commands();
    let ids: Vec<&str> = registry.commands().map(|c| c.id.as_str()).collect();
    let mut expected: Vec<&str> = editor_commands::ALL
        .iter()
        .chain(&cursor_commands::ALL)
        .chain(&vim_commands::ALL)
        .copied()
        .collect();
    expected.sort_unstable();
    assert_eq!(ids, expected);
    assert_eq!(ids.len(), 91);
    for command in registry.commands() {
        assert!(!command.title.is_empty(), "{}", command.id);
    }
    // The registries before T-108 are unchanged.
    assert_eq!(CommandRegistry::with_all_commands().commands().count(), 36);
    assert_eq!(
        CommandRegistry::with_editor_commands().commands().count(),
        10
    );
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn the_keymap_has_vims_modes() {
    let keymap = vim::keymap();
    assert_eq!(keymap.name(), "vim");
    assert_eq!(vim::NAME, "vim");
    assert_eq!(
        keymap.modes(),
        &[
            Mode::new("normal", Typing::Count),
            Mode::new("insert", Typing::Insert),
            Mode::new("visual", Typing::Count),
            Mode::new("visual_line", Typing::Count),
            Mode::new("delete_pending", Typing::Count),
            Mode::new("change_pending", Typing::Count),
        ]
    );
    assert_eq!(keymap.initial_mode(), &ModeId::from("normal"));
    assert_eq!(
        [
            vim::NORMAL,
            vim::INSERT,
            vim::VISUAL,
            vim::VISUAL_LINE,
            vim::DELETE_PENDING,
            vim::CHANGE_PENDING
        ],
        [
            "normal",
            "insert",
            "visual",
            "visual_line",
            "delete_pending",
            "change_pending"
        ]
    );
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn the_keymap_has_exactly_the_documented_bindings() {
    let keymap = vim::keymap();
    let mut actual: Vec<Row> = keymap.bindings().iter().map(as_row).collect();
    let mut expected = documented();
    assert_eq!(expected.len(), 115);
    let key = |r: &Row| (r.mode.clone(), r.keys.clone());
    actual.sort_by_key(key);
    expected.sort_by_key(key);
    assert_eq!(actual, expected);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn every_bound_command_is_registered() {
    let registry = CommandRegistry::with_vim_commands();
    for binding in vim::keymap().bindings() {
        if let Some(Action::Run(command) | Action::Operator(command)) = &binding.action {
            assert!(
                registry.get(command.as_str()).is_some(),
                "{command} is not registered"
            );
        }
    }
}

/// An editor on `text` with a cursor at 0.
fn editor(text: &str) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    editor.set_selections(Selections::single(Selection::cursor(0)));
    (set, editor)
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(Keystroke::new(Key::Char(c), Modifiers::NONE))
}

fn named(key: Key) -> KeyEvent {
    KeyEvent::new(Keystroke::new(key, Modifiers::NONE))
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn each_motion_resolves_in_each_mode_it_is_bound_in() {
    let d = Dispatcher::new(CommandRegistry::with_vim_commands(), vim::keymap());
    for (keys, motion, select) in MOTIONS {
        let events: Vec<KeyEvent> = KeySequence::parse(keys)
            .unwrap()
            .as_slice()
            .iter()
            .map(|&k| KeyEvent::new(k))
            .collect();
        let resolve = |prefix: &[KeyEvent]| {
            let mut state = d.key_state();
            for event in prefix {
                d.resolve(&mut state, event, &[]);
            }
            let mut last = Resolution::Unbound;
            for event in &events {
                last = d.resolve(&mut state, event, &[]);
            }
            (last, state.mode().as_str().to_owned())
        };
        let run = |command: &str| {
            let mut invocation = helm_editor::Invocation::new(command);
            invocation.args = Default::default();
            Resolution::Run(invocation)
        };
        let operator = |op: &str| {
            let mut invocation = helm_editor::Invocation::new(op);
            invocation.args.motion = Some(motion.into());
            Resolution::Run(invocation)
        };
        assert_eq!(resolve(&[]), (run(motion), "normal".into()), "{keys}");
        assert_eq!(
            resolve(&[key('v')]),
            (run(select), "visual".into()),
            "{keys}"
        );
        assert_eq!(
            resolve(&[key('V')]),
            (run(select), "visual_line".into()),
            "{keys}"
        );
        assert_eq!(
            resolve(&[key('d')]),
            (operator("vim.delete"), "normal".into()),
            "{keys}"
        );
        assert_eq!(
            resolve(&[key('c')]),
            (operator("vim.change"), "insert".into()),
            "{keys}"
        );
    }
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn switching_between_vscode_and_vim_applies_to_open_editors() {
    let mut d = Dispatcher::new(CommandRegistry::with_vim_commands(), vscode::keymap());
    let (mut set, mut e) = editor("abc");
    let mut state = d.key_state();
    let mut press = |d: &Dispatcher, state: &mut helm_editor::KeyState, event: KeyEvent| {
        d.dispatch(state, &event, &[], &mut e, &mut set).unwrap();
    };
    // VS Code types `x`.
    press(&d, &mut state, key('x'));
    d.set_keymap(vim::keymap());
    // Vim starts in normal mode, where `x` deletes.
    press(&d, &mut state, key('x'));
    assert_eq!(state.mode().as_str(), "normal");
    press(&d, &mut state, key('i'));
    press(&d, &mut state, key('y'));
    assert_eq!(state.mode().as_str(), "insert");
    d.set_keymap(vscode::keymap());
    press(&d, &mut state, key('z'));
    assert_eq!(state.mode().as_str(), "default");
    d.set_keymap(vim::keymap());
    d.sync(&mut state);
    assert_eq!(state.mode().as_str(), "normal");
    press(&d, &mut state, named(Key::Escape));
    drop(press);
    assert_eq!(set.get(e.buffer()).unwrap().to_string(), "xyzbc");
}
