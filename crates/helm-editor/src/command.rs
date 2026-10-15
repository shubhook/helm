//! The command registry: every command has a stable string id, and runs
//! against one [`Editor`] and the [`BufferSet`] that owns its buffer.

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;
use std::sync::Arc;

use helm_text::BufferSet;

use crate::{Editor, EditorError, editor_commands};

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
        &self.0
    }
}

impl From<&str> for CommandId {
    fn from(id: &str) -> Self {
        Self(Arc::from(id))
    }
}

impl From<String> for CommandId {
    fn from(id: String) -> Self {
        Self(Arc::from(id))
    }
}

impl fmt::Display for CommandId {
    /// Writes the id as it is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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
        Self {
            command: command.into(),
            args: CommandArgs::default(),
        }
    }

    /// The insert command,
    /// [`editor_commands::INSERT`](crate::editor_commands::INSERT), typing
    /// `text`.
    pub fn insert(text: impl Into<String>) -> Self {
        Self {
            command: CommandId::from(editor_commands::INSERT),
            args: CommandArgs {
                text: Some(text.into()),
                motion: None,
            },
        }
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
    /// Keyed by the id's text, so [`get`](Self::get) looks up a `&str`.
    /// `Arc<str>` sorts as `CommandId` does.
    commands: BTreeMap<Arc<str>, Command>,
}

impl CommandRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry holding one command for each [`Editor`] command, under
    /// the ids in [`editor_commands`](crate::editor_commands). Each runs
    /// the `Editor` method of the same name on the context's editor and
    /// set. [`INSERT`](crate::editor_commands::INSERT) types
    /// [`CommandArgs::text`] and errors with
    /// [`CommandError::MissingText`] without it.
    pub fn with_editor_commands() -> Self {
        let mut registry = Self::new();
        editor_commands::register_all(&mut registry);
        registry
    }

    /// Adds `command`. Errors, and adds nothing, if its id isn't in the
    /// form [`CommandId`] describes or is already registered.
    pub fn register(&mut self, command: Command) -> Result<(), RegisterError> {
        if !is_valid_id(command.id.as_str()) {
            return Err(RegisterError::InvalidId { id: command.id });
        }
        match self.commands.entry(Arc::clone(&command.id.0)) {
            Entry::Occupied(_) => Err(RegisterError::Duplicate { id: command.id }),
            Entry::Vacant(entry) => {
                entry.insert(command);
                Ok(())
            }
        }
    }

    /// The command with id `id`, if registered.
    pub fn get(&self, id: &str) -> Option<&Command> {
        self.commands.get(id)
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
        let command =
            self.get(invocation.command.as_str())
                .ok_or_else(|| CommandError::UnknownCommand {
                    id: invocation.command.clone(),
                })?;
        (command.run)(CommandContext {
            editor,
            set,
            args: &invocation.args,
        })
    }
}

/// Whether `id` is in the form [`CommandId`] describes.
fn is_valid_id(id: &str) -> bool {
    id.split('.').all(|segment| {
        !segment.is_empty()
            && segment
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
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
        match self {
            Self::InvalidId { id } => write!(
                f,
                "command id {:?} is not dot-separated segments of a-z, 0-9 and _",
                id.as_str()
            ),
            Self::Duplicate { id } => {
                write!(f, "command {:?} is already registered", id.as_str())
            }
        }
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
        Self::Editor(error)
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownCommand { id } => {
                write!(f, "no command {:?} is registered", id.as_str())
            }
            Self::MissingText { id } => {
                write!(f, "command {:?} needs text and got none", id.as_str())
            }
            Self::Editor(error) => write!(f, "editor command failed: {error}"),
        }
    }
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Editor(error) => Some(error),
            _ => None,
        }
    }
}
