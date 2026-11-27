//! State every editor shares (T-109): Vim's registers and its last change,
//! and the requests a command makes for `helm-ui` to carry out.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::Invocation;

/// What a register holds: one piece of text per cursor that wrote it, in
/// selection order, and whether the text is whole lines.
///
/// A linewise piece ends with its line break, as the lines did in the
/// buffer. A charwise piece is the bytes as they were.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Register {
    /// The text, one piece per cursor. Never empty in a register the
    /// session holds.
    pub pieces: Vec<String>,
    /// Whether the text is whole lines (Vim's linewise register).
    pub linewise: bool,
}

impl Register {
    /// The register's text as one string: the pieces joined by `newline`
    /// when charwise, and run together when linewise, since each linewise
    /// piece already ends with a line break. This is what a paste at a
    /// different number of cursors puts at each cursor, and what goes on
    /// the clipboard.
    #[expect(
        unused_variables,
        reason = "T-109 interface stub; the implementation deletes this"
    )]
    pub fn text(&self, newline: &str) -> String {
        todo!()
    }
}

/// Something only `helm-ui` can do, asked for by a command (T-109).
///
/// A command pushes requests onto
/// [`CommandContext::requests`](crate::CommandContext::requests) and
/// returns. [`Dispatcher::dispatch_in`](crate::Dispatcher::dispatch_in) and
/// [`CommandRegistry::run_in`](crate::CommandRegistry::run_in) hand them
/// back in order, and `helm-ui` carries them out after the command has
/// returned, stopping at the first one that fails. `helm-editor` never
/// learns whether one worked: it knows nothing about files, windows or the
/// clipboard.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Request {
    /// Save this editor's buffer, to `path` if given (Vim's `:w`, `:w path`
    /// and `:w!`). `force` is Vim's `!`. A relative path is relative to
    /// whatever `helm-ui` decides.
    Save {
        /// Where to save, or `None` for the buffer's own file.
        path: Option<PathBuf>,
        /// Vim's `!`.
        force: bool,
    },
    /// Close this editor (Vim's `:q` and `:q!`). Without `force`, `helm-ui`
    /// refuses when the buffer has unsaved edits that no other editor
    /// shows, as Vim's E37 does. Whether closing the last editor also
    /// closes the window is `helm-ui`'s call.
    Close {
        /// Vim's `!`.
        force: bool,
    },
    /// Open `path` in this editor, or reload this editor's own file when
    /// `path` is `None` (Vim's `:e path`, `:e` and `:e!`). Without `force`,
    /// `helm-ui` refuses when that would drop unsaved edits.
    Open {
        /// The file to open, or `None` to reload.
        path: Option<PathBuf>,
        /// Vim's `!`.
        force: bool,
    },
    /// Put `text` on the system clipboard (the `+` and `*` registers).
    SetClipboard {
        /// The text, exactly as the register holds it.
        text: String,
    },
}

/// The state every editor shares (T-109): Vim's registers and the last
/// change `.` repeats.
///
/// `helm-ui` holds one, next to its one [`Dispatcher`](crate::Dispatcher),
/// and passes it to every
/// [`dispatch_in`](crate::Dispatcher::dispatch_in), so text yanked in one
/// editor puts in another and `.` repeats the last change wherever the
/// cursor is now. It holds no platform handle, and commands change it only
/// through [`CommandContext::session`](crate::CommandContext::session).
///
/// # Registers
///
/// The names are `"` (unnamed), `0` to `9`, `a` to `z`, `-` (small
/// delete), `_` (black hole), and `+` and `*`, which are both the system
/// clipboard. `A` to `Z` name `a` to `z` for appending. The
/// [`vim`](crate::vim) module docs' "Registers" say what each command
/// writes.
///
/// # The clipboard
///
/// Before it dispatches a key, `helm-ui` checks whether the system
/// clipboard changed since it last told the session (on macOS,
/// `NSPasteboard`'s `changeCount`), and if it did it calls
/// [`set_clipboard`](Self::set_clipboard). A write to `+` or `*` pushes
/// [`Request::SetClipboard`].
#[derive(Clone, Debug, Default)]
pub struct Session {
    /// The registers by name. A placeholder: the implementation may
    /// reshape it.
    #[expect(
        dead_code,
        reason = "T-109 interface stub; the implementation reads it"
    )]
    registers: BTreeMap<char, Register>,
    /// The last change `.` repeats. A placeholder.
    #[expect(
        dead_code,
        reason = "T-109 interface stub; the implementation reads it"
    )]
    last_change: Option<Vec<Invocation>>,
}

impl Session {
    /// A session with every register empty and no last change.
    pub fn new() -> Self {
        Self::default()
    }

    /// What register `name` holds, or `None` if it is empty or `name`
    /// isn't a register. `'"'` gives what the unnamed register holds, `A`
    /// to `Z` read as `a` to `z`, `*` reads as `+`, and `_` is always empty.
    #[expect(
        unused_variables,
        reason = "T-109 interface stub; the implementation deletes this"
    )]
    pub fn register(&self, name: char) -> Option<&Register> {
        todo!()
    }

    /// Tells the session the system clipboard now holds `text`, or nothing
    /// (T-109). `helm-ui` calls this when the clipboard changed; see the
    /// type docs.
    ///
    /// If `text` is exactly the text the session last pushed in a
    /// [`Request::SetClipboard`], the `+` register keeps the pieces and the
    /// linewise flag it had. Otherwise `+` becomes one piece, linewise if
    /// `text` ends with a line break. `None` or an empty string empties
    /// `+`. The unnamed register doesn't change.
    #[expect(
        unused_variables,
        reason = "T-109 interface stub; the implementation deletes this"
    )]
    pub fn set_clipboard(&mut self, text: Option<String>) {
        todo!()
    }

    /// The last change, the invocations `.` runs again, in order, or
    /// `None` before the first change.
    pub fn last_change(&self) -> Option<&[Invocation]> {
        todo!()
    }
}
