//! T-109 acceptance tests: the modes, bindings and commands T-109 adds to
//! Vim's keymap and registry (ED-5).
//!
//! T-108's `vim_keymap.rs` pins T-108's 115 bindings, six modes and 91
//! commands. These tests pin only what T-109 adds. Whether the additions
//! join `vim::keymap()` and `with_vim_commands()` is open question 2 of
//! `tasks/109-vim-objects.md`; `vim_harness` names the keymap and registry
//! in one place.

mod vim_harness;

use helm_editor::{
    Action, Binding, CommandId, Invocation, Key, KeyEvent, Keystroke, ModeId, Modifiers,
    Resolution, Typing, vim, vim_commands as ids,
};
use vim_harness::{editor_on, keymap, registry};

/// One binding, as data: keys, mode, action and then-mode.
type Row = (String, String, Option<Action>, Option<String>);

fn row(keys: &str, mode: &str, action: Option<Action>, then_mode: Option<&str>) -> Row {
    (
        keys.to_owned(),
        mode.to_owned(),
        action,
        then_mode.map(str::to_owned),
    )
}

fn runs(command: &str) -> Option<Action> {
    Some(Action::Run(CommandId::from(command)))
}

/// Every register name a `"` chord takes.
fn register_names() -> Vec<char> {
    let mut names: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    names.extend(['"', '-', '_', '+', '*']);
    names
}

/// The fifteen motion keys and their ids, as T-108 binds them.
const MOTIONS: [(&str, &str); 15] = [
    ("h", ids::LEFT),
    ("l", ids::RIGHT),
    ("j", ids::DOWN),
    ("k", ids::UP),
    ("w", ids::NEXT_WORD),
    ("b", ids::PREV_WORD),
    ("e", ids::WORD_END),
    ("W", ids::NEXT_BIG_WORD),
    ("B", ids::PREV_BIG_WORD),
    ("E", ids::BIG_WORD_END),
    ("0", ids::LINE_START),
    ("^", ids::FIRST_NON_BLANK),
    ("$", ids::LINE_END),
    ("g g", ids::FIRST_LINE),
    ("G", ids::LAST_LINE),
];

