//! T-106 acceptance tests: keymaps as data, and table tests of how
//! `Dispatcher::resolve` turns keys into commands: chords, precedence,
//! typing, modes, pending operators and switching keymaps (ED-6).

use helm_editor::{
    Action, Binding, CommandId, CommandRegistry, Dispatcher, Invocation, KeyEvent, KeyParseError,
    KeySequence, KeyState, Keymap, KeymapError, Keystroke, Mode, ModeId, Resolution, Typing,
};

/// `Keystroke::parse`, which must accept `text`.
fn ks(text: &str) -> Keystroke {
    Keystroke::parse(text).expect("valid keystroke")
}

/// A binding from `keys` to running `command`.
fn run(keys: &str, command: &str) -> Binding {
    Binding::run(keys, command).expect("valid keys")
}

/// The resolution that runs `command` with no arguments.
fn runs(command: &str) -> Resolution {
    Resolution::Run(Invocation::new(command))
}

/// The resolution that runs operator `operator` over motion `motion`.
fn runs_operator(operator: &str, motion: &str) -> Resolution {
    let mut invocation = Invocation::new(operator);
    invocation.args.motion = Some(CommandId::from(motion));
    Resolution::Run(invocation)
}

/// The resolution that types `text`.
fn types(text: &str) -> Resolution {
    Resolution::Run(Invocation::insert(text))
}

/// A keymap named `name` with `modes`, given as id and typing, and
/// `bindings` in order.
fn keymap(name: &str, modes: &[(&str, Typing)], bindings: Vec<Binding>) -> Keymap {
    let modes = modes
        .iter()
        .map(|&(id, typing)| Mode::new(id, typing))
        .collect();
    let mut keymap = Keymap::new(name, modes).expect("valid modes");
    for binding in bindings {
        keymap.bind(binding).expect("valid binding");
    }
    keymap
}

/// A keymap with one mode, `"default"`, where typing inserts.
fn one_mode(bindings: Vec<Binding>) -> Keymap {
    keymap("one", &[("default", Typing::Insert)], bindings)
}

/// A small Vim-like keymap: `normal` first, then `insert` and `operator`.
fn vim_like() -> Keymap {
    keymap(
        "vim",
        &[
            ("normal", Typing::Ignore),
            ("insert", Typing::Insert),
            ("operator", Typing::Ignore),
        ],
        vec![
            Binding::switch_mode("i", "insert")
                .unwrap()
                .in_mode("normal"),
            run("a", "vim.right").in_mode("normal").then_mode("insert"),
            Binding::switch_mode("escape", "normal")
                .unwrap()
                .in_mode("insert"),
            run("x", "editor.delete_forward").in_mode("normal"),
            Binding::operator("d", "vim.delete")
                .unwrap()
                .in_mode("normal")
                .then_mode("operator"),
            Binding::operator("c", "vim.change")
                .unwrap()
                .then_mode("operator"),
            run("d", "vim.line").in_mode("operator"),
            run("w", "vim.word"),
            run("g g", "vim.top"),
            run("j", "vim.down").in_mode("normal"),
            run("j", "vim.down").in_mode("operator"),
        ],
    )
}

/// A dispatcher with an empty registry and `keymap`, and a state for one
/// editor.
fn dispatcher(keymap: Keymap) -> (Dispatcher, KeyState) {
    let dispatcher = Dispatcher::new(CommandRegistry::new(), keymap);
    let state = dispatcher.key_state();
    (dispatcher, state)
}

/// Resolves each keystroke of `keys`, with no contexts, and returns the
/// resolutions.
fn press(dispatcher: &Dispatcher, state: &mut KeyState, keys: &str) -> Vec<Resolution> {
    press_in(dispatcher, state, keys, &[])
}

/// Resolves each keystroke of `keys` with `contexts`.
fn press_in(
    dispatcher: &Dispatcher,
    state: &mut KeyState,
    keys: &str,
    contexts: &[&str],
) -> Vec<Resolution> {
    KeySequence::parse(keys)
        .expect("valid keys")
        .as_slice()
        .iter()
        .map(|&keystroke| dispatcher.resolve(state, &KeyEvent::new(keystroke), contexts))
        .collect()
}

