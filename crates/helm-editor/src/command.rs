//! The command registry: every command has a stable string id, and runs
//! against one [`Editor`] and the [`BufferSet`] that owns its buffer.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use helm_text::BufferSet;

use crate::vim::ExError;
use crate::{
    Editor, EditorError, Request, Session, cursor_commands, editor_commands, vim_commands,
};

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

/// What a command gets besides the editor: the text to type, the motion a
/// pending operator runs over, the count typed before it, and the register
/// named before it. A command ignores what it doesn't use.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommandArgs {
    /// The text [`editor_commands::INSERT`](crate::editor_commands::INSERT)
    /// types. A key no binding matches arrives here (see
    /// [`Dispatcher::resolve`](crate::Dispatcher::resolve)).
    pub text: Option<String>,
    /// For an operator, the motion it applies to: the command a key bound
    /// after the operator named (see [`Action::Operator`](crate::Action)).
    /// What an operator does with it is up to the operator. Vim's
    /// operators (T-108) take the motion ids in
    /// [`vim_commands`](crate::vim_commands).
    pub motion: Option<CommandId>,
    /// The count typed before the command (T-108), or `None` if none was.
    /// [`Dispatcher::resolve`](crate::Dispatcher::resolve) says where it
    /// comes from: in Vim's keymap `3 w` gives 3, and `2 d 3 w` gives the
    /// operator 6. A command that takes no count ignores it. One that does
    /// reads `None` as its default, usually 1.
    pub count: Option<usize>,
    /// The register named before the command (T-109), or `None` if none
    /// was: `'a'` after Vim's `" a`.
    /// [`Dispatcher::resolve`](crate::Dispatcher::resolve) says where it
    /// comes from. Vim's `.` replays it with the rest of the arguments.
    pub register: Option<char>,
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
                ..CommandArgs::default()
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
    /// The invocation's arguments, its count included.
    pub args: &'a CommandArgs,
    /// The registry the command runs from (T-109), so a command such as
    /// Vim's `.` can run other commands through
    /// [`CommandRegistry::run_in`].
    pub registry: &'a CommandRegistry,
    /// The state every editor shares (T-109): Vim's registers and its last
    /// change.
    pub session: &'a mut Session,
    /// Where the command puts what only `helm-ui` can do (T-109), such as
    /// saving a file. A command that errors pushes nothing.
    pub requests: &'a mut Vec<Request>,
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
    /// The ids marked as changes, the commands Vim's `.` repeats (T-109).
    changes: BTreeSet<Arc<str>>,
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

    /// A registry holding the commands of
    /// [`with_editor_commands`](Self::with_editor_commands) and the cursor
    /// commands under the ids in
    /// [`cursor_commands`](crate::cursor_commands): every command the
    /// [`vscode`](crate::vscode) keymap binds. `helm-ui` builds its
    /// dispatcher with [`with_vim_commands`](Self::with_vim_commands),
    /// which holds these and Vim's.
    pub fn with_all_commands() -> Self {
        let mut registry = Self::with_editor_commands();
        cursor_commands::register_all(&mut registry);
        registry
    }

    /// A registry holding the commands of
    /// [`with_all_commands`](Self::with_all_commands) and the Vim commands
    /// under the ids in [`vim_commands`](crate::vim_commands) (T-108):
    /// every command the [`vscode`](crate::vscode) and [`vim`](crate::vim)
    /// keymaps bind, 143 in all. One dispatcher built with it can switch
    /// between the two keymaps at runtime (ED-6). This is the registry
    /// `helm-ui` builds its dispatcher with.
    pub fn with_vim_commands() -> Self {
        let mut registry = Self::with_all_commands();
        vim_commands::register_all(&mut registry);
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
    ///
    /// The command gets the invocation's arguments, so a count typed before
    /// it arrives as [`CommandArgs::count`].
    ///
    /// It runs with a fresh [`Session`], dropped afterwards with the
    /// [`Request`]s the command made (T-109). This is
    /// [`run_in`](Self::run_in) for callers with no session, such as VS
    /// Code's commands and the T-106 to T-108 tests.
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
        let mut session = Session::new();
        let mut requests = Vec::new();
        (command.run)(CommandContext {
            editor,
            set,
            args: &invocation.args,
            registry: self,
            session: &mut session,
            requests: &mut requests,
        })
    }

    /// Runs `invocation`'s command on `editor`, `set` and `session`, and
    /// returns the [`Request`]s it made, in order (T-109). Errors as
    /// [`run`](Self::run) does, and returns no request then.
    ///
    /// It also keeps the record Vim's `.` repeats, as the
    /// [`vim`](crate::vim) module docs' "`.` repeat" says. In short: a
    /// successful run of a command [marked as a change](Self::mark_change)
    /// starts a record. If the editor's undo group is open after it, every
    /// later run on that editor joins the record until the group closes;
    /// then, or at once if no group was open, the record becomes the
    /// session's [last change](Session::last_change). Runs inside the
    /// replay of `.` are recorded the same way.
    pub fn run_in(
        &self,
        invocation: &Invocation,
        editor: &mut Editor,
        set: &mut BufferSet,
        session: &mut Session,
    ) -> Result<Vec<Request>, CommandError> {
        let command = self.command(invocation)?;
        // A strong reference only while this runs, to look up records.
        let id = editor.id().clone();
        session.end_closed_record(&id, editor.in_undo_group());
        // Recorded before it runs, so a command can read the record it is
        // part of, as Escape does for a counted insert.
        let recorded = session.is_recording(&id) || self.is_change(invocation.command.as_str());
        if recorded {
            session.record(&id, invocation);
        }
        let mut requests = Vec::new();
        let result = (command.run)(CommandContext {
            editor,
            set,
            args: &invocation.args,
            registry: self,
            session,
            requests: &mut requests,
        });
        if let Err(error) = result {
            if recorded {
                session.unrecord(&id);
            }
            return Err(error);
        }
        if recorded && !editor.in_undo_group() {
            session.finish_record(&id);
        }
        Ok(requests)
    }

    /// Runs `invocation`'s command with `session` and `requests` and
    /// records nothing, as a counted insert's repeats run (T-109).
    pub(crate) fn run_unrecorded(
        &self,
        invocation: &Invocation,
        editor: &mut Editor,
        set: &mut BufferSet,
        session: &mut Session,
        requests: &mut Vec<Request>,
    ) -> Result<(), CommandError> {
        let command = self.command(invocation)?;
        (command.run)(CommandContext {
            editor,
            set,
            args: &invocation.args,
            registry: self,
            session,
            requests,
        })
    }

    /// The command `invocation` names, or the error for an unknown id.
    fn command(&self, invocation: &Invocation) -> Result<&Command, CommandError> {
        self.get(invocation.command.as_str())
            .ok_or_else(|| CommandError::UnknownCommand {
                id: invocation.command.clone(),
            })
    }

    /// Marks the command with id `id` as a change (T-109): one that Vim's
    /// `.` repeats. Marking an id twice, or one that isn't registered, is
    /// allowed; an unregistered id is never run. Marks are kept apart from
    /// [`Command`], whose fields T-106's tests build by name.
    pub fn mark_change(&mut self, id: &str) {
        self.changes.insert(Arc::from(id));
    }

    /// Whether the command with id `id` is marked as a change (T-109).
    pub fn is_change(&self, id: &str) -> bool {
        self.changes.contains(id)
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
    /// A Vim command line that isn't one Helm runs (T-109), such as `:foo`
    /// or `:q file`. Nothing changed and no request was made.
    Ex {
        /// The line as typed, without the `:`.
        line: String,
        /// What is wrong with it.
        error: ExError,
    },
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
            Self::Ex { line, error } => write!(f, ":{line}: {error}"),
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
