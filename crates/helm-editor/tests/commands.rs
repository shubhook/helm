//! T-106 acceptance tests: the command registry, its ids, and running
//! every `Editor` command through it.

use helm_editor::{
    Command, CommandArgs, CommandContext, CommandError, CommandId, CommandRegistry, Editor,
    EditorError, Invocation, RegisterError, Selection, Selections, editor_commands,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// A set with one buffer holding `text`, and an editor on it with the
/// file's format and the given selections.
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

/// The text of the editor's buffer.
fn text(set: &BufferSet, editor: &Editor) -> String {
    set.get(editor.buffer())
        .expect("buffer is in the set")
        .to_string()
}

/// A command that does nothing.
fn nothing(_: CommandContext<'_>) -> Result<(), CommandError> {
    Ok(())
}

/// A command with id `id` that does nothing.
fn noop(id: &str) -> Command {
    Command {
        id: CommandId::from(id),
        title: "Do nothing",
        run: nothing,
    }
}

/// A command that types its motion's id, to show what `run` passes on.
fn type_motion(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let motion = cx.args.motion.as_ref().map_or("none", CommandId::as_str);
    cx.editor.insert(cx.set, motion)?;
    Ok(())
}

#[test]
fn command_ids_compare_as_text() {
    let id = CommandId::from("editor.undo");
    assert_eq!(id.as_str(), "editor.undo");
    assert_eq!(id.to_string(), "editor.undo");
    assert_eq!(id, CommandId::from("editor.undo".to_owned()));
    assert_ne!(id, CommandId::from("editor.redo"));
    assert!(CommandId::from("a.b") < CommandId::from("a.c"));
}

#[test]
fn editor_command_ids_are_the_documented_ones() {
    assert_eq!(
        editor_commands::ALL,
        [
            "editor.add_cursor_above",
            "editor.add_cursor_below",
            "editor.add_next_match",
            "editor.delete_backward",
            "editor.delete_forward",
            "editor.insert",
            "editor.newline",
            "editor.redo",
            "editor.select_all_matches",
            "editor.undo",
        ]
    );
    // Calls into the registry so the test runs against the implementation.
    assert!(CommandRegistry::new().commands().next().is_none());
}

#[test]
fn registers_every_editor_command_sorted_with_a_title() {
    let registry = CommandRegistry::with_editor_commands();
    let ids: Vec<&str> = registry.commands().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, editor_commands::ALL);
    for id in editor_commands::ALL {
        let command = registry.get(id).expect("registered");
        assert_eq!(command.id.as_str(), id);
        assert!(!command.title.is_empty(), "{id} has a title");
    }
    assert!(registry.get("editor.nope").is_none());
}

#[test]
fn register_checks_the_id() {
    let mut registry = CommandRegistry::new();
    for id in ["a", "editor.x", "vim.delete_line", "a1.b_2.c3", "_"] {
        assert_eq!(registry.register(noop(id)), Ok(()), "{id}");
    }
    for id in [
        "",
        ".",
        "a.",
        ".a",
        "a..b",
        "Editor.x",
        "editor.x-y",
        "editor x",
        "é",
    ] {
        assert_eq!(
            registry.register(noop(id)),
            Err(RegisterError::InvalidId {
                id: CommandId::from(id)
            }),
            "{id:?}"
        );
    }
    assert_eq!(
        registry.register(noop("editor.x")),
        Err(RegisterError::Duplicate {
            id: CommandId::from("editor.x")
        })
    );
    let ids: Vec<&str> = registry.commands().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["_", "a", "a1.b_2.c3", "editor.x", "vim.delete_line"]);
}

#[test]
fn editor_commands_cannot_be_registered_twice() {
    let mut registry = CommandRegistry::with_editor_commands();
    assert_eq!(
        registry.register(noop(editor_commands::UNDO)),
        Err(RegisterError::Duplicate {
            id: CommandId::from(editor_commands::UNDO)
        })
    );
}

#[test]
fn each_editing_command_runs_its_editor_method() {
    let registry = CommandRegistry::with_editor_commands();
    let (mut set, mut editor) = setup("ab\r\ncd", &[Selection::cursor(1)]);

    registry
        .run(&Invocation::insert("X"), &mut editor, &mut set)
        .unwrap();
    assert_eq!(text(&set, &editor), "aXb\r\ncd");

    let run = |id: &str, editor: &mut Editor, set: &mut BufferSet| {
        registry.run(&Invocation::new(id), editor, set).unwrap();
    };
    run(editor_commands::NEWLINE, &mut editor, &mut set);
    assert_eq!(text(&set, &editor), "aX\r\nb\r\ncd");
    run(editor_commands::DELETE_BACKWARD, &mut editor, &mut set);
    assert_eq!(text(&set, &editor), "aXb\r\ncd");
    run(editor_commands::DELETE_FORWARD, &mut editor, &mut set);
    assert_eq!(text(&set, &editor), "aX\r\ncd");
    assert_eq!(editor.selections().as_slice(), &[Selection::cursor(2)]);

    run(editor_commands::UNDO, &mut editor, &mut set);
    assert_eq!(text(&set, &editor), "aXb\r\ncd");
    run(editor_commands::REDO, &mut editor, &mut set);
    assert_eq!(text(&set, &editor), "aX\r\ncd");
}

