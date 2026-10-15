//! Key events with no window behind them: a key, its modifiers and the text
//! it types. `helm-ui` turns each platform key event into a [`KeyEvent`]
//! (Phase 2), so nothing below it sees a GPUI type.

use std::fmt;

/// A key, apart from the modifiers held with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// A key that types a character. The character is what the key types
    /// with Shift and the keyboard layout applied, but not Alt, Ctrl or
    /// Cmd: Shift+A is `Char('A')`, Shift+4 on a US layout is `Char('$')`,
    /// and Option+A on macOS is `Char('a')` with [`Modifiers::alt`] set.
    /// The space bar is `Char(' ')`. Never a control character; `helm-ui`
    /// sends the named keys below for those.
    Char(char),
    /// Return.
    Enter,
    /// Tab.
    Tab,
    /// Backspace, labelled "delete" on a Mac keyboard.
    Backspace,
    /// Forward delete, Fn+Delete on a Mac laptop.
    Delete,
    /// Escape.
    Escape,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home, Fn+Left on a Mac laptop.
    Home,
    /// End, Fn+Right on a Mac laptop.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// A function key, `F(1)` to `F(20)`.
    F(u8),
}

/// The modifier keys held during a keystroke.
///
/// `cmd` is the Command key (⌘) on macOS. Fn is not a modifier here: macOS
/// folds it into the key (Fn+Left arrives as [`Key::Home`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    /// Control (⌃).
    pub ctrl: bool,
    /// Option (⌥) on macOS.
    pub alt: bool,
    /// Shift (⇧).
    pub shift: bool,
    /// Command (⌘) on macOS.
    pub cmd: bool,
}

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        cmd: false,
    };
    /// Control alone.
    pub const CTRL: Self = Self {
        ctrl: true,
        ..Self::NONE
    };
    /// Option alone.
    pub const ALT: Self = Self {
        alt: true,
        ..Self::NONE
    };
    /// Shift alone.
    pub const SHIFT: Self = Self {
        shift: true,
        ..Self::NONE
    };
    /// Command alone.
    pub const CMD: Self = Self {
        cmd: true,
        ..Self::NONE
    };
}

/// One key with its modifiers, in normal form, so two keystrokes that mean
/// the same key compare equal.
///
/// # Normal form
///
/// Shift is folded into [`Key::Char`]: a char keystroke never has
/// [`Modifiers::shift`] set. [`new`](Self::new) turns an ASCII lowercase
/// letter with Shift into the uppercase letter, and for any other char
/// drops Shift and keeps the char, which `helm-ui` has already shifted. So
/// Shift+A, `Char('A')` and the notation `"shift-a"` and `"A"` are all one
/// keystroke. Named keys keep Shift: Shift+Tab and Shift+Left are not Tab
/// and Left.
///
/// # Notation
///
/// Keymaps and tests write keystrokes as text. [`parse`](Self::parse) reads
/// it and [`Display`](fmt::Display) writes it:
///
/// - Zero or more modifiers, each followed by `-`: `ctrl-`, `alt-`,
///   `shift-` and `cmd-`, in any order, each at most once.
/// - Then the key: one char, which is a [`Key::Char`], or one of the names
///   `enter`, `tab`, `backspace`, `delete`, `escape`, `space`, `left`,
///   `right`, `up`, `down`, `home`, `end`, `pageup`, `pagedown` and `f1` to
///   `f20`. `space` is `Char(' ')`.
///
/// Names and modifiers are lowercase. A single char is taken as it is, so
/// `"A"` is Shift+A, `"$"` is the dollar key and `"-"` is the minus key
/// (`"cmd--"` is Cmd and minus). `shift-` before a char other than an
/// ASCII letter is an error, since which char Shift makes depends on the
/// layout: write `"$"`, not `"shift-4"`.
///
/// [`Display`](fmt::Display) writes the modifiers in the order `ctrl`,
/// `alt`, `shift`, `cmd`, then the key: the char itself, `space` for
/// `Char(' ')`, or the key's name. Parsing what it writes gives back the
/// same keystroke.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Keystroke {
    key: Key,
    modifiers: Modifiers,
}

impl Keystroke {
    /// `key` with `modifiers`, in normal form: Shift is folded into a
    /// [`Key::Char`] as the type docs describe.
    pub fn new(key: Key, mut modifiers: Modifiers) -> Self {
        let key = match key {
            Key::Char(c) if modifiers.shift => {
                modifiers.shift = false;
                Key::Char(c.to_ascii_uppercase())
            }
            key => key,
        };
        Self { key, modifiers }
    }

