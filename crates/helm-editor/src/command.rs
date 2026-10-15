//! The command registry: every command has a stable string id, and runs
//! against one [`Editor`] and the [`BufferSet`] that owns its buffer.

#![expect(
    unused_variables,
    dead_code,
    reason = "T-106 interface stubs; the implementation deletes this"
)]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use helm_text::BufferSet;

use crate::{Editor, EditorError};

/// A command's id, such as `"editor.delete_backward"`. Keymaps, the
/// keybindings file and the command palette name commands by id, so an id
/// never changes once a command ships.
///
/// Any string converts to an id. [`CommandRegistry::register`] checks the
/// form: one or more segments separated by `.`, each segment one or more
/// of `a` to `z`, `0` to `9` and `_`. The first segment names the crate
/// area that owns the command: `editor.` for [`Editor`] commands.
///
/// Clones are cheap: the string is shared.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(Arc<str>);

impl CommandId {
    /// The id as text.
    pub fn as_str(&self) -> &str {
        todo!()
    }
}

impl From<&str> for CommandId {
    fn from(id: &str) -> Self {
        todo!()
    }
}

impl From<String> for CommandId {
    fn from(id: String) -> Self {
        todo!()
    }
}

impl fmt::Display for CommandId {
    /// Writes the id as it is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

/// What a command gets besides the editor: the text to type, or the motion
/// a pending operator runs over. A command ignores what it doesn't use.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandArgs {
    /// The text [`editor_commands::INSERT`](crate::editor_commands::INSERT)
    /// types. A key no binding matches arrives here (see
    /// [`Dispatcher::resolve`](crate::Dispatcher::resolve)).
    pub text: Option<String>,
    /// For an operator, the motion it applies to: the command a key bound
    /// after the operator named (see [`Action::Operator`](crate::Action)).
    /// What an operator does with it is up to the operator (T-108).
    pub motion: Option<CommandId>,
}

/// One command to run, with its arguments.
/// [`Dispatcher::resolve`](crate::Dispatcher::resolve) returns these, and
/// [`CommandRegistry::run`] runs them.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Invocation {
    /// The command to run.
    pub command: CommandId,
    /// Its arguments.
    pub args: CommandArgs,
}

impl Invocation {
    /// `command` with no arguments.
    pub fn new(command: impl Into<CommandId>) -> Self {
        todo!()
    }

    /// The insert command,
    /// [`editor_commands::INSERT`](crate::editor_commands::INSERT), typing
    /// `text`.
    pub fn insert(text: impl Into<String>) -> Self {
        todo!()
    }
}

/// What a command runs against.
pub struct CommandContext<'a> {
    /// The editor the key went to.
    pub editor: &'a mut Editor,
    /// The set that owns the editor's buffer.
    pub set: &'a mut BufferSet,
    /// The invocation's arguments.
    pub args: &'a CommandArgs,
}

/// The function a command runs. Plain function pointers keep commands
/// `Send`, `Sync` and cheap to copy, and hold no state of their own.
pub type CommandFn = fn(CommandContext<'_>) -> Result<(), CommandError>;

/// A command: its id, the title the command palette shows (ST-3), and the
/// function that runs it.
#[derive(Clone, Debug)]
pub struct Command {
    /// The stable id.
    pub id: CommandId,
    /// What the command palette shows, in sentence case: "Delete backward".
    pub title: &'static str,
    /// What runs.
    pub run: CommandFn,
}

/// Every command `helm-editor` can run, by id.
///
/// The registry doesn't hold every id a keymap may name. Commands that need
/// more than an editor and a buffer set, such as saving a file, belong to
/// `helm-ui`, which runs the ids it owns itself and hands the rest here.
#[derive(Clone, Debug, Default)]
pub struct CommandRegistry {
    commands: BTreeMap<CommandId, Command>,
}

impl CommandRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        todo!()
    }

    /// A registry holding one command for each [`Editor`] command, under
    /// the ids in [`editor_commands`](crate::editor_commands). Each runs
    /// the `Editor` method of the same name on the context's editor and
    /// set. [`INSERT`](crate::editor_commands::INSERT) types
    /// [`CommandArgs::text`] and errors with
    /// [`CommandError::MissingText`] without it.
    pub fn with_editor_commands() -> Self {
        todo!()
    }

    /// Adds `command`. Errors, and adds nothing, if its id isn't in the
    /// form [`CommandId`] describes or is already registered.
    pub fn register(&mut self, command: Command) -> Result<(), RegisterError> {
        todo!()
    }

    /// The command with id `id`, if registered.
    pub fn get(&self, id: &str) -> Option<&Command> {
        todo!()
    }

    /// Every registered command, sorted by id.
    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.values()
    }

    /// Runs `invocation`'s command on `editor` and `set`.
    ///
    /// Errors with [`CommandError::UnknownCommand`] if the id isn't
    /// registered, and passes on the command's own error. Every editor
    /// command changes nothing when it errors, as [`EditorError`] says.
    pub fn run(
        &self,
        invocation: &Invocation,
        editor: &mut Editor,
        set: &mut BufferSet,
    ) -> Result<(), CommandError> {
        todo!()
    }
}

/// Whether `id` is in the form [`CommandId`] describes.
fn is_valid_id(id: &str) -> bool {
    todo!()
}

/// Why [`CommandRegistry::register`] refused a command.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RegisterError {
    /// The id isn't in the form [`CommandId`] describes.
    InvalidId {
        /// The id.
        id: CommandId,
    },
    /// A command with this id is already registered.
    Duplicate {
        /// The id.
        id: CommandId,
    },
}

impl fmt::Display for RegisterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for RegisterError {}

/// Why a command didn't run. On an error the buffer and the editor are
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommandError {
    /// No command with this id is registered.
    UnknownCommand {
        /// The id.
        id: CommandId,
    },
    /// The command needs [`CommandArgs::text`] and got none.
    MissingText {
        /// The command's id.
        id: CommandId,
    },
    /// The editor command failed.
    Editor(EditorError),
}

impl From<EditorError> for CommandError {
    fn from(error: EditorError) -> Self {
        todo!()
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for CommandError {}