#[test]
fn keymap_new_checks_modes() {
    assert_eq!(
        Keymap::new("empty", Vec::new()).unwrap_err(),
        KeymapError::NoModes
    );
    let twice = vec![
        Mode::new("normal", Typing::Ignore),
        Mode::new("insert", Typing::Insert),
        Mode::new("normal", Typing::Insert),
    ];
    assert_eq!(
        Keymap::new("twice", twice).unwrap_err(),
        KeymapError::DuplicateMode {
            mode: ModeId::from("normal")
        }
    );

    let keymap = vim_like();
    assert_eq!(keymap.name(), "vim");
    assert_eq!(keymap.initial_mode().as_str(), "normal");
    let ids: Vec<&str> = keymap.modes().iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["normal", "insert", "operator"]);
    assert_eq!(
        keymap.mode("insert"),
        Some(&Mode {
            id: ModeId::from("insert"),
            typing: Typing::Insert
        })
    );
    assert_eq!(keymap.mode("visual"), None);
}

#[test]
fn bind_checks_modes_and_keeps_order() {
    let mut keymap = keymap(
        "k",
        &[("normal", Typing::Ignore), ("insert", Typing::Insert)],
        Vec::new(),
    );
    assert_eq!(
        keymap.bind(run("x", "a.x").in_mode("visual")),
        Err(KeymapError::UnknownMode {
            mode: ModeId::from("visual")
        })
    );
    assert_eq!(
        keymap.bind(run("x", "a.x").then_mode("replace")),
        Err(KeymapError::UnknownMode {
            mode: ModeId::from("replace")
        })
    );
    assert!(keymap.bindings().is_empty());

    keymap.bind(run("x", "a.first")).unwrap();
    keymap.bind(run("y", "a.second").in_mode("insert")).unwrap();
    keymap.bind(run("x", "a.third")).unwrap();
    let commands: Vec<Option<Action>> =
        keymap.bindings().iter().map(|b| b.action.clone()).collect();
    assert_eq!(
        commands,
        [
            Some(Action::Run(CommandId::from("a.first"))),
            Some(Action::Run(CommandId::from("a.second"))),
            Some(Action::Run(CommandId::from("a.third"))),
        ]
    );
}

#[test]
fn binding_builders_fill_the_fields() {
    let binding = run("cmd-k cmd-s", "workspace.save_all")
        .in_mode("default")
        .when("editor_focus")
        .then_mode("other");
    assert_eq!(
        binding,
        Binding {
            keys: KeySequence::parse("cmd-k cmd-s").unwrap(),
            mode: Some(ModeId::from("default")),
            context: Some("editor_focus".to_owned()),
            action: Some(Action::Run(CommandId::from("workspace.save_all"))),
            then_mode: Some(ModeId::from("other")),
        }
    );
    assert_eq!(
        Binding::operator("d", "vim.delete").unwrap(),
        Binding {
            keys: KeySequence::parse("d").unwrap(),
            mode: None,
            context: None,
            action: Some(Action::Operator(CommandId::from("vim.delete"))),
            then_mode: None,
        }
    );
    assert_eq!(
        Binding::switch_mode("escape", "normal").unwrap(),
        Binding {
            keys: KeySequence::parse("escape").unwrap(),
            mode: None,
            context: None,
            action: None,
            then_mode: Some(ModeId::from("normal")),
        }
    );
    assert_eq!(
        Binding::run("cmd-nope", "a.b"),
        Err(KeyParseError::UnknownKey {
            keystroke: "cmd-nope".to_owned()
        })
    );
}

#[test]
fn a_new_state_starts_idle_in_the_initial_mode() {
    let (_, state) = dispatcher(vim_like());
    assert_eq!(state.mode().as_str(), "normal");
    assert!(state.pending().is_empty());
    assert_eq!(state.operator(), None);
}

#[test]
fn single_keys_run_their_command() {
    let (d, mut state) = dispatcher(one_mode(vec![
        run("cmd-z", "editor.undo"),
        run("shift-cmd-z", "editor.redo"),
        run("enter", "editor.newline"),
    ]));
    assert_eq!(
        press(&d, &mut state, "cmd-z cmd-Z enter"),
        [
            runs("editor.undo"),
            runs("editor.redo"),
            runs("editor.newline")
        ]
    );
    assert!(state.pending().is_empty());
}

