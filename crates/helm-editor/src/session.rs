//! State every editor shares (T-109): Vim's registers and its last change,
//! and the requests a command makes for `helm-ui` to carry out.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use helm_text::BufferId;

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
    pub fn text(&self, newline: &str) -> String {
        if self.linewise {
            self.pieces.concat()
        } else {
            self.pieces.join(newline)
        }
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
    /// The registers by name: `"`, `0` to `9`, `a` to `z`, `-` and `+`.
    /// An empty register has no entry. Registers written together, such
    /// as `"` and `1`, share one copy of the text.
    registers: BTreeMap<char, Arc<Register>>,
    /// The last change `.` repeats.
    last_change: Option<Vec<Invocation>>,
    /// The change still being recorded, while its undo group is open.
    recording: Option<Recording>,
    /// The text of the last [`Request::SetClipboard`] this session pushed.
    clipboard: Option<String>,
}

/// A change being recorded: the buffer of the editor it runs on, and its
/// invocations so far.
#[derive(Clone, Debug)]
struct Recording {
    buffer: BufferId,
    invocations: Vec<Invocation>,
}

/// What wrote a register, which decides the registers it goes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Write {
    /// `y` and its forms.
    Yank,
    /// `d`, `c`, their forms and visual `p`.
    Delete,
}

impl Session {
    /// A session with every register empty and no last change.
    pub fn new() -> Self {
        Self::default()
    }

    /// What register `name` holds, or `None` if it is empty or `name`
    /// isn't a register. `'"'` gives what the unnamed register holds, `A`
    /// to `Z` read as `a` to `z`, `*` reads as `+`, and `_` is always empty.
    pub fn register(&self, name: char) -> Option<&Register> {
        self.registers
            .get(&stored_name(name)?)
            .map(|register| &**register)
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
    pub fn set_clipboard(&mut self, text: Option<String>) {
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            self.registers.remove(&'+');
            self.clipboard = None;
            return;
        };
        if self.clipboard.as_deref() == Some(text.as_str()) && self.registers.contains_key(&'+') {
            return;
        }
        self.clipboard = None;
        let linewise = text.ends_with(['\n', '\r']);
        self.registers.insert(
            '+',
            Arc::new(Register {
                pieces: vec![text],
                linewise,
            }),
        );
    }

    /// The last change, the invocations `.` runs again, in order, or
    /// `None` before the first change.
    pub fn last_change(&self) -> Option<&[Invocation]> {
        self.last_change.as_deref()
    }

    /// Writes `register` as `write` does to register `name`, or with no
    /// name given, as the `vim` module docs' "Registers" say. `newline` is
    /// the editor's line break. A write to `+` or `*` pushes
    /// [`Request::SetClipboard`] onto `requests`.
    pub(crate) fn write(
        &mut self,
        name: Option<char>,
        register: Register,
        write: Write,
        newline: &str,
        requests: &mut Vec<Request>,
    ) {
        if register.pieces.is_empty() {
            return;
        }
        let register = Arc::new(register);
        let register = match name.filter(|&name| name != '"') {
            None => {
                match write {
                    Write::Yank => {
                        self.registers.insert('0', Arc::clone(&register));
                    }
                    Write::Delete
                        if register.linewise
                            || register.pieces.iter().any(|p| p.contains(['\n', '\r'])) =>
                    {
                        for n in (b'1'..b'9').rev() {
                            if let Some(moved) = self.registers.remove(&char::from(n)) {
                                self.registers.insert(char::from(n + 1), moved);
                            }
                        }
                        self.registers.insert('1', Arc::clone(&register));
                    }
                    Write::Delete => {
                        self.registers.insert('-', Arc::clone(&register));
                    }
                }
                register
            }
            Some('_') => return,
            Some(name @ 'A'..='Z') => {
                let name = name.to_ascii_lowercase();
                let joined = match self.registers.remove(&name) {
                    Some(old) => Arc::new(append(
                        Arc::unwrap_or_clone(old),
                        Arc::unwrap_or_clone(register),
                        newline,
                    )),
                    None => register,
                };
                self.registers.insert(name, Arc::clone(&joined));
                joined
            }
            Some(name) => {
                let Some(name) = stored_name(name) else {
                    return;
                };
                if name == '+' {
                    let text = register.text(newline);
                    self.clipboard = Some(text.clone());
                    requests.push(Request::SetClipboard { text });
                }
                self.registers.insert(name, Arc::clone(&register));
                register
            }
        };
        self.registers.insert('"', register);
    }

