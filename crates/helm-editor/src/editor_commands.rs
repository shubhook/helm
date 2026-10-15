//! The ids [`CommandRegistry::with_editor_commands`] registers, one per
//! [`Editor`](crate::Editor) command. Keymaps (T-107, T-108) bind these.
//!
//! Each runs the `Editor` method of the same name. The selection commands
//! ([`ADD_NEXT_MATCH`] and the three after it) don't edit, so they read
//! the set through the context's `&mut BufferSet` without changing it.

use crate::{Command, CommandContext, CommandError, CommandFn, CommandId, CommandRegistry};

/// [`Editor::insert`](crate::Editor::insert) with
/// [`CommandArgs::text`](crate::CommandArgs::text). Keys no binding matches
/// type through this command.
pub const INSERT: &str = "editor.insert";
/// [`Editor::newline`](crate::Editor::newline).
pub const NEWLINE: &str = "editor.newline";
/// [`Editor::delete_backward`](crate::Editor::delete_backward).
pub const DELETE_BACKWARD: &str = "editor.delete_backward";
/// [`Editor::delete_forward`](crate::Editor::delete_forward).
pub const DELETE_FORWARD: &str = "editor.delete_forward";
/// [`Editor::add_next_match`](crate::Editor::add_next_match).
pub const ADD_NEXT_MATCH: &str = "editor.add_next_match";
/// [`Editor::select_all_matches`](crate::Editor::select_all_matches).
pub const SELECT_ALL_MATCHES: &str = "editor.select_all_matches";
/// [`Editor::add_cursor_above`](crate::Editor::add_cursor_above).
pub const ADD_CURSOR_ABOVE: &str = "editor.add_cursor_above";
/// [`Editor::add_cursor_below`](crate::Editor::add_cursor_below).
pub const ADD_CURSOR_BELOW: &str = "editor.add_cursor_below";
/// [`Editor::undo`](crate::Editor::undo).
pub const UNDO: &str = "editor.undo";
/// [`Editor::redo`](crate::Editor::redo).
pub const REDO: &str = "editor.redo";

/// Every id above, sorted.
pub const ALL: [&str; 10] = [
    ADD_CURSOR_ABOVE,
    ADD_CURSOR_BELOW,
    ADD_NEXT_MATCH,
    DELETE_BACKWARD,
    DELETE_FORWARD,
    INSERT,
    NEWLINE,
    REDO,
    SELECT_ALL_MATCHES,
    UNDO,
];

/// Registers one command for each id in [`ALL`].
pub(crate) fn register_all(registry: &mut CommandRegistry) {
    let commands: [(&str, &'static str, CommandFn); 10] = [
        (INSERT, "Insert text", insert),
        (NEWLINE, "Insert line break", |cx| {
            cx.editor.newline(cx.set)?;
            Ok(())
        }),
        (DELETE_BACKWARD, "Delete backward", |cx| {
            cx.editor.delete_backward(cx.set)?;
            Ok(())
        }),
        (DELETE_FORWARD, "Delete forward", |cx| {
            cx.editor.delete_forward(cx.set)?;
            Ok(())
        }),
        (ADD_NEXT_MATCH, "Add selection to next match", |cx| {
            cx.editor.add_next_match(cx.set)?;
            Ok(())
        }),
        (SELECT_ALL_MATCHES, "Select all matches", |cx| {
            cx.editor.select_all_matches(cx.set)?;
            Ok(())
        }),
        (ADD_CURSOR_ABOVE, "Add cursor above", |cx| {
            cx.editor.add_cursor_above(cx.set)?;
            Ok(())
        }),
        (ADD_CURSOR_BELOW, "Add cursor below", |cx| {
            cx.editor.add_cursor_below(cx.set)?;
            Ok(())
        }),
        (UNDO, "Undo", |cx| {
            cx.editor.undo(cx.set)?;
            Ok(())
        }),
        (REDO, "Redo", |cx| {
            cx.editor.redo(cx.set)?;
            Ok(())
        }),
    ];
    for (id, title, run) in commands {
        registry
            .register(Command {
                id: CommandId::from(id),
                title,
                run,
            })
            .expect("editor command ids are valid and distinct");
    }
}

/// [`INSERT`]: errors with [`CommandError::MissingText`] when the
/// invocation has no text.
fn insert(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let text = cx
        .args
        .text
        .as_deref()
        .ok_or_else(|| CommandError::MissingText {
            id: CommandId::from(INSERT),
        })?;
    cx.editor.insert(cx.set, text)?;
    Ok(())
}