#[test]
fn a_chord_waits_for_its_last_key() {
    let (d, mut state) = dispatcher(one_mode(vec![
        run("cmd-k cmd-s", "workspace.save_all"),
        run("cmd-k cmd-c", "editor.comment"),
        run("ctrl-x ctrl-x ctrl-x", "a.three"),
    ]));
    assert_eq!(press(&d, &mut state, "cmd-k"), [Resolution::Pending]);
    assert_eq!(state.pending(), &[ks("cmd-k")]);
    assert_eq!(press(&d, &mut state, "cmd-c"), [runs("editor.comment")]);
    assert!(state.pending().is_empty());

    assert_eq!(
        press(&d, &mut state, "cmd-k cmd-s ctrl-x ctrl-x ctrl-x"),
        [
            Resolution::Pending,
            runs("workspace.save_all"),
            Resolution::Pending,
            Resolution::Pending,
            runs("a.three"),
        ]
    );
}

#[test]
fn a_broken_chord_drops_every_key() {
    let (d, mut state) = dispatcher(one_mode(vec![
        run("cmd-k cmd-s", "workspace.save_all"),
        run("cmd-j", "a.j"),
    ]));
    // `x` would type, and `cmd-j` is bound, but after `cmd-k` both break the
    // chord and are dropped.
    assert_eq!(
        press(&d, &mut state, "cmd-k x"),
        [Resolution::Pending, Resolution::Cancelled]
    );
    assert!(state.pending().is_empty());
    assert_eq!(
        press(&d, &mut state, "cmd-k cmd-j"),
        [Resolution::Pending, Resolution::Cancelled]
    );
    // Escape cancels a chord when nothing binds it.
    assert_eq!(
        press(&d, &mut state, "cmd-k escape"),
        [Resolution::Pending, Resolution::Cancelled]
    );
    // Afterwards keys resolve as usual.
    assert_eq!(press(&d, &mut state, "x cmd-j"), [types("x"), runs("a.j")]);
}

#[test]
fn a_chord_prefix_beats_an_exact_match() {
    // `cmd-k` alone is bound, but it also starts a chord, so it waits.
    let (d, mut state) = dispatcher(one_mode(vec![
        run("cmd-k cmd-s", "workspace.save_all"),
        run("cmd-k", "a.k"),
    ]));
    assert_eq!(press(&d, &mut state, "cmd-k"), [Resolution::Pending]);
    assert_eq!(press(&d, &mut state, "cmd-s"), [runs("workspace.save_all")]);
}

#[test]
fn precedence_table() {
    // Every binding is on `cmd-d`, which types nothing. Each case lists the
    // bindings as (mode, context, command) in the order they are added, the
    // state's mode, the contexts that hold, and the command that wins.
    type Case = (
        &'static [(Option<&'static str>, Option<&'static str>, &'static str)],
        &'static str,
        &'static [&'static str],
        Option<&'static str>,
    );
    let cases: &[Case] = &[
        // Two plain bindings: the later one.
        (
            &[(None, None, "a.1"), (None, None, "a.2")],
            "normal",
            &[],
            Some("a.2"),
        ),
        // A mode beats no mode, wherever it was added.
        (
            &[(Some("normal"), None, "a.1"), (None, None, "a.2")],
            "normal",
            &[],
            Some("a.1"),
        ),
        (
            &[(None, None, "a.1"), (Some("normal"), None, "a.2")],
            "normal",
            &[],
            Some("a.2"),
        ),
        // A binding for another mode doesn't apply.
        (
            &[(None, None, "a.1"), (Some("insert"), None, "a.2")],
            "normal",
            &[],
            Some("a.1"),
        ),
        (&[(Some("insert"), None, "a.1")], "normal", &[], None),
        // A context beats no context when it holds.
        (
            &[(None, Some("sel"), "a.1"), (None, None, "a.2")],
            "normal",
            &["sel"],
            Some("a.1"),
        ),
        // And doesn't apply when it doesn't hold.
        (
            &[(None, Some("sel"), "a.1"), (None, None, "a.2")],
            "normal",
            &["other"],
            Some("a.2"),
        ),
        (&[(None, Some("sel"), "a.1")], "normal", &[], None),
        // Mode first, then context.
        (
            &[(Some("normal"), None, "a.1"), (None, Some("sel"), "a.2")],
            "normal",
            &["sel"],
            Some("a.1"),
        ),
        (
            &[
                (Some("normal"), Some("sel"), "a.1"),
                (Some("normal"), None, "a.2"),
            ],
            "normal",
            &["sel"],
            Some("a.1"),
        ),
        // Equal rank: the later one.
        (
            &[
                (Some("normal"), Some("x"), "a.1"),
                (Some("normal"), Some("y"), "a.2"),
            ],
            "normal",
            &["x", "y"],
            Some("a.2"),
        ),
        (
            &[
                (Some("normal"), Some("x"), "a.1"),
                (Some("normal"), Some("y"), "a.2"),
            ],
            "normal",
            &["x"],
            Some("a.1"),
        ),
        // The state's mode decides.
        (
            &[(Some("normal"), None, "a.1"), (Some("insert"), None, "a.2")],
            "insert",
            &[],
            Some("a.2"),
        ),
    ];
    for (i, &(bindings, mode, contexts, winner)) in cases.iter().enumerate() {
        let mut list: Vec<Binding> = bindings
            .iter()
            .map(|&(mode, context, command)| {
                let mut binding = run("cmd-d", command);
                if let Some(mode) = mode {
                    binding = binding.in_mode(mode);
                }
                if let Some(context) = context {
                    binding = binding.when(context);
                }
                binding
            })
            .collect();
        // `escape` moves from normal to insert, to set up the state's mode.
        list.push(
            Binding::switch_mode("escape", "insert")
                .unwrap()
                .in_mode("normal"),
        );
        let (d, mut state) = dispatcher(keymap(
            "k",
            &[("normal", Typing::Insert), ("insert", Typing::Insert)],
            list,
        ));
        if mode == "insert" {
            assert_eq!(press(&d, &mut state, "escape"), [Resolution::Done]);
        }
        let expected = winner.map_or(Resolution::Unbound, runs);
        assert_eq!(
            press_in(&d, &mut state, "cmd-d", contexts),
            [expected],
            "case {i}"
        );
    }
}

