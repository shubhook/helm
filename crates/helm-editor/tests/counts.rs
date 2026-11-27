//! T-108 acceptance tests: counts. Digits typed in a `Typing::Count` mode
//! make a count in the `KeyState`, `Dispatcher::resolve` puts it in the
//! `CommandArgs::count` of the command it goes with, and
//! `CommandRegistry::run` passes it to the command in its arguments.

use helm_editor::{
    Binding, Command, CommandContext, CommandError, CommandId, CommandRegistry, Dispatcher, Editor,
    Invocation, KeyEvent, KeySequence, KeyState, Keymap, Keystroke, Mode, ModeId, Resolution,
    Selection, Selections, Typing,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

fn ks(text: &str) -> Keystroke {
    Keystroke::parse(text).expect("valid keystroke")
}

fn event(text: &str) -> KeyEvent {
    KeyEvent::new(ks(text))
}

fn run(keys: &str, command: &str) -> Binding {
    Binding::run(keys, command).expect("valid keys")
}

fn runs(command: &str) -> Resolution {
    Resolution::Run(Invocation::new(command))
}

fn runs_operator(operator: &str, motion: &str) -> Resolution {
    let mut invocation = Invocation::new(operator);
    invocation.args.motion = Some(CommandId::from(motion));
    Resolution::Run(invocation)
}

/// A Vim-like keymap: `normal` counts, `insert` types, `ignore` drops
/// keys, and `operator` counts while an operator waits for its motion.
fn keymap() -> Keymap {
    let mut keymap = Keymap::new(
        "counting",
        vec![
            Mode::new("normal", Typing::Count),
            Mode::new("insert", Typing::Insert),
            Mode::new("ignore", Typing::Ignore),
            Mode::new("operator", Typing::Count),
        ],
    )
    .expect("valid modes");
    for binding in [
        run("w", "t.word").in_mode("normal"),
        run("w", "t.word").in_mode("operator"),
        run("0", "t.line_start").in_mode("normal"),
        run("0", "t.line_start").in_mode("operator"),
        run("x", "t.x").in_mode("ignore"),
        run("1", "t.one").in_mode("ignore"),
        run("1", "t.one").in_mode("normal"),
        run("ctrl-3", "t.ctrl_three"),
        run("g g", "t.top").in_mode("normal"),
        run("g 3", "t.g_three").in_mode("normal"),
        run("t", "t.count"),
        Binding::operator("d", "t.delete")
            .unwrap()
            .in_mode("normal")
            .then_mode("operator"),
        Binding::operator("c", "t.change")
            .unwrap()
            .in_mode("operator"),
        run("d", "t.line").in_mode("operator"),
        Binding::switch_mode("i", "insert")
            .unwrap()
            .in_mode("normal"),
        Binding::switch_mode("escape", "normal")
            .unwrap()
            .in_mode("insert"),
        Binding::switch_mode("v", "ignore")
            .unwrap()
            .in_mode("normal"),
        run("a", "t.append").in_mode("normal").then_mode("insert"),
    ] {
        keymap.bind(binding).expect("valid binding");
    }
    keymap
}

fn dispatcher() -> (Dispatcher, KeyState) {
    let dispatcher = Dispatcher::new(registry(), keymap());
    let state = dispatcher.key_state();
    (dispatcher, state)
}

/// `Dispatcher::resolve`, with the count a `Run` carries in its arguments
/// taken out and returned beside the resolution.
fn resolve_counted(
    d: &Dispatcher,
    state: &mut KeyState,
    event: &KeyEvent,
    contexts: &[&str],
) -> (Resolution, Option<usize>) {
    match d.resolve(state, event, contexts) {
        Resolution::Run(mut invocation) => {
            let count = invocation.args.count.take();
            (Resolution::Run(invocation), count)
        }
        other => (other, None),
    }
}

/// `CommandRegistry::run` of `invocation` with `count` in its arguments.
fn run_counted(
    registry: &CommandRegistry,
    invocation: &Invocation,
    count: Option<usize>,
    editor: &mut Editor,
    set: &mut BufferSet,
) -> Result<(), CommandError> {
    let mut invocation = invocation.clone();
    invocation.args.count = count;
    registry.run(&invocation, editor, set)
}

/// Resolves each keystroke of `keys` with `resolve_counted`, and returns
/// every result.
fn press(d: &Dispatcher, state: &mut KeyState, keys: &str) -> Vec<(Resolution, Option<usize>)> {
    KeySequence::parse(keys)
        .expect("valid keys")
        .as_slice()
        .iter()
        .map(|&k| resolve_counted(d, state, &KeyEvent::new(k), &[]))
        .collect()
}

/// The last result of `press`.
fn last(d: &Dispatcher, state: &mut KeyState, keys: &str) -> (Resolution, Option<usize>) {
    press(d, state, keys).pop().expect("at least one key")
}

/// `t.count` types the count it was given, or `none`.
fn type_count(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let text = cx.args.count.map_or("none".to_owned(), |n| n.to_string());
    cx.editor.insert(cx.set, &format!("{text};"))?;
    Ok(())
}

fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    registry
        .register(Command {
            id: CommandId::from("t.count"),
            title: "Type the count",
            run: type_count,
        })
        .unwrap();
    registry
}