    /// The key.
    pub fn key(&self) -> Key {
        self.key
    }

    /// The modifiers. `shift` is never set when the key is a
    /// [`Key::Char`].
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }

    /// Reads one keystroke in the notation the type docs describe, such as
    /// `"cmd-s"`, `"ctrl-shift-tab"` or `"A"`.
    pub fn parse(text: &str) -> Result<Self, KeyParseError> {
        if text.trim().is_empty() {
            return Err(KeyParseError::Empty);
        }
        let keystroke = || text.to_owned();
        let mut modifiers = Modifiers::NONE;
        let mut rest = text;
        loop {
            let (held, after) = if let Some(after) = rest.strip_prefix("ctrl-") {
                (&mut modifiers.ctrl, after)
            } else if let Some(after) = rest.strip_prefix("alt-") {
                (&mut modifiers.alt, after)
            } else if let Some(after) = rest.strip_prefix("shift-") {
                (&mut modifiers.shift, after)
            } else if let Some(after) = rest.strip_prefix("cmd-") {
                (&mut modifiers.cmd, after)
            } else {
                break;
            };
            if *held {
                return Err(KeyParseError::RepeatedModifier {
                    keystroke: keystroke(),
                });
            }
            if after.is_empty() {
                return Err(KeyParseError::MissingKey {
                    keystroke: keystroke(),
                });
            }
            *held = true;
            rest = after;
        }
        let key = parse_key(rest).ok_or_else(|| KeyParseError::UnknownKey {
            keystroke: keystroke(),
        })?;
        if let Key::Char(c) = key
            && modifiers.shift
            && !c.is_ascii_alphabetic()
        {
            return Err(KeyParseError::ShiftedChar {
                keystroke: keystroke(),
            });
        }
        Ok(Self::new(key, modifiers))
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.modifiers;
        for (held, prefix) in [
            (m.ctrl, "ctrl-"),
            (m.alt, "alt-"),
            (m.shift, "shift-"),
            (m.cmd, "cmd-"),
        ] {
            if held {
                f.write_str(prefix)?;
            }
        }
        let name = match self.key {
            Key::Char(' ') => "space",
            Key::Char(c) => return write!(f, "{c}"),
            Key::F(n) => return write!(f, "f{n}"),
            Key::Enter => "enter",
            Key::Tab => "tab",
            Key::Backspace => "backspace",
            Key::Delete => "delete",
            Key::Escape => "escape",
            Key::Left => "left",
            Key::Right => "right",
            Key::Up => "up",
            Key::Down => "down",
            Key::Home => "home",
            Key::End => "end",
            Key::PageUp => "pageup",
            Key::PageDown => "pagedown",
        };
        f.write_str(name)
    }
}

/// The key that `text`, the part of a keystroke after its modifiers,
/// stands for in the notation: one char other than a control char, or a
/// key name. `None` for anything else.
fn parse_key(text: &str) -> Option<Key> {
    let key = match text {
        "enter" => Key::Enter,
        "tab" => Key::Tab,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "escape" => Key::Escape,
        "space" => Key::Char(' '),
        "left" => Key::Left,
        "right" => Key::Right,
        "up" => Key::Up,
        "down" => Key::Down,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        _ => {
            let mut chars = text.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                return (!c.is_control()).then_some(Key::Char(c));
            }
            // `f1` to `f20`, digits only and no leading zero, so the name
            // reads back as `Display` writes it.
            let digits = text.strip_prefix('f')?;
            if digits.starts_with('0') || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let n: u8 = digits.parse().ok()?;
            return (1..=20).contains(&n).then_some(Key::F(n));
        }
    };
    Some(key)
}

/// One or more keystrokes pressed one after another. A sequence of two or
/// more is a chord, such as VS Code's `cmd-k cmd-s`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeySequence {
    /// Never empty.
    keys: Vec<Keystroke>,
}

impl KeySequence {
    /// The sequence `keys`, or `None` if `keys` is empty.
    pub fn new(keys: Vec<Keystroke>) -> Option<Self> {
        (!keys.is_empty()).then_some(Self { keys })
    }