    /// Before `editor` runs a command: ends a record on its buffer whose
    /// undo group closed since the last run, making it the last change.
    pub(crate) fn end_closed_record(&mut self, buffer: BufferId, in_group: bool) {
        if !in_group
            && self
                .recording
                .as_ref()
                .is_some_and(|recording| recording.buffer == buffer)
        {
            self.finish_record();
        }
    }

    /// Whether a record is open on `buffer`.
    pub(crate) fn is_recording(&self, buffer: BufferId) -> bool {
        self.recording
            .as_ref()
            .is_some_and(|recording| recording.buffer == buffer)
    }

    /// Adds `invocation` to the record open on its editor's buffer, or
    /// starts a new record on `buffer` with it. A record open on another
    /// buffer becomes the last change first.
    pub(crate) fn record(&mut self, buffer: BufferId, invocation: &Invocation) {
        match &mut self.recording {
            Some(recording) if recording.buffer == buffer => {
                recording.invocations.push(invocation.clone());
            }
            _ => {
                self.finish_record();
                self.recording = Some(Recording {
                    buffer,
                    invocations: vec![invocation.clone()],
                });
            }
        }
    }

    /// Takes back the last invocation recorded on `buffer`, after it
    /// failed, dropping the record if that was all it held.
    pub(crate) fn unrecord(&mut self, buffer: BufferId) {
        if let Some(recording) = &mut self.recording
            && recording.buffer == buffer
        {
            recording.invocations.pop();
            if recording.invocations.is_empty() {
                self.recording = None;
            }
        }
    }

    /// Makes the open record, if any, the last change.
    pub(crate) fn finish_record(&mut self) {
        if let Some(recording) = self.recording.take() {
            self.last_change = Some(recording.invocations);
        }
    }

    /// The invocations recorded so far on `buffer`, if a record is open
    /// there.
    pub(crate) fn recorded(&self, buffer: BufferId) -> Option<&[Invocation]> {
        self.recording
            .as_ref()
            .filter(|recording| recording.buffer == buffer)
            .map(|recording| recording.invocations.as_slice())
    }
}

/// The register `name` is stored under, or `None` if `name` isn't one
/// that holds text.
fn stored_name(name: char) -> Option<char> {
    match name {
        '"' | '0'..='9' | 'a'..='z' | '-' | '+' => Some(name),
        'A'..='Z' => Some(name.to_ascii_lowercase()),
        '*' => Some('+'),
        _ => None,
    }
}

/// `old` with `new` appended (Vim's `A` to `Z`): piece by piece when both
/// have as many pieces, and as one piece each otherwise. Linewise onto
/// charwise, or the reverse, is linewise, each charwise piece ending with
/// `newline`.
fn append(old: Register, new: Register, newline: &str) -> Register {
    let linewise = old.linewise || new.linewise;
    let lines = |register: Register| -> Vec<String> {
        let as_lines = linewise && !register.linewise;
        let pieces = register.pieces.into_iter().map(|mut piece| {
            if as_lines {
                piece.push_str(newline);
            }
            piece
        });
        pieces.collect()
    };
    let (old, new) = if old.pieces.len() == new.pieces.len() {
        (lines(old), lines(new))
    } else {
        let one = |register: Register| {
            let text = Register {
                pieces: vec![register.text(newline)],
                linewise: register.linewise,
            };
            lines(text)
        };
        (one(old), one(new))
    };
    Register {
        pieces: old.into_iter().zip(new).map(|(a, b)| a + &b).collect(),
        linewise,
    }
}