/// Each object key after `i` or `a`, with the inner and the a- ids under
/// an operator and in a visual mode, and whether it is a paragraph.
fn objects() -> Vec<(char, [&'static str; 4], bool)> {
    let word = [
        ids::INNER_WORD,
        ids::A_WORD,
        ids::SELECT_INNER_WORD,
        ids::SELECT_A_WORD,
    ];
    let big = [
        ids::INNER_BIG_WORD,
        ids::A_BIG_WORD,
        ids::SELECT_INNER_BIG_WORD,
        ids::SELECT_A_BIG_WORD,
    ];
    let paren = [
        ids::INNER_PAREN,
        ids::A_PAREN,
        ids::SELECT_INNER_PAREN,
        ids::SELECT_A_PAREN,
    ];
    let bracket = [
        ids::INNER_BRACKET,
        ids::A_BRACKET,
        ids::SELECT_INNER_BRACKET,
        ids::SELECT_A_BRACKET,
    ];
    let brace = [
        ids::INNER_BRACE,
        ids::A_BRACE,
        ids::SELECT_INNER_BRACE,
        ids::SELECT_A_BRACE,
    ];
    let angle = [
        ids::INNER_ANGLE,
        ids::A_ANGLE,
        ids::SELECT_INNER_ANGLE,
        ids::SELECT_A_ANGLE,
    ];
    let double = [
        ids::INNER_DOUBLE_QUOTE,
        ids::A_DOUBLE_QUOTE,
        ids::SELECT_INNER_DOUBLE_QUOTE,
        ids::SELECT_A_DOUBLE_QUOTE,
    ];
    let single = [
        ids::INNER_SINGLE_QUOTE,
        ids::A_SINGLE_QUOTE,
        ids::SELECT_INNER_SINGLE_QUOTE,
        ids::SELECT_A_SINGLE_QUOTE,
    ];
    let backtick = [
        ids::INNER_BACKTICK,
        ids::A_BACKTICK,
        ids::SELECT_INNER_BACKTICK,
        ids::SELECT_A_BACKTICK,
    ];
    let paragraph = [
        ids::INNER_PARAGRAPH,
        ids::A_PARAGRAPH,
        ids::SELECT_INNER_PARAGRAPH,
        ids::SELECT_A_PARAGRAPH,
    ];
    vec![
        ('w', word, false),
        ('W', big, false),
        ('(', paren, false),
        (')', paren, false),
        ('b', paren, false),
        ('[', bracket, false),
        (']', bracket, false),
        ('{', brace, false),
        ('}', brace, false),
        ('B', brace, false),
        ('<', angle, false),
        ('>', angle, false),
        ('"', double, false),
        ('\'', single, false),
        ('`', backtick, false),
        ('p', paragraph, true),
    ]
}

/// The bindings T-109 adds, as its `vim.rs` table documents them.
fn documented() -> Vec<Row> {
    let mut rows = vec![
        row(
            "y",
            "normal",
            Some(Action::Operator(ids::YANK.into())),
            Some("yank_pending"),
        ),
        row("Y", "normal", runs(ids::YANK_LINE), None),
        row("p", "normal", runs(ids::PUT_AFTER), None),
        row("P", "normal", runs(ids::PUT_BEFORE), None),
        row(".", "normal", runs(ids::REPEAT), None),
        row(":", "normal", None, Some("command_line")),
        row("enter", "command_line", runs(ids::EX), Some("normal")),
        row("escape", "command_line", None, Some("normal")),
        row("y", "visual", runs(ids::YANK_SELECTION), Some("normal")),
        row("p", "visual", runs(ids::PUT_SELECTION), Some("normal")),
        row("P", "visual", runs(ids::PUT_SELECTION_KEEP), Some("normal")),
        row("y", "visual_line", runs(ids::YANK_LINES), Some("normal")),
        row("p", "visual_line", runs(ids::PUT_LINES), Some("normal")),
        row(
            "P",
            "visual_line",
            runs(ids::PUT_LINES_KEEP),
            Some("normal"),
        ),
        row("y", "yank_pending", runs(ids::LINE), None),
    ];
    for (keys, motion) in MOTIONS {
        rows.push(row(keys, "yank_pending", runs(motion), None));
    }
    for mode in ["normal", "visual", "visual_line"] {
        for name in register_names() {
            let keys = format!("\" {name}");
            rows.push(row(&keys, mode, Some(Action::Register(name)), None));
        }
    }
    for (key, [inner, a, select_inner, select_a], paragraph) in objects() {
        for (prefix, id, select) in [('i', inner, select_inner), ('a', a, select_a)] {
            let keys = format!("{prefix} {key}");
            rows.push(row(&keys, "delete_pending", runs(id), None));
            rows.push(row(&keys, "change_pending", runs(id), Some("insert")));
            rows.push(row(&keys, "yank_pending", runs(id), None));
            let (in_visual, in_visual_line) = if paragraph {
                (Some("visual_line"), None)
            } else {
                (None, Some("visual"))
            };
            rows.push(row(&keys, "visual", runs(select), in_visual));
            rows.push(row(&keys, "visual_line", runs(select), in_visual_line));
        }
    }
    rows
}

fn as_row(binding: &Binding) -> Row {
    assert_eq!(binding.context, None, "{binding:?}");
    (
        binding
            .keys
            .as_slice()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" "),
        binding.mode.as_ref().expect("scoped to a mode").to_string(),
        binding.action.clone(),
        binding.then_mode.as_ref().map(ToString::to_string),
    )
}