    /// Reads keystrokes separated by whitespace, each in
    /// [`Keystroke`]'s notation: `"cmd-k cmd-s"`, `"g g"`. Errors with
    /// [`KeyParseError::Empty`] if there is none.
    pub fn parse(text: &str) -> Result<Self, KeyParseError> {
        let keys = text
            .split_whitespace()
            .map(Keystroke::parse)
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(keys).ok_or(KeyParseError::Empty)
    }

    /// The keystrokes, in the order they are pressed.
    pub fn as_slice(&self) -> &[Keystroke] {
        &self.keys
    }
}

impl From<Keystroke> for KeySequence {
    /// A sequence of one keystroke.
    fn from(keystroke: Keystroke) -> Self {
        Self {
            keys: vec![keystroke],
        }
    }
}

impl fmt::Display for KeySequence {
    /// The keystrokes in [`Keystroke`]'s notation, joined by one space.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, keystroke) in self.keys.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{keystroke}")?;
        }
        Ok(())
    }
}

/// A key press as `helm-ui` hands it to the editor: the keystroke, and the
/// text the key types, if any.
///
/// The keystroke picks the binding. The text is what goes into the buffer
/// when no binding matches and the mode inserts typed text (see
/// [`Dispatcher::resolve`](crate::Dispatcher::resolve)). They differ for
/// Option on macOS: Option+A is the keystroke `alt-a` and types `"å"`.
///
/// Text composed by an input method (ED-8) doesn't come through here;
/// `helm-ui` runs the insert command with it directly.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// The key and its modifiers.
    pub keystroke: Keystroke,
    /// The text the key types, or `None` for a key that types nothing.
    pub text: Option<String>,
}

impl KeyEvent {
    /// A key event with the text a plain key types: for a [`Key::Char`]
    /// with none of Alt, Ctrl or Cmd held, the char itself; for anything
    /// else, `None`. So `a` types `"a"`, `A` types `"A"`, `space` types
    /// `" "`, and `ctrl-a`, `alt-a` and `enter` type nothing.
    pub fn new(keystroke: Keystroke) -> Self {
        let m = keystroke.modifiers;
        let text = match keystroke.key {
            Key::Char(c) if !(m.alt || m.ctrl || m.cmd) => Some(c.to_string()),
            _ => None,
        };
        Self { keystroke, text }
    }

    /// A key event that types `text`, whatever the keystroke. `helm-ui`
    /// uses this when the platform reports text the keystroke alone
    /// doesn't give, such as `"å"` for `alt-a`.
    pub fn with_text(keystroke: Keystroke, text: impl Into<String>) -> Self {
        Self {
            keystroke,
            text: Some(text.into()),
        }
    }

    /// [`Keystroke::parse`], then [`new`](Self::new).
    pub fn parse(text: &str) -> Result<Self, KeyParseError> {
        Keystroke::parse(text).map(Self::new)
    }
}

/// Why [`Keystroke::parse`] or [`KeySequence::parse`] rejected some text.
/// Each case carries the keystroke's text as written.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum KeyParseError {
    /// The text holds no keystroke: empty, or only whitespace.
    Empty,
    /// Modifiers with no key after them, like `"cmd-"`.
    MissingKey {
        /// The keystroke as written.
        keystroke: String,
    },
    /// A modifier written twice, like `"cmd-cmd-k"`.
    RepeatedModifier {
        /// The keystroke as written.
        keystroke: String,
    },
    /// The key is neither one char nor a known name, like `"cmd-foo"` or
    /// `"Enter"`, or is a control char.
    UnknownKey {
        /// The keystroke as written.
        keystroke: String,
    },
    /// `shift-` before a char that isn't an ASCII letter, like
    /// `"shift-4"`. Write the shifted char instead.
    ShiftedChar {
        /// The keystroke as written.
        keystroke: String,
    },
}

impl fmt::Display for KeyParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("no keystroke given"),
            Self::MissingKey { keystroke } => {
                write!(f, "keystroke {keystroke:?} has modifiers but no key")
            }
            Self::RepeatedModifier { keystroke } => {
                write!(f, "keystroke {keystroke:?} repeats a modifier")
            }
            Self::UnknownKey { keystroke } => {
                write!(f, "keystroke {keystroke:?} names no known key")
            }
            Self::ShiftedChar { keystroke } => write!(
                f,
                "keystroke {keystroke:?} puts shift before a char that isn't an ASCII letter; write the shifted char instead"
            ),
        }
    }
}

impl std::error::Error for KeyParseError {}