fn setup(text: &str) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    editor.set_selections(Selections::single(Selection::cursor(text.len())));
    (set, editor)
}

fn text(set: &BufferSet, editor: &Editor) -> String {
    set.get(editor.buffer()).expect("in the set").to_string()
}

#[test]
fn digits_make_a_count_that_goes_with_the_next_command() {
    let (d, mut state) = dispatcher();
    assert_eq!(state.count(), None);
    assert_eq!(press(&d, &mut state, "3"), [(Resolution::Pending, None)]);
    assert_eq!(state.count(), Some(3));
    assert_eq!(state.pending(), []);
    assert_eq!(press(&d, &mut state, "5"), [(Resolution::Pending, None)]);
    assert_eq!(state.count(), Some(35));
    assert_eq!(press(&d, &mut state, "w"), [(runs("t.word"), Some(35))]);
    assert_eq!(state.count(), None);
    // The next command has no count.
    assert_eq!(press(&d, &mut state, "w"), [(runs("t.word"), None)]);
}

#[test]
fn resolve_gives_the_same_resolution_and_state() {
    use Resolution::{Cancelled, Done, Pending, Unbound};
    // A `Run` with the count it carries in its arguments.
    let run = |command: &str, motion: Option<&str>, count: Option<usize>| {
        let mut invocation = Invocation::new(command);
        invocation.args.motion = motion.map(CommandId::from);
        invocation.args.count = count;
        Resolution::Run(invocation)
    };
    // Each key history: what each key resolves to, then the state's mode,
    // count and operator after the last key. No chord is left pending.
    type Case = (
        &'static str,
        Vec<Resolution>,
        &'static str,
        Option<usize>,
        Option<&'static str>,
    );
    let cases: [Case; 14] = [
        (
            "3 w",
            vec![Pending, run("t.word", None, Some(3))],
            "normal",
            None,
            None,
        ),
        (
            "1 0 w",
            vec![Pending, Pending, run("t.word", None, Some(10))],
            "normal",
            None,
            None,
        ),
        (
            "0",
            vec![run("t.line_start", None, None)],
            "normal",
            None,
            None,
        ),
        (
            "2 d 3 w",
            vec![
                Pending,
                Pending,
                Pending,
                run("t.delete", Some("t.word"), Some(6)),
            ],
            "normal",
            None,
            None,
        ),
        (
            "d d",
            vec![Pending, run("t.delete", Some("t.line"), None)],
            "normal",
            None,
            None,
        ),
        ("3 escape", vec![Pending, Cancelled], "normal", None, None),
        ("3 q", vec![Pending, Cancelled], "normal", None, None),
        (
            "g 3",
            vec![Pending, run("t.g_three", None, None)],
            "normal",
            None,
            None,
        ),
        (
            "3 g g",
            vec![Pending, Pending, run("t.top", None, Some(3))],
            "normal",
            None,
            None,
        ),
        (
            "i 3 escape",
            vec![Done, Resolution::Run(Invocation::insert("3")), Done],
            "normal",
            None,
            None,
        ),
        (
            "v 1",
            vec![Done, run("t.one", None, None)],
            "ignore",
            None,
            None,
        ),
        ("3 v w", vec![Pending, Done, Unbound], "ignore", None, None),
        (
            "ctrl-3",
            vec![run("t.ctrl_three", None, None)],
            "normal",
            None,
            None,
        ),
        (
            "2 d c",
            vec![Pending, Pending, Cancelled],
            "normal",
            None,
            None,
        ),
    ];
    for (keys, expected, mode, count, operator) in cases {
        let (d, mut state) = dispatcher();
        let resolutions: Vec<Resolution> = KeySequence::parse(keys)
            .unwrap()
            .as_slice()
            .iter()
            .map(|&k| d.resolve(&mut state, &KeyEvent::new(k), &[]))
            .collect();
        assert_eq!(resolutions, expected, "{keys}");
        assert_eq!(state.mode().as_str(), mode, "{keys}");
        assert_eq!(state.count(), count, "{keys}");
        assert_eq!(
            state.operator(),
            operator.map(CommandId::from).as_ref(),
            "{keys}"
        );
        assert_eq!(state.pending(), [], "{keys}");
    }
}