/// Every id T-109 adds.
fn new_ids() -> Vec<(&'static str, String)> {
    let mut list: Vec<(&'static str, String)> = [
        (ids::YANK, "vim.yank"),
        (ids::YANK_LINE, "vim.yank_line"),
        (ids::PUT_AFTER, "vim.put_after"),
        (ids::PUT_BEFORE, "vim.put_before"),
        (ids::YANK_SELECTION, "vim.yank_selection"),
        (ids::YANK_LINES, "vim.yank_lines"),
        (ids::PUT_SELECTION, "vim.put_selection"),
        (ids::PUT_SELECTION_KEEP, "vim.put_selection_keep"),
        (ids::PUT_LINES, "vim.put_lines"),
        (ids::PUT_LINES_KEEP, "vim.put_lines_keep"),
        (ids::REPEAT, "vim.repeat"),
        (ids::EX, "vim.ex"),
    ]
    .into_iter()
    .map(|(id, text)| (id, text.to_owned()))
    .collect();
    let names = [
        "word",
        "big_word",
        "paren",
        "bracket",
        "brace",
        "angle",
        "double_quote",
        "single_quote",
        "backtick",
        "paragraph",
    ];
    let mut seen = Vec::new();
    for (_, four, _) in objects() {
        if seen.contains(&four[0]) {
            continue;
        }
        seen.push(four[0]);
        let name = names[seen.len() - 1];
        let expected = [
            format!("vim.inner_{name}"),
            format!("vim.a_{name}"),
            format!("vim.select_inner_{name}"),
            format!("vim.select_a_{name}"),
        ];
        for (id, text) in four.into_iter().zip(expected) {
            list.push((id, text));
        }
    }
    list
}

#[test]
fn the_new_ids_are_the_documented_ones() {
    let list = new_ids();
    assert_eq!(list.len(), 52);
    for (id, text) in &list {
        assert_eq!(id, text);
    }
    let mut sorted: Vec<&str> = list.iter().map(|(id, _)| *id).collect();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), 52);
}

#[test]
fn the_registry_holds_every_new_command() {
    let registry = registry();
    for (id, _) in new_ids() {
        let command = registry.get(id).unwrap_or_else(|| panic!("{id}"));
        assert!(!command.title.is_empty(), "{id}");
    }
    // Every T-108 command is still there.
    for id in ids::ALL {
        assert!(registry.get(id).is_some(), "{id}");
    }
}

#[test]
fn changes_are_marked() {
    let registry = registry();
    for id in [
        ids::DELETE,
        ids::CHANGE,
        ids::DELETE_CHAR,
        ids::DELETE_CHAR_BEFORE,
        ids::DELETE_TO_LINE_END,
        ids::CHANGE_TO_LINE_END,
        ids::SUBSTITUTE,
        ids::SUBSTITUTE_LINE,
        ids::INSERT,
        ids::APPEND,
        ids::INSERT_LINE_START,
        ids::APPEND_LINE_END,
        ids::OPEN_BELOW,
        ids::OPEN_ABOVE,
        ids::PUT_AFTER,
        ids::PUT_BEFORE,
    ] {
        assert!(registry.is_change(id), "{id}");
    }
    for id in [
        ids::NEXT_WORD,
        ids::YANK,
        ids::YANK_LINE,
        ids::UNDO,
        ids::REDO,
        ids::REPEAT,
        ids::EX,
        ids::NORMAL,
        ids::VISUAL,
        ids::DELETE_SELECTION,
        ids::CHANGE_SELECTION,
        ids::PUT_SELECTION,
        "editor.insert",
        "cursor.left",
    ] {
        assert!(!registry.is_change(id), "{id}");
    }
}

#[test]
fn the_keymap_has_the_two_new_modes() {
    let keymap = keymap();
    let modes: Vec<(&str, Typing)> = keymap
        .modes()
        .iter()
        .map(|m| (m.id.as_str(), m.typing))
        .collect();
    assert_eq!(
        modes,
        [
            ("normal", Typing::Count),
            ("insert", Typing::Insert),
            ("visual", Typing::Count),
            ("visual_line", Typing::Count),
            ("delete_pending", Typing::Count),
            ("change_pending", Typing::Count),
            ("yank_pending", Typing::Count),
            ("command_line", Typing::Line),
        ]
    );
    assert_eq!(vim::YANK_PENDING, "yank_pending");
    assert_eq!(vim::COMMAND_LINE, "command_line");
    assert_eq!(keymap.initial_mode(), &ModeId::from("normal"));
}

#[test]
fn the_keymap_adds_exactly_the_documented_bindings() {
    let keymap = keymap();
    let mut expected = documented();
    assert_eq!(expected.len(), 391);
    expected.sort_by_key(|r| format!("{r:?}"));
    let mut added: Vec<Row> = keymap.bindings().iter().skip(115).map(as_row).collect();
    added.sort_by_key(|r| format!("{r:?}"));
    assert_eq!(added, expected);
    assert_eq!(keymap.bindings().len(), 115 + 391);
    let reg = registry();
    for binding in keymap.bindings() {
        if let Some(Action::Run(id) | Action::Operator(id)) = &binding.action {
            assert!(reg.get(id.as_str()).is_some(), "{id}");
        }
    }
}

