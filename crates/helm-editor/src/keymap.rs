//! Keymaps as data: the modes a keymap has, and bindings from key sequences
//! to commands, each scoped by mode and by an optional context.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::{CommandId, KeyParseError, KeySequence, Keystroke};

/// A mode's id, such as `"normal"` or `"insert"`. Unique within a keymap.
///
/// Clones are cheap: the string is shared.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ModeId(Arc<str>);

impl ModeId {
    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ModeId {
    fn from(id: &str) -> Self {
        Self(Arc::from(id))
    }
}

impl From<String> for ModeId {
    fn from(id: String) -> Self {
        Self(Arc::from(id))
    }
}

impl fmt::Display for ModeId {
    /// Writes the id as it is.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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
        Self {
            id: id.into(),
            typing,
        }
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
        Self::with_action(keys, Some(Action::Run(command.into())))
    }

    /// Keys `keys` that hold `command` as the pending operator, in every
    /// mode and context.
    pub fn operator(keys: &str, command: impl Into<CommandId>) -> Result<Self, KeyParseError> {
        Self::with_action(keys, Some(Action::Operator(command.into())))
    }

    /// Keys `keys` that only switch to mode `mode`, in every mode and
    /// context.
    pub fn switch_mode(keys: &str, mode: impl Into<ModeId>) -> Result<Self, KeyParseError> {
        Ok(Self::with_action(keys, None)?.then_mode(mode))
    }

    /// This binding, applying only in mode `mode`.
    pub fn in_mode(self, mode: impl Into<ModeId>) -> Self {
        Self {
            mode: Some(mode.into()),
            ..self
        }
    }

    /// This binding, applying only while context `context` holds.
    pub fn when(self, context: impl Into<String>) -> Self {
        Self {
            context: Some(context.into()),
            ..self
        }
    }

    /// This binding, also switching to mode `mode` when it matches.
    pub fn then_mode(self, mode: impl Into<ModeId>) -> Self {
        Self {
            then_mode: Some(mode.into()),
            ..self
        }
    }

    /// Keys `keys` doing `action`, in every mode and context, with no mode
    /// switch.
    fn with_action(keys: &str, action: Option<Action>) -> Result<Self, KeyParseError> {
        Ok(Self {
            keys: KeySequence::parse(keys)?,
            mode: None,
            context: None,
            action,
            then_mode: None,
        })
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
    /// The index into `bindings` of every binding whose keys start with
    /// the keystroke, in the order they were added. Resolving a key only
    /// looks at one of these lists, not every binding.
    by_first_key: HashMap<Keystroke, Vec<usize>>,
}

impl Keymap {
    /// A keymap named `name` with `modes` and no bindings. The first mode
    /// is the initial mode.
    ///
    /// Errors with [`KeymapError::NoModes`] if `modes` is empty, and with
    /// [`KeymapError::DuplicateMode`] if two modes share an id.
    pub fn new(name: impl Into<String>, modes: Vec<Mode>) -> Result<Self, KeymapError> {
        if modes.is_empty() {
            return Err(KeymapError::NoModes);
        }
        for (i, mode) in modes.iter().enumerate() {
            if modes[..i].iter().any(|earlier| earlier.id == mode.id) {
                return Err(KeymapError::DuplicateMode {
                    mode: mode.id.clone(),
                });
            }
        }
        Ok(Self {
            name: name.into(),
            modes,
            bindings: Vec::new(),
            by_first_key: HashMap::new(),
        })
    }

    /// The keymap's name, such as `"vscode"` or `"vim"`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The modes, the initial mode first.
    pub fn modes(&self) -> &[Mode] {
        &self.modes
    }

    /// The mode with id `id`, if the keymap has it.
    pub fn mode(&self, id: &str) -> Option<&Mode> {
        self.modes.iter().find(|mode| mode.id.as_str() == id)
    }

    /// The id of the first mode.
    pub fn initial_mode(&self) -> &ModeId {
        &self.modes[0].id
    }

    /// Adds `binding` after every binding added before it.
    ///
    /// Errors with [`KeymapError::UnknownMode`], and adds nothing, if
    /// `binding.mode` or `binding.then_mode` names a mode the keymap
    /// doesn't have.
    pub fn bind(&mut self, binding: Binding) -> Result<(), KeymapError> {
        for mode in [&binding.mode, &binding.then_mode].into_iter().flatten() {
            if self.mode(mode.as_str()).is_none() {
                return Err(KeymapError::UnknownMode { mode: mode.clone() });
            }
        }
        let first = binding.keys.as_slice()[0];
        self.by_first_key
            .entry(first)
            .or_default()
            .push(self.bindings.len());
        self.bindings.push(binding);
        Ok(())
    }

    /// The bindings, in the order they were added.
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Every binding whose keys start with `first`, in the order they were
    /// added.
    pub(crate) fn bindings_starting_with(
        &self,
        first: Keystroke,
    ) -> impl Iterator<Item = &Binding> {
        self.by_first_key
            .get(&first)
            .into_iter()
            .flatten()
            .map(|&i| &self.bindings[i])
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
        match self {
            Self::NoModes => f.write_str("a keymap needs at least one mode"),
            Self::DuplicateMode { mode } => write!(f, "two modes have the id {:?}", mode.as_str()),
            Self::UnknownMode { mode } => {
                write!(f, "the keymap has no mode {:?}", mode.as_str())
            }
        }
    }
}

impl std::error::Error for KeymapError {}