#[test]
fn zero_is_a_motion_alone_and_a_digit_after_a_count() {
    let (d, mut state) = dispatcher();
    assert_eq!(last(&d, &mut state, "0"), (runs("t.line_start"), None));
    assert_eq!(last(&d, &mut state, "1 0 w"), (runs("t.word"), Some(10)));
    assert_eq!(last(&d, &mut state, "2 0 0 0"), (Resolution::Pending, None));
    assert_eq!(state.count(), Some(2000));
    assert_eq!(last(&d, &mut state, "0"), (Resolution::Pending, None));
    assert_eq!(state.count(), Some(20_000));
}

#[test]
fn digits_beat_bindings_in_a_counting_mode() {
    let (d, mut state) = dispatcher();
    // `1` is bound in normal mode, but a counting mode counts it.
    assert_eq!(last(&d, &mut state, "1"), (Resolution::Pending, None));
    assert_eq!(state.count(), Some(1));
    assert_eq!(last(&d, &mut state, "t"), (runs("t.count"), Some(1)));
}

#[test]
fn only_counting_modes_count() {
    let (d, mut state) = dispatcher();
    // Insert mode types digits.
    press(&d, &mut state, "i");
    assert_eq!(state.mode().as_str(), "insert");
    let (resolution, count) = resolve_counted(&d, &mut state, &event("3"), &[]);
    assert_eq!(resolution, Resolution::Run(Invocation::insert("3")));
    assert_eq!((count, state.count()), (None, None));
    // An ignoring mode runs its binding on `1` and drops other digits.
    press(&d, &mut state, "escape v");
    assert_eq!(state.mode().as_str(), "ignore");
    assert_eq!(last(&d, &mut state, "1"), (runs("t.one"), None));
    assert_eq!(last(&d, &mut state, "2"), (Resolution::Unbound, None));
    assert_eq!(state.count(), None);
    // A one-mode keymap where typing inserts never counts.
    let mut plain = Keymap::new("plain", vec![Mode::new("default", Typing::Insert)]).unwrap();
    plain.bind(run("1", "t.one")).unwrap();
    let d = Dispatcher::new(CommandRegistry::new(), plain);
    let mut state = d.key_state();
    assert_eq!(last(&d, &mut state, "1"), (runs("t.one"), None));
    assert_eq!(
        last(&d, &mut state, "2"),
        (Resolution::Run(Invocation::insert("2")), None)
    );
}