#[test]
fn each_selection_command_runs_its_editor_method() {
    let registry = CommandRegistry::with_editor_commands();
    let run = |id: &str, editor: &mut Editor, set: &mut BufferSet| {
        registry.run(&Invocation::new(id), editor, set).unwrap();
    };

    let (mut set, mut editor) = setup("ab ab ab", &[Selection::cursor(0)]);
    run(editor_commands::ADD_NEXT_MATCH, &mut editor, &mut set);
    assert_eq!(editor.selections().as_slice(), &[Selection::new(0, 2)]);
    run(editor_commands::ADD_NEXT_MATCH, &mut editor, &mut set);
    assert_eq!(
        editor.selections().as_slice(),
        &[Selection::new(0, 2), Selection::new(3, 5)]
    );
    run(editor_commands::SELECT_ALL_MATCHES, &mut editor, &mut set);
    assert_eq!(editor.selections().as_slice().len(), 3);
    assert_eq!(text(&set, &editor), "ab ab ab");

    let (mut set, mut editor) = setup("abc\nabc\nabc", &[Selection::cursor(5)]);
    run(editor_commands::ADD_CURSOR_ABOVE, &mut editor, &mut set);
    assert_eq!(
        editor.selections().as_slice(),
        &[Selection::cursor(1), Selection::cursor(5)]
    );
    run(editor_commands::ADD_CURSOR_BELOW, &mut editor, &mut set);
    assert_eq!(
        editor.selections().as_slice(),
        &[
            Selection::cursor(1),
            Selection::cursor(5),
            Selection::cursor(9)
        ]
    );
}

#[test]
fn insert_without_text_is_an_error() {
    let registry = CommandRegistry::with_editor_commands();
    let (mut set, mut editor) = setup("ab", &[Selection::cursor(1)]);
    assert_eq!(
        registry.run(
            &Invocation::new(editor_commands::INSERT),
            &mut editor,
            &mut set
        ),
        Err(CommandError::MissingText {
            id: CommandId::from(editor_commands::INSERT)
        })
    );
    assert_eq!(text(&set, &editor), "ab");
}

#[test]
fn unknown_commands_and_editor_errors_are_reported() {
    let registry = CommandRegistry::with_editor_commands();
    let (mut set, mut editor) = setup("ab", &[Selection::cursor(1)]);
    assert_eq!(
        registry.run(&Invocation::new("workspace.save"), &mut editor, &mut set),
        Err(CommandError::UnknownCommand {
            id: CommandId::from("workspace.save")
        })
    );

    // An editor whose buffer isn't in the set.
    let mut other = BufferSet::new();
    let error = registry
        .run(&Invocation::insert("x"), &mut editor, &mut other)
        .unwrap_err();
    assert_eq!(
        error,
        CommandError::Editor(EditorError::UnknownBuffer {
            buffer: editor.buffer()
        })
    );
    assert_eq!(
        CommandError::from(EditorError::UnknownBuffer {
            buffer: editor.buffer()
        }),
        error
    );
    assert_eq!(text(&set, &editor), "ab");
}

#[test]
fn run_passes_the_invocation_args() {
    let mut registry = CommandRegistry::new();
    registry
        .register(Command {
            id: CommandId::from("test.type_motion"),
            title: "Type the motion",
            run: type_motion,
        })
        .unwrap();
    let (mut set, mut editor) = setup("", &[Selection::cursor(0)]);
    let invocation = Invocation {
        command: CommandId::from("test.type_motion"),
        args: CommandArgs {
            text: None,
            motion: Some(CommandId::from("vim.word")),
        },
    };
    registry.run(&invocation, &mut editor, &mut set).unwrap();
    assert_eq!(text(&set, &editor), "vim.word");
}

#[test]
fn invocation_constructors() {
    assert_eq!(
        Invocation::new("editor.undo"),
        Invocation {
            command: CommandId::from("editor.undo"),
            args: CommandArgs::default(),
        }
    );
    assert_eq!(
        Invocation::insert("hé"),
        Invocation {
            command: CommandId::from(editor_commands::INSERT),
            args: CommandArgs {
                text: Some("hé".to_owned()),
                motion: None,
            },
        }
    );
}