fn char_event(c: char) -> KeyEvent {
    KeyEvent::new(Keystroke::new(Key::Char(c), Modifiers::NONE))
}

#[test]
fn each_object_resolves_in_each_of_its_modes() {
    let d = helm_editor::Dispatcher::new(registry(), keymap());
    for (key, [inner, a, select_inner, select_a], paragraph) in objects() {
        for (prefix, id, select) in [('i', inner, select_inner), ('a', a, select_a)] {
            for (operator, op_id, after) in [
                ('d', ids::DELETE, "normal"),
                ('c', ids::CHANGE, "insert"),
                ('y', ids::YANK, "normal"),
            ] {
                let mut state = d.key_state();
                let mut last = Resolution::Unbound;
                for c in [operator, prefix, key] {
                    last = d.resolve(&mut state, &char_event(c), &[]);
                }
                let mut expected = Invocation::new(op_id);
                expected.args.motion = Some(CommandId::from(id));
                assert_eq!(last, Resolution::Run(expected), "{operator}{prefix}{key}");
                assert_eq!(state.mode().as_str(), after, "{operator}{prefix}{key}");
            }
            for (enter, after) in [
                ('v', if paragraph { "visual_line" } else { "visual" }),
                ('V', if paragraph { "visual_line" } else { "visual" }),
            ] {
                let mut state = d.key_state();
                let mut last = Resolution::Unbound;
                for c in [enter, prefix, key] {
                    last = d.resolve(&mut state, &char_event(c), &[]);
                }
                assert_eq!(
                    last,
                    Resolution::Run(Invocation::new(select)),
                    "{enter}{prefix}{key}"
                );
                assert_eq!(state.mode().as_str(), after, "{enter}{prefix}{key}");
            }
        }
    }
}

#[test]
fn register_chords_hold_a_register_in_each_of_their_modes() {
    let d = helm_editor::Dispatcher::new(registry(), keymap());
    for enter in [None, Some('v'), Some('V')] {
        for name in register_names() {
            let mut state = d.key_state();
            if let Some(c) = enter {
                d.resolve(&mut state, &char_event(c), &[]);
            }
            let mode = state.mode().clone();
            assert_eq!(
                d.resolve(&mut state, &char_event('"'), &[]),
                Resolution::Pending
            );
            assert_eq!(
                d.resolve(&mut state, &char_event(name), &[]),
                Resolution::Pending
            );
            assert_eq!(state.register(), Some(name), "{enter:?} {name}");
            assert_eq!(state.mode(), &mode);
        }
    }
    // Not in insert mode, and not inside an operator.
    let mut state = d.key_state();
    d.resolve(&mut state, &char_event('i'), &[]);
    let (mut set, mut editor) = editor_on("", vec![helm_editor::Selection::cursor(0)]);
    for c in ['"', 'a'] {
        d.dispatch(&mut state, &char_event(c), &[], &mut editor, &mut set)
            .unwrap();
    }
    assert_eq!(set.get(editor.buffer()).unwrap().to_string(), "\"a");
    let mut state = d.key_state();
    d.resolve(&mut state, &char_event('d'), &[]);
    assert_eq!(
        d.resolve(&mut state, &char_event('"'), &[]),
        Resolution::Cancelled
    );
    assert_eq!(state.mode().as_str(), "normal");
}

#[test]
fn a_broken_object_chord_cancels_the_operator() {
    let d = helm_editor::Dispatcher::new(registry(), keymap());
    let mut state = d.key_state();
    for c in ['d', 'i'] {
        assert_eq!(
            d.resolve(&mut state, &char_event(c), &[]),
            Resolution::Pending
        );
    }
    assert_eq!(
        d.resolve(&mut state, &char_event('q'), &[]),
        Resolution::Cancelled
    );
    assert_eq!(state.mode().as_str(), "normal");
    assert_eq!(state.operator(), None);
}