#[test]
fn a_counting_mode_types_nothing() {
    let (d, mut state) = dispatcher();
    assert_eq!(last(&d, &mut state, "q"), (Resolution::Unbound, None));
    assert_eq!(last(&d, &mut state, "space"), (Resolution::Unbound, None));
}

#[test]
fn digits_with_modifiers_are_keys() {
    let (d, mut state) = dispatcher();
    assert_eq!(last(&d, &mut state, "ctrl-3"), (runs("t.ctrl_three"), None));
    assert_eq!(last(&d, &mut state, "alt-3"), (Resolution::Unbound, None));
    assert_eq!(state.count(), None);
    // A count goes with a binding on a digit with a modifier.
    assert_eq!(
        last(&d, &mut state, "2 ctrl-3"),
        (runs("t.ctrl_three"), Some(2))
    );
}

#[test]
fn a_count_waits_through_a_chord() {
    let (d, mut state) = dispatcher();
    assert_eq!(
        press(&d, &mut state, "1 2 g g"),
        [
            (Resolution::Pending, None),
            (Resolution::Pending, None),
            (Resolution::Pending, None),
            (runs("t.top"), Some(12)),
        ]
    );
    // A digit inside a chord is a key of the chord, not a count.
    assert_eq!(last(&d, &mut state, "g 3"), (runs("t.g_three"), None));
    assert_eq!(last(&d, &mut state, "4 g 3"), (runs("t.g_three"), Some(4)));
    // And one that breaks the chord breaks it, dropping the count.
    assert_eq!(last(&d, &mut state, "5 g 4"), (Resolution::Cancelled, None));
    assert_eq!(state.count(), None);
}

#[test]
fn operator_counts_multiply() {
    let cases: [(&str, Option<usize>); 6] = [
        ("d w", None),
        ("2 d w", Some(2)),
        ("d 3 w", Some(3)),
        ("2 d 3 w", Some(6)),
        ("1 2 d 1 0 w", Some(120)),
        ("3 d d", Some(3)),
    ];
    for (keys, count) in cases {
        let (d, mut state) = dispatcher();
        let results = press(&d, &mut state, keys);
        let motion = if keys.ends_with('w') {
            "t.word"
        } else {
            "t.line"
        };
        assert_eq!(
            results.last().unwrap(),
            &(runs_operator("t.delete", motion), count),
            "{keys}"
        );
        assert!(
            results[..results.len() - 1]
                .iter()
                .all(|r| *r == (Resolution::Pending, None)),
            "{keys}: {results:?}"
        );
        assert_eq!(state.mode().as_str(), "normal");
        assert_eq!(state.count(), None);
    }
}

#[test]
fn a_count_before_an_operator_moves_onto_it() {
    let (d, mut state) = dispatcher();
    press(&d, &mut state, "2 d");
    assert_eq!(state.operator(), Some(&CommandId::from("t.delete")));
    assert_eq!(state.mode().as_str(), "operator");
    assert_eq!(state.count(), None);
    press(&d, &mut state, "4");
    assert_eq!(state.count(), Some(4));
    // `0` after the operator's count is a digit, and alone it is a motion.
    assert_eq!(
        last(&d, &mut state, "0 w"),
        (runs_operator("t.delete", "t.word"), Some(80))
    );
    assert_eq!(
        last(&d, &mut state, "d 0"),
        (runs_operator("t.delete", "t.line_start"), None)
    );
}

