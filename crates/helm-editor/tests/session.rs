//! T-109 acceptance tests, with no Vim: a pending register
//! (`Action::Register`), the command line (`Typing::Line`), requests from
//! `dispatch_in` and `run_in`, change marks, and the record `.` repeats, on
//! a small keymap and test commands.

use helm_editor::{
    Binding, Command, CommandContext, CommandError, CommandId, CommandRegistry, Dispatcher, Editor,
    Invocation, Key, KeyEvent, Keymap, Keystroke, Mode, Modifiers, Request, Resolution, Selection,
    Selections, Session, Typing,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

fn ok(_: CommandContext<'_>) -> Result<(), CommandError> {
    Ok(())
}

/// Pushes two requests.
fn ask(cx: CommandContext<'_>) -> Result<(), CommandError> {
    cx.requests.push(Request::Close { force: false });
    cx.requests.push(Request::SetClipboard {
        text: cx.args.text.clone().unwrap_or_default(),
    });
    Ok(())
}

/// Runs `test.ask` through the registry it was given, then pushes one more.
fn nest(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let inner = cx
        .registry
        .run_in(&Invocation::new("test.ask"), cx.editor, cx.set, cx.session)?;
    cx.requests.extend(inner);
    cx.requests.push(Request::Close { force: true });
    Ok(())
}

/// Always fails.
fn fail(cx: CommandContext<'_>) -> Result<(), CommandError> {
    Err(CommandError::MissingText {
        id: CommandId::from(cx.args.motion.as_ref().map_or("test.fail", |m| m.as_str())),
    })
}

fn begin(cx: CommandContext<'_>) -> Result<(), CommandError> {
    cx.editor.begin_undo_group();
    Ok(())
}

fn end(cx: CommandContext<'_>) -> Result<(), CommandError> {
    cx.editor.end_undo_group();
    Ok(())
}

fn type_x(cx: CommandContext<'_>) -> Result<(), CommandError> {
    cx.editor.insert(cx.set, "x")?;
    Ok(())
}

fn registry() -> CommandRegistry {
    let mut registry = CommandRegistry::new();
    for (id, run) in [
        (
            "test.ok",
            ok as fn(CommandContext<'_>) -> Result<(), CommandError>,
        ),
        ("test.ask", ask),
        ("test.nest", nest),
        ("test.fail", fail),
        ("test.begin", begin),
        ("test.end", end),
        ("test.x", type_x),
        ("test.op", ok),
    ] {
        registry
            .register(Command {
                id: CommandId::from(id),
                title: "Test",
                run,
            })
            .unwrap();
    }
    registry
}

/// Normal (counting), insert, and a command line.
fn keymap() -> Keymap {
    let mut keymap = Keymap::new(
        "test",
        vec![
            Mode::new("normal", Typing::Count),
            Mode::new("insert", Typing::Insert),
            Mode::new("line", Typing::Line),
        ],
    )
    .unwrap();
    for binding in [
        Binding::register("\" a", 'a').unwrap().in_mode("normal"),
        Binding::register("\" b", 'b').unwrap().in_mode("normal"),
        Binding::run("x", "test.ok").unwrap().in_mode("normal"),
        Binding::operator("d", "test.op").unwrap().in_mode("normal"),
        Binding::run("w", "test.ok").unwrap().in_mode("normal"),
        Binding::switch_mode(":", "line").unwrap().in_mode("normal"),
        Binding::switch_mode("i", "insert")
            .unwrap()
            .in_mode("normal"),
        Binding::run("enter", "test.ask")
            .unwrap()
            .in_mode("line")
            .then_mode("normal"),
        Binding::switch_mode("escape", "normal")
            .unwrap()
            .in_mode("line"),
    ] {
        keymap.bind(binding).unwrap();
    }
    keymap
}

fn key(c: char) -> KeyEvent {
    KeyEvent::new(Keystroke::new(Key::Char(c), Modifiers::NONE))
}

fn named(key: Key) -> KeyEvent {
    KeyEvent::new(Keystroke::new(key, Modifiers::NONE))
}

fn editor(text: &str) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    editor.set_selections(Selections::new(vec![Selection::cursor(0)], 0).unwrap());
    (set, editor)
}

#[test]
fn a_register_waits_for_the_next_run() {
    let d = Dispatcher::new(registry(), keymap());
    let mut state = d.key_state();
    assert_eq!(state.register(), None);
    assert_eq!(d.resolve(&mut state, &key('"'), &[]), Resolution::Pending);
    assert_eq!(state.register(), None);
    assert_eq!(d.resolve(&mut state, &key('a'), &[]), Resolution::Pending);
    assert_eq!(state.register(), Some('a'));
    // A second name replaces the first.
    d.resolve(&mut state, &key('"'), &[]);
    d.resolve(&mut state, &key('b'), &[]);
    assert_eq!(state.register(), Some('b'));
    assert!(matches!(
        d.resolve(&mut state, &key('x'), &[]),
        Resolution::Run(_)
    ));
    assert_eq!(state.register(), None);
}

#[test]
fn a_register_and_a_count_go_in_either_order() {
    let d = Dispatcher::new(registry(), keymap());
    for keys in ["2\"ax", "\"a2x"] {
        let mut state = d.key_state();
        let mut last = Resolution::Unbound;
        for c in keys.chars() {
            last = d.resolve(&mut state, &key(c), &[]);
        }
        let Resolution::Run(invocation) = last else {
            panic!("{keys}: {last:?}");
        };
        assert_eq!(invocation.args.count, Some(2), "{keys}");
        assert_eq!(state.register(), None, "{keys}");
        assert_eq!(state.count(), None, "{keys}");
    }
}

#[test]
fn a_register_typed_before_an_operator_waits_for_it() {
    let d = Dispatcher::new(registry(), keymap());
    let mut state = d.key_state();
    for c in ['"', 'a', 'd'] {
        assert_eq!(d.resolve(&mut state, &key(c), &[]), Resolution::Pending);
    }
    assert_eq!(state.register(), Some('a'));
    assert!(matches!(
        d.resolve(&mut state, &key('w'), &[]),
        Resolution::Run(_)
    ));
    assert_eq!(state.register(), None);
}

#[test]
fn a_pending_register_drops_where_a_count_drops() {
    let d = Dispatcher::new(registry(), keymap());
    // An unbound key, and Escape, which is unbound in normal mode.
    for event in [key('q'), named(Key::Escape)] {
        let mut state = d.key_state();
        d.resolve(&mut state, &key('"'), &[]);
        d.resolve(&mut state, &key('a'), &[]);
        assert_eq!(d.resolve(&mut state, &event, &[]), Resolution::Cancelled);
        assert_eq!(state.register(), None);
    }
    // A broken chord.
    let mut state = d.key_state();
    d.resolve(&mut state, &key('"'), &[]);
    assert_eq!(d.resolve(&mut state, &key('z'), &[]), Resolution::Cancelled);
    // A binding with no action.
    let mut state = d.key_state();
    d.resolve(&mut state, &key('"'), &[]);
    d.resolve(&mut state, &key('a'), &[]);
    assert_eq!(d.resolve(&mut state, &key('i'), &[]), Resolution::Done);
    assert_eq!(state.register(), None);
    // `cancel`.
    let mut state = d.key_state();
    d.resolve(&mut state, &key('"'), &[]);
    d.resolve(&mut state, &key('a'), &[]);
    assert!(state.cancel());
    assert_eq!(state.register(), None);
    assert!(!state.cancel());
    // A register name with an operator pending cancels the operator.
    let mut state = d.key_state();
    d.resolve(&mut state, &key('d'), &[]);
    d.resolve(&mut state, &key('"'), &[]);
    assert_eq!(d.resolve(&mut state, &key('a'), &[]), Resolution::Cancelled);
    assert_eq!(state.operator(), None);
    // A keymap switch.
    let mut d = Dispatcher::new(registry(), keymap());
    let mut state = d.key_state();
    d.resolve(&mut state, &key('"'), &[]);
    d.resolve(&mut state, &key('a'), &[]);
    d.set_keymap(keymap());
    d.sync(&mut state);
    assert_eq!(state.register(), None);
}

#[test]
fn the_command_line_collects_typed_text() {
    let d = Dispatcher::new(registry(), keymap());
    let mut state = d.key_state();
    assert_eq!(state.line(), None);
    assert_eq!(d.resolve(&mut state, &key(':'), &[]), Resolution::Done);
    assert_eq!(state.mode().as_str(), "line");
    assert_eq!(state.line(), Some(""));
    for c in "w é😀".chars() {
        assert_eq!(d.resolve(&mut state, &key(c), &[]), Resolution::Pending);
    }
    assert_eq!(state.line(), Some("w é😀"));
    assert_eq!(
        d.resolve(&mut state, &named(Key::Backspace), &[]),
        Resolution::Pending
    );
    assert_eq!(state.line(), Some("w é"));
    // Ctrl and Cmd type nothing.
    let ctrl = KeyEvent::new(Keystroke::new(Key::Char('a'), Modifiers::CTRL));
    assert_eq!(d.resolve(&mut state, &ctrl, &[]), Resolution::Unbound);
    assert_eq!(state.line(), Some("w é"));
    // Other unbound keys do nothing either.
    assert_eq!(
        d.resolve(&mut state, &named(Key::Left), &[]),
        Resolution::Unbound
    );
    let mut expected = Invocation::new("test.ask");
    expected.args.text = Some("w é".to_owned());
    assert_eq!(
        d.resolve(&mut state, &named(Key::Enter), &[]),
        Resolution::Run(expected)
    );
    assert_eq!(state.mode().as_str(), "normal");
    assert_eq!(state.line(), None);
}

#[test]
fn leaving_the_command_line_drops_it() {
    let mut d = Dispatcher::new(registry(), keymap());
    // Escape.
    let mut state = d.key_state();
    for c in [':', 'q'] {
        d.resolve(&mut state, &key(c), &[]);
    }
    assert_eq!(
        d.resolve(&mut state, &named(Key::Escape), &[]),
        Resolution::Done
    );
    assert_eq!(state.line(), None);
    d.resolve(&mut state, &key(':'), &[]);
    assert_eq!(state.line(), Some(""));
    // Backspace on an empty line goes back to where the line began.
    assert_eq!(
        d.resolve(&mut state, &named(Key::Backspace), &[]),
        Resolution::Done
    );
    assert_eq!(state.mode().as_str(), "normal");
    assert_eq!(state.line(), None);
    // `cancel` and a keymap switch go back too.
    d.resolve(&mut state, &key(':'), &[]);
    d.resolve(&mut state, &key('q'), &[]);
    assert!(state.cancel());
    assert_eq!(state.mode().as_str(), "normal");
    assert_eq!(state.line(), None);
    d.resolve(&mut state, &key(':'), &[]);
    d.set_keymap(keymap());
    d.sync(&mut state);
    assert_eq!(state.mode().as_str(), "normal");
    assert_eq!(state.line(), None);
}

#[test]
fn dispatch_in_returns_requests_and_dispatch_drops_them() {
    let d = Dispatcher::new(registry(), keymap());
    let (mut set, mut e) = editor("ab");
    let mut session = Session::new();
    let mut state = d.key_state();
    for c in [':', 'h'] {
        let (resolution, requests) = d
            .dispatch_in(&mut state, &key(c), &[], &mut e, &mut set, &mut session)
            .unwrap();
        assert_ne!(resolution, Resolution::Unbound);
        assert!(requests.is_empty());
    }
    let (resolution, requests) = d
        .dispatch_in(
            &mut state,
            &named(Key::Enter),
            &[],
            &mut e,
            &mut set,
            &mut session,
        )
        .unwrap();
    assert!(matches!(resolution, Resolution::Run(_)));
    assert_eq!(
        requests,
        vec![
            Request::Close { force: false },
            Request::SetClipboard {
                text: "h".to_owned()
            }
        ]
    );
    // The same keys through `dispatch` run the command and drop them.
    let mut state = d.key_state();
    for event in [key(':'), key('h'), named(Key::Enter)] {
        d.dispatch(&mut state, &event, &[], &mut e, &mut set)
            .unwrap();
    }
    assert_eq!(state.mode().as_str(), "normal");
}

#[test]
fn run_in_passes_the_registry_and_the_session() {
    let registry = registry();
    let (mut set, mut e) = editor("ab");
    let mut session = Session::new();
    let requests = registry
        .run_in(
            &Invocation::new("test.nest"),
            &mut e,
            &mut set,
            &mut session,
        )
        .unwrap();
    assert_eq!(
        requests,
        vec![
            Request::Close { force: false },
            Request::SetClipboard {
                text: String::new()
            },
            Request::Close { force: true },
        ]
    );
    assert_eq!(
        registry.run_in(
            &Invocation::new("test.fail"),
            &mut e,
            &mut set,
            &mut session
        ),
        Err(CommandError::MissingText {
            id: CommandId::from("test.fail")
        })
    );
    assert_eq!(
        registry.run_in(
            &Invocation::new("test.none"),
            &mut e,
            &mut set,
            &mut session
        ),
        Err(CommandError::UnknownCommand {
            id: CommandId::from("test.none")
        })
    );
}

#[test]
fn marks_say_which_commands_are_changes() {
    let mut registry = registry();
    assert!(!registry.is_change("test.x"));
    registry.mark_change("test.x");
    registry.mark_change("test.x");
    registry.mark_change("test.unregistered");
    assert!(registry.is_change("test.x"));
    assert!(registry.is_change("test.unregistered"));
    assert!(!registry.is_change("test.ok"));
}

#[test]
fn a_change_is_recorded_as_its_invocations() {
    let mut registry = registry();
    registry.mark_change("test.x");
    registry.mark_change("test.begin");
    let (mut set, mut e) = editor("ab");
    let mut session = Session::new();
    let run = |id: &str, e: &mut Editor, set: &mut BufferSet, session: &mut Session| {
        let mut invocation = Invocation::new(id);
        invocation.args.count = Some(2);
        registry.run_in(&invocation, e, set, session).unwrap();
    };
    assert_eq!(session.last_change(), None);
    // Unmarked commands record nothing.
    run("test.ok", &mut e, &mut set, &mut session);
    assert_eq!(session.last_change(), None);
    // A marked command with no group open is the whole change.
    run("test.x", &mut e, &mut set, &mut session);
    let ids = |s: &Session| -> Vec<String> {
        s.last_change()
            .unwrap()
            .iter()
            .map(|i| i.command.to_string())
            .collect()
    };
    assert_eq!(ids(&session), ["test.x"]);
    assert_eq!(session.last_change().unwrap()[0].args.count, Some(2));
    // A marked command that opens a group records until it closes.
    run("test.begin", &mut e, &mut set, &mut session);
    run("test.x", &mut e, &mut set, &mut session);
    run("test.ok", &mut e, &mut set, &mut session);
    assert_eq!(ids(&session), ["test.x"]);
    run("test.end", &mut e, &mut set, &mut session);
    assert_eq!(
        ids(&session),
        ["test.begin", "test.x", "test.ok", "test.end"]
    );
    // A failed command isn't recorded.
    let _ = registry.run_in(
        &Invocation::new("test.fail"),
        &mut e,
        &mut set,
        &mut session,
    );
    assert_eq!(ids(&session).len(), 4);
}

#[test]
fn a_group_closed_from_outside_ends_the_record_at_the_next_run() {
    let mut registry = registry();
    registry.mark_change("test.begin");
    let (mut set, mut e) = editor("ab");
    let mut session = Session::new();
    registry
        .run_in(
            &Invocation::new("test.begin"),
            &mut e,
            &mut set,
            &mut session,
        )
        .unwrap();
    registry
        .run_in(&Invocation::new("test.x"), &mut e, &mut set, &mut session)
        .unwrap();
    e.end_undo_group();
    assert_eq!(session.last_change(), None);
    registry
        .run_in(&Invocation::new("test.ok"), &mut e, &mut set, &mut session)
        .unwrap();
    let ids: Vec<&str> = session
        .last_change()
        .unwrap()
        .iter()
        .map(|i| i.command.as_str())
        .collect();
    assert_eq!(ids, ["test.begin", "test.x"]);
}
