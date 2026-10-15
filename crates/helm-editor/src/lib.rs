//! Editor logic: cursors, multi-cursor, selections, commands and the keymap with VS Code and Vim modes.
//!
//! # Selections
//!
//! A [`Selection`] is an anchor and a head, both byte offsets into a
//! buffer's text. The head is where the cursor is drawn and the end that
//! moves when a selection grows; the anchor stays put. A cursor is a
//! selection whose anchor equals its head. A selection can point either
//! way: `head < anchor` is a selection made leftwards.
//!
//! [`Selections`] holds one or more selections for one editor, always kept
//! in a normal form:
//!
//! - Sorted by start. No two selections overlap.
//! - Two selections merge when they overlap, or when they touch and at
//!   least one of them is empty. So two cursors at one offset become one
//!   cursor, and a cursor at either end of a selection joins it. Two
//!   non-empty selections that only touch, such as `0..1` and `1..2`, stay
//!   apart, so selecting every `a` in `"aaa"` gives three selections.
//! - One selection is the primary: the one the next "add" command starts
//!   from, and the one a later scroll-into-view follows.
//!
//! # Editing at every cursor
//!
//! An [`Editor`] edits one buffer of a [`BufferSet`](helm_text::BufferSet).
//! Each editing command builds one edit per selection and applies them all
//! as one [`Transaction`](helm_text::Transaction), so a keystroke at a
//! thousand cursors is one undo step. Every edit's range refers to the text
//! before the command, as `BufferSet::apply` requires. [`Editor`] documents
//! where each selection lands afterwards.
//!
//! A new line is [`FileFormat::newline`](helm_text::FileFormat::newline) of
//! the editor's format, so a CRLF file gets `"\r\n"`. The editor never
//! writes a hard-coded `"\n"`.
//!
//! # Undo and redo
//!
//! [`Editor::undo`] and [`Editor::redo`] go through the buffer set, then put
//! back the selections the editor had just before or just after the command
//! that made the transaction. Commands that only move or add selections are
//! not transactions and are not undone.
//!
//! # Commands
//!
//! Every command has a stable string id, a [`CommandId`] such as
//! `"editor.delete_backward"`, and runs against one [`Editor`] and the
//! [`BufferSet`](helm_text::BufferSet) that owns its buffer. A
//! [`CommandRegistry`] maps ids to commands.
//! [`CommandRegistry::with_editor_commands`] registers the T-105 `Editor`
//! commands under the ids in [`editor_commands`], and
//! [`CommandRegistry::with_all_commands`] adds the cursor commands in
//! [`cursor_commands`]: moving, selecting and deleting by a [`Motion`].
//!
//! # Keys, keymaps and modes
//!
//! `helm-ui` turns each platform key press into a [`KeyEvent`]: a
//! [`Keystroke`] (a [`Key`] and its [`Modifiers`], Cmd included) and the
//! text it types. No GPUI type gets this far down.
//!
//! A [`Keymap`] is data: a list of [`Mode`]s and a list of [`Binding`]s
//! from a [`KeySequence`] (one keystroke, or a chord such as
//! `cmd-k cmd-s`) to a command id, each scoped to a mode and an optional
//! context. VS Code's keymap ([`vscode::keymap`], T-107) has one mode;
//! Vim's (T-108) has several. A mode says whether a key no binding matches types its text,
//! so typing takes the same path as every other command.
//!
//! A [`Dispatcher`] holds the active keymap and the registry, and each
//! editor holds a [`KeyState`]: its mode, a chord typed so far and a
//! pending operator. [`Dispatcher::resolve`] turns one key event into a
//! [`Resolution`] and documents which binding wins.
//! [`Dispatcher::set_keymap`] switches keymaps at runtime, and every open
//! editor uses the new keymap from its next key (ED-6).

mod command;
pub mod cursor_commands;
mod dispatch;
mod editor;
pub mod editor_commands;
mod error;
mod key;
mod keymap;
mod motion;
mod selection;
pub mod vscode;

pub use command::{
    Command, CommandArgs, CommandContext, CommandError, CommandFn, CommandId, CommandRegistry,
    Invocation, RegisterError,
};
pub use dispatch::{Dispatcher, KeyState, Resolution};
pub use editor::Editor;
pub use error::EditorError;
pub use key::{Key, KeyEvent, KeyParseError, KeySequence, Keystroke, Modifiers};
pub use keymap::{Action, Binding, Keymap, KeymapError, Mode, ModeId, Typing};
pub use motion::Motion;
pub use selection::{Selection, Selections};