#[test]
fn cancelling_drops_the_count() {
    let (d, mut state) = dispatcher();
    // An unbound key and Escape cancel a count, and are dropped.
    assert_eq!(last(&d, &mut state, "3 q"), (Resolution::Cancelled, None));
    assert_eq!(state.count(), None);
    assert_eq!(
        last(&d, &mut state, "3 escape"),
        (Resolution::Cancelled, None)
    );
    assert_eq!(last(&d, &mut state, "w"), (runs("t.word"), None));
    // A second operator cancels the first and both counts.
    assert_eq!(
        last(&d, &mut state, "2 d 3 c"),
        (Resolution::Cancelled, None)
    );
    assert_eq!((state.count(), state.operator()), (None, None));
    assert_eq!(state.mode().as_str(), "normal");
    // A binding with no action drops the count.
    assert_eq!(last(&d, &mut state, "3 v"), (Resolution::Done, None));
    assert_eq!(state.mode().as_str(), "ignore");
    assert_eq!(state.count(), None);
}

#[test]
fn a_mode_switching_run_takes_the_count() {
    let (d, mut state) = dispatcher();
    assert_eq!(last(&d, &mut state, "4 a"), (runs("t.append"), Some(4)));
    assert_eq!(state.mode().as_str(), "insert");
}

#[test]
fn cancel_drops_the_count_and_says_so() {
    let (d, mut state) = dispatcher();
    assert!(!state.cancel());
    press(&d, &mut state, "7");
    assert!(state.cancel());
    assert_eq!(state.count(), None);
    assert_eq!(state.mode().as_str(), "normal");
    press(&d, &mut state, "2 d 3");
    assert!(state.cancel());
    assert_eq!((state.count(), state.operator()), (None, None));
    assert_eq!(state.mode().as_str(), "normal");
    assert_eq!(last(&d, &mut state, "w"), (runs("t.word"), None));
}

#[test]
fn switching_keymaps_drops_the_count() {
    let (mut d, mut state) = dispatcher();
    press(&d, &mut state, "4");
    d.set_keymap(keymap());
    d.sync(&mut state);
    assert_eq!(state.count(), None);
    assert_eq!(state.mode(), &ModeId::from("normal"));
    press(&d, &mut state, "2 d 5");
    d.set_keymap(keymap());
    assert_eq!(last(&d, &mut state, "w"), (runs("t.word"), None));
}

#[test]
fn huge_counts_saturate() {
    let (d, mut state) = dispatcher();
    let nines = vec!["9"; 40].join(" ");
    press(&d, &mut state, &nines);
    assert_eq!(state.count(), Some(usize::MAX));
    assert_eq!(
        last(&d, &mut state, "w"),
        (runs("t.word"), Some(usize::MAX))
    );
    assert_eq!(
        last(&d, &mut state, &format!("{nines} d 2 w")),
        (runs_operator("t.delete", "t.word"), Some(usize::MAX))
    );
}

#[test]
fn dispatch_passes_the_count_to_the_command() {
    let (d, mut state) = dispatcher();
    let (mut set, mut editor) = setup("");
    for keys in ["t", "1 2 t", "3 q", "t"] {
        for &k in KeySequence::parse(keys).unwrap().as_slice() {
            d.dispatch(&mut state, &KeyEvent::new(k), &[], &mut editor, &mut set)
                .expect("t.count runs; the rest aren't run");
        }
    }
    assert_eq!(text(&set, &editor), "none;12;none;");
}

#[test]
fn run_counted_passes_the_count_and_run_passes_none() {
    let registry = registry();
    let (mut set, mut editor) = setup("");
    let invocation = Invocation::new("t.count");
    run_counted(&registry, &invocation, Some(7), &mut editor, &mut set).unwrap();
    registry.run(&invocation, &mut editor, &mut set).unwrap();
    run_counted(&registry, &invocation, None, &mut editor, &mut set).unwrap();
    assert_eq!(text(&set, &editor), "7;none;none;");
    assert_eq!(
        run_counted(
            &registry,
            &Invocation::new("t.missing"),
            Some(2),
            &mut editor,
            &mut set
        ),
        Err(CommandError::UnknownCommand {
            id: CommandId::from("t.missing")
        })
    );
}