#[test]
fn contexts_scope_chords_too() {
    let (d, mut state) = dispatcher(one_mode(vec![
        run("cmd-k cmd-s", "a.save").when("editor_focus"),
        run("cmd-k", "a.k"),
    ]));
    // Without the context the chord doesn't apply, so `cmd-k` runs alone.
    assert_eq!(press(&d, &mut state, "cmd-k"), [runs("a.k")]);
    assert_eq!(
        press_in(&d, &mut state, "cmd-k cmd-s", &["editor_focus"]),
        [Resolution::Pending, runs("a.save")]
    );
}

#[test]
fn unbound_keys_type_their_text() {
    let (d, mut state) = dispatcher(one_mode(vec![run("cmd-d", "a.d")]));
    assert_eq!(
        press(&d, &mut state, "a A shift-b space é 😀"),
        [
            types("a"),
            types("A"),
            types("B"),
            types(" "),
            types("é"),
            types("😀")
        ]
    );
    // Ctrl and Cmd never type, and named keys have no text.
    assert_eq!(
        press(
            &d,
            &mut state,
            "ctrl-a cmd-a ctrl-cmd-a enter tab escape left f1"
        ),
        vec![Resolution::Unbound; 8]
    );
    // Option types the text the platform reports.
    let option_a = KeyEvent::with_text(ks("alt-a"), "å");
    assert_eq!(d.resolve(&mut state, &option_a, &[]), types("å"));
    // Ctrl with text still doesn't type.
    let ctrl = KeyEvent::with_text(ks("ctrl-a"), "a");
    assert_eq!(d.resolve(&mut state, &ctrl, &[]), Resolution::Unbound);
    // Empty text types nothing.
    let empty = KeyEvent::with_text(ks("x"), "");
    assert_eq!(d.resolve(&mut state, &empty, &[]), Resolution::Unbound);
    // A binding beats typing.
    let bound = KeyEvent::with_text(ks("cmd-d"), "d");
    assert_eq!(d.resolve(&mut state, &bound, &[]), runs("a.d"));
}

#[test]
fn ignoring_modes_drop_unbound_keys() {
    let (d, mut state) = dispatcher(vim_like());
    assert_eq!(
        press(&d, &mut state, "z Z space enter"),
        vec![Resolution::Unbound; 4]
    );
    assert_eq!(state.mode().as_str(), "normal");
    // `x` is bound in normal mode only.
    assert_eq!(
        press(&d, &mut state, "x i x"),
        [runs("editor.delete_forward"), Resolution::Done, types("x")]
    );
}

