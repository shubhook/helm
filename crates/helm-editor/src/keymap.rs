//! Keymaps as data: the modes a keymap has, and bindings from key sequences
//! to commands, each scoped by mode and by an optional context.

#![expect(
    unused_variables,
    dead_code,
    reason = "T-106 interface stubs; the implementation deletes this"
)]

use std::fmt;
use std::sync::Arc;

use crate::{CommandId, KeyParseError, KeySequence};

/// A mode's id, such as `"normal"` or `"insert"`. Unique within a keymap.
///
/// Clones are cheap: the string is shared.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModeId(Arc<str>);

impl ModeId {
    /// The id as text.
    pub fn as_str(&self) -> &str {
        todo!()
    }
}

impl From<&str> for ModeId {
    fn from(id: &str) -> Self {
        todo!()
    }
}

impl From<String> for ModeId {
    fn from(id: String) -> Self {
        todo!()
    }
}

impl fmt::Display for ModeId {
    /// Writes the id as it is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

/// What a key no binding matches does in a mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Typing {
    /// It types its [`KeyEvent::text`](crate::KeyEvent::text), if it has
    /// any and Ctrl and Cmd aren't held. VS Code's only mode, and Vim's
    /// insert mode.
    Insert,
    /// It does nothing. Vim's normal and visual modes.
    Ignore,
}

/// One mode of a keymap. A mode is data: its id and what typing does in
/// it. Which keys do what in it are the keymap's bindings scoped to it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Mode {
    /// The mode's id.
    pub id: ModeId,
    /// What a key no binding matches does in this mode.
    pub typing: Typing,
}

impl Mode {
    /// A mode with id `id`.
    pub fn new(id: impl Into<ModeId>, typing: Typing) -> Self {
        todo!()
    }
}

/// What a binding does with its command.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Run the command. If an operator is pending, run the operator instead,
    /// with this command as its [`motion`](crate::CommandArgs::motion).
    Run(CommandId),
    /// Hold the command as the pending operator, such as Vim's `d`. Nothing
    /// runs until a key bound to [`Action::Run`] gives it a motion.
    Operator(CommandId),
}

/// One binding: a key sequence, where it applies, and what it does.
///
/// A binding with no [`action`](Self::action) and no
/// [`then_mode`](Self::then_mode) still matches its keys and does nothing,
/// which hides a binding it beats. A keybindings file uses that to unbind
/// a default key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Binding {
    /// The keys, one keystroke or a chord.
    pub keys: KeySequence,
    /// The mode it applies in, or `None` for every mode of the keymap.
    pub mode: Option<ModeId>,
    /// A context that must hold for it to apply, or `None` for always.
    /// Contexts are plain names, such as `"editor_has_selection"`, and the
    /// caller of [`Dispatcher::resolve`](crate::Dispatcher::resolve) says
    /// which hold.
    pub context: Option<String>,
    /// What it does with a command, if anything.
    pub action: Option<Action>,
    /// The mode to switch to when it matches, if any.
    pub then_mode: Option<ModeId>,
}

impl Binding {
    /// Keys `keys`, in [`KeySequence`]'s notation, that run `command`, in
    /// every mode and context.
    pub fn run(keys: &str, command: impl Into<CommandId>) -> Result<Self, KeyParseError> {
        todo!()
    }

    /// Keys `keys` that hold `command` as the pending operator, in every
    /// mode and context.
    pub fn operator(keys: &str, command: impl Into<CommandId>) -> Result<Self, KeyParseError> {
        todo!()
    }

    /// Keys `keys` that only switch to mode `mode`, in every mode and
    /// context.
    pub fn switch_mode(keys: &str, mode: impl Into<ModeId>) -> Result<Self, KeyParseError> {
        todo!()
    }

    /// This binding, applying only in mode `mode`.
    pub fn in_mode(self, mode: impl Into<ModeId>) -> Self {
        todo!()
    }

    /// This binding, applying only while context `context` holds.
    pub fn when(self, context: impl Into<String>) -> Self {
        todo!()
    }

    /// This binding, also switching to mode `mode` when it matches.
    pub fn then_mode(self, mode: impl Into<ModeId>) -> Self {
        todo!()
    }
}

/// A named set of modes and bindings: one editing model, such as VS Code's
/// (T-107) or Vim's (T-108).
///
/// The first mode is the initial mode, the one a new
/// [`KeyState`](crate::KeyState) starts in. Bindings keep the order they
/// were added in, which breaks ties in
/// [`Dispatcher::resolve`](crate::Dispatcher::resolve): of two bindings
/// that both apply, the later one wins, so a keybindings file loaded after
/// the defaults overrides them.
///
/// A binding may name a command no registry holds. The keymap doesn't know
/// which commands exist: `helm-ui` runs some itself, and running an unknown
/// one errors with [`CommandError::UnknownCommand`](crate::CommandError).
#[derive(Clone, Debug)]
pub struct Keymap {
    name: String,
    /// Never empty. The first is the initial mode.
    modes: Vec<Mode>,
    /// In the order they were added.
    bindings: Vec<Binding>,
}

impl Keymap {
    /// A keymap named `name` with `modes` and no bindings. The first mode
    /// is the initial mode.
    ///
    /// Errors with [`KeymapError::NoModes`] if `modes` is empty, and with
    /// [`KeymapError::DuplicateMode`] if two modes share an id.
    pub fn new(name: impl Into<String>, modes: Vec<Mode>) -> Result<Self, KeymapError> {
        todo!()
    }

    /// The keymap's name, such as `"vscode"` or `"vim"`.
    pub fn name(&self) -> &str {
        todo!()
    }

    /// The modes, the initial mode first.
    pub fn modes(&self) -> &[Mode] {
        todo!()
    }

    /// The mode with id `id`, if the keymap has it.
    pub fn mode(&self, id: &str) -> Option<&Mode> {
        todo!()
    }

    /// The id of the first mode.
    pub fn initial_mode(&self) -> &ModeId {
        todo!()
    }

    /// Adds `binding` after every binding added before it.
    ///
    /// Errors with [`KeymapError::UnknownMode`], and adds nothing, if
    /// `binding.mode` or `binding.then_mode` names a mode the keymap
    /// doesn't have.
    pub fn bind(&mut self, binding: Binding) -> Result<(), KeymapError> {
        todo!()
    }

    /// The bindings, in the order they were added.
    pub fn bindings(&self) -> &[Binding] {
        todo!()
    }
}

/// Why [`Keymap::new`] or [`Keymap::bind`] refused.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeymapError {
    /// A keymap needs at least one mode.
    NoModes,
    /// Two modes share this id.
    DuplicateMode {
        /// The id.
        mode: ModeId,
    },
    /// A binding names a mode the keymap doesn't have.
    UnknownMode {
        /// The mode named.
        mode: ModeId,
    },
}

impl fmt::Display for KeymapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for KeymapError {}