#[test]
fn bindings_switch_modes() {
    let (d, mut state) = dispatcher(vim_like());
    assert_eq!(press(&d, &mut state, "i"), [Resolution::Done]);
    assert_eq!(state.mode().as_str(), "insert");
    assert_eq!(press(&d, &mut state, "j"), [types("j")]);
    assert_eq!(press(&d, &mut state, "escape"), [Resolution::Done]);
    assert_eq!(state.mode().as_str(), "normal");
    // A binding that runs a command and switches mode does both.
    assert_eq!(press(&d, &mut state, "a"), [runs("vim.right")]);
    assert_eq!(state.mode().as_str(), "insert");
}

#[test]
fn an_operator_waits_for_its_motion() {
    let (d, mut state) = dispatcher(vim_like());
    assert_eq!(press(&d, &mut state, "d"), [Resolution::Pending]);
    assert_eq!(state.operator(), Some(&CommandId::from("vim.delete")));
    assert_eq!(state.mode().as_str(), "operator");
    assert!(state.pending().is_empty());

    assert_eq!(
        press(&d, &mut state, "w"),
        [runs_operator("vim.delete", "vim.word")]
    );
    assert_eq!(state.operator(), None);
    assert_eq!(state.mode().as_str(), "normal");

    // A mode-scoped binding gives `d d` its own motion, and a chord can be
    // the motion.
    assert_eq!(
        press(&d, &mut state, "d d d g g d j"),
        [
            Resolution::Pending,
            runs_operator("vim.delete", "vim.line"),
            Resolution::Pending,
            Resolution::Pending,
            runs_operator("vim.delete", "vim.top"),
            Resolution::Pending,
            runs_operator("vim.delete", "vim.down"),
        ]
    );
    assert_eq!(state.mode().as_str(), "normal");
}

#[test]
fn a_pending_operator_is_cancelled_by_any_other_key() {
    let (d, mut state) = dispatcher(vim_like());
    // An unbound key, escape, and a broken chord motion.
    for keys in ["d z", "d escape", "d g x", "d space"] {
        let resolutions = press(&d, &mut state, keys);
        assert_eq!(resolutions.last(), Some(&Resolution::Cancelled), "{keys}");
        assert_eq!(state.operator(), None, "{keys}");
        assert!(state.pending().is_empty(), "{keys}");
        assert_eq!(state.mode().as_str(), "normal", "{keys}");
    }
    // A second operator cancels the first, without its mode switch.
    assert_eq!(
        press(&d, &mut state, "d c"),
        [Resolution::Pending, Resolution::Cancelled]
    );
    assert_eq!(state.operator(), None);
    assert_eq!(state.mode().as_str(), "normal");
}

#[test]
fn an_operator_returns_to_the_mode_it_was_set_in() {
    // `c` is bound in every mode, so it can start in insert mode too.
    let (d, mut state) = dispatcher(vim_like());
    assert_eq!(press(&d, &mut state, "i"), [Resolution::Done]);
    assert_eq!(press(&d, &mut state, "c"), [Resolution::Pending]);
    assert_eq!(state.mode().as_str(), "operator");
    assert_eq!(
        press(&d, &mut state, "w"),
        [runs_operator("vim.change", "vim.word")]
    );
    assert_eq!(state.mode().as_str(), "insert");

    assert_eq!(
        press(&d, &mut state, "c escape"),
        [Resolution::Pending, Resolution::Cancelled]
    );
    assert_eq!(state.mode().as_str(), "insert");
}

#[test]
fn an_empty_binding_hides_an_earlier_one() {
    let mut empty = run("cmd-d", "unused.command");
    empty.action = None;
    let (d, mut state) = dispatcher(one_mode(vec![run("cmd-d", "a.d"), empty]));
    assert_eq!(press(&d, &mut state, "cmd-d"), [Resolution::Done]);
}

#[test]
fn an_empty_binding_drops_a_pending_operator() {
    let mut keymap = vim_like();
    let mut empty = run("q", "unused.command").in_mode("operator");
    empty.action = None;
    keymap.bind(empty).unwrap();
    let (d, mut state) = dispatcher(keymap);
    assert_eq!(
        press(&d, &mut state, "d q"),
        [Resolution::Pending, Resolution::Done]
    );
    assert_eq!(state.operator(), None);
    assert_eq!(state.mode().as_str(), "normal");
}

#[test]
fn cancel_drops_what_is_pending() {
    let (d, mut state) = dispatcher(vim_like());
    assert!(!state.cancel());

    press(&d, &mut state, "g");
    assert_eq!(state.pending(), &[ks("g")]);
    assert!(state.cancel());
    assert!(state.pending().is_empty());

    press(&d, &mut state, "d g");
    assert!(state.cancel());
    assert!(state.pending().is_empty());
    assert_eq!(state.operator(), None);
    assert_eq!(state.mode().as_str(), "normal");
    assert!(!state.cancel());

    // The next key resolves from scratch.
    assert_eq!(
        press(&d, &mut state, "g g"),
        [Resolution::Pending, runs("vim.top")]
    );
}

#[test]
fn switching_keymaps_drops_a_pending_chord() {
    let (mut d, mut state) = dispatcher(one_mode(vec![run("cmd-k cmd-s", "a.save_all")]));
    assert_eq!(press(&d, &mut state, "cmd-k"), [Resolution::Pending]);

    let old = d.set_keymap(keymap(
        "other",
        &[("default", Typing::Insert)],
        vec![run("cmd-s", "b.save"), run("cmd-k cmd-s", "b.save_all")],
    ));
    assert_eq!(old.name(), "one");
    assert_eq!(d.keymap().name(), "other");

    // `cmd-s` isn't the end of the old chord: it runs alone in the new map.
    assert_eq!(press(&d, &mut state, "cmd-s"), [runs("b.save")]);
    assert_eq!(
        press(&d, &mut state, "cmd-k cmd-s"),
        [Resolution::Pending, runs("b.save_all")]
    );
}

#[test]
fn switching_keymaps_reaches_every_editor() {
    let (mut d, mut first) = dispatcher(vim_like());
    let mut second = d.key_state();
    let mut third = d.key_state();
    press(&d, &mut first, "i");
    press(&d, &mut second, "d");
    press(&d, &mut third, "g");

    d.set_keymap(one_mode(vec![run("cmd-d", "b.d")]));
    for state in [&mut first, &mut second, &mut third] {
        assert_eq!(press(&d, state, "x cmd-d"), [types("x"), runs("b.d")]);
        assert_eq!(state.mode().as_str(), "default");
        assert_eq!(state.operator(), None);
    }
}

#[test]
fn switching_keeps_a_mode_the_new_keymap_has() {
    let (mut d, mut state) = dispatcher(vim_like());
    press(&d, &mut state, "i");
    assert_eq!(state.mode().as_str(), "insert");

    // Reloading the same keymap keeps insert mode.
    d.set_keymap(vim_like());
    d.sync(&mut state);
    assert_eq!(state.mode().as_str(), "insert");

    // VS Code-like has no insert mode: its initial mode.
    d.set_keymap(one_mode(Vec::new()));
    d.sync(&mut state);
    assert_eq!(state.mode().as_str(), "default");

    // And back: Vim has no default mode, so normal.
    d.set_keymap(vim_like());
    d.sync(&mut state);
    assert_eq!(state.mode().as_str(), "normal");
}

#[test]
fn switching_drops_a_pending_operator_and_its_mode() {
    let (mut d, mut state) = dispatcher(vim_like());
    press(&d, &mut state, "d g");
    assert_eq!(state.mode().as_str(), "operator");

    // The new keymap has an `operator` mode too, but the state first goes
    // back to the mode the operator was set in.
    d.set_keymap(vim_like());
    d.sync(&mut state);
    assert_eq!(state.operator(), None);
    assert!(state.pending().is_empty());
    assert_eq!(state.mode().as_str(), "normal");
    assert_eq!(press(&d, &mut state, "w"), [runs("vim.word")]);
}

#[test]
fn sync_without_a_switch_changes_nothing() {
    let (d, mut state) = dispatcher(vim_like());
    press(&d, &mut state, "d g");
    let before = state.clone();
    d.sync(&mut state);
    assert_eq!(state, before);
    assert_eq!(
        press(&d, &mut state, "g"),
        [runs_operator("vim.delete", "vim.top")]
    );
}

#[test]
fn a_state_made_after_a_switch_starts_in_the_new_initial_mode() {
    let (mut d, _) = dispatcher(one_mode(Vec::new()));
    d.set_keymap(vim_like());
    let state = d.key_state();
    assert_eq!(state.mode().as_str(), "normal");
}
