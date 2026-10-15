//! Buffer versions and snapshots: read-only copies of a buffer that worker
//! threads read while the UI thread keeps editing. [`Version`] and
//! [`Snapshot`] hold the rules.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Buffer, BufferId, Point, TextBuffer, TextError, Utf16Point};

/// The version of one buffer in a [`BufferSet`]: how many times a
/// transaction has been applied, undone or redone on it since it was added.
///
/// A buffer starts at version 0 when [`BufferSet::add`] adds it. Each
/// successful [`apply`](BufferSet::apply), [`undo`](BufferSet::undo) and
/// [`redo`](BufferSet::redo) adds exactly one to the version of every buffer
/// the transaction touched, however many edits it had there. Each
/// successful [`amend`](crate::BufferSet::amend) adds one to the version of
/// every buffer its new edits touched. A transaction or amend whose edits
/// leave the text as it was still counts. Nothing else changes a version: not a failed `apply` or
/// `amend`, not an undo or redo that returns `Ok(None)` or an error, and not
/// [`get`](BufferSet::get),
/// [`version`](BufferSet::version) or [`snapshot`](BufferSet::snapshot).
/// Buffers a transaction didn't touch keep their versions.
///
/// Undo and redo give a buffer a new version, even though its text is then
/// byte for byte what it was at an earlier version. A version names a point
/// in the buffer's history, not a text, so versions only ever grow and an
/// older version is never current again. LSP needs the same of the version
/// it sends with each change, undo and redo included.
///
/// A version only means something together with the buffer it belongs to.
/// Comparing the versions of two different buffers means nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub(crate) u64);

impl Version {
    /// The version as a number: 0 for a newly added buffer, one more after
    /// each change.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// A read-only copy of one buffer's text at one [`Version`].
///
/// [`BufferSet::snapshot`] takes one in O(1) time and memory: the snapshot
/// shares the rope's nodes with the buffer instead of copying the text, and
/// later edits to the buffer copy the nodes they change instead of changing
/// shared ones. So the snapshot's text and version never change, whatever
/// happens to the buffer afterwards, and a snapshot stays readable after its
/// set is dropped. Cloning a snapshot is O(1) too.
///
/// Snapshots are [`Send`] and [`Sync`], so a worker thread can read one
/// while the UI thread keeps editing the buffer, and several threads can
/// read the same one at once. [`TextBuffer`] methods return exactly what
/// they would have returned on the buffer when the snapshot was taken,
/// errors included.
///
/// # Checking for a stale snapshot
///
/// A worker that reads a snapshot tags its result with
/// [`buffer`](Self::buffer) and [`version`](Self::version). When the result
/// reaches the UI thread, which owns the set, the UI thread compares that
/// version with [`BufferSet::version`] and drops the result if they differ.
/// That check is the one that decides, because nothing else can change the
/// buffer while the UI thread is running.
///
/// A worker can also call [`is_stale`](Self::is_stale) to give up early on
/// work whose result would be dropped anyway. It never blocks. Its `true` is
/// final, and its `false` only means the worker hasn't seen a change yet.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// The buffer's text when the snapshot was taken. Cloning a `Buffer`
    /// clones its rope, which shares every node instead of copying text.
    buffer: Buffer,
    /// Which buffer this is a snapshot of.
    id: BufferId,
    /// The buffer's version when the snapshot was taken.
    version: Version,
    /// The buffer's current version, shared with the set, which is its only
    /// writer.
    latest: Arc<AtomicU64>,
}

impl Snapshot {
    /// A snapshot of `buffer`, the text of buffer `id` at `version`, whose
    /// set stores each later version into `latest`.
    pub(crate) fn new(
        buffer: Buffer,
        id: BufferId,
        version: Version,
        latest: Arc<AtomicU64>,
    ) -> Self {
        Self {
            buffer,
            id,
            version,
            latest,
        }
    }

    /// The buffer this is a snapshot of.
    pub fn buffer(&self) -> BufferId {
        self.id
    }

    /// The buffer's version when the snapshot was taken.
    pub fn version(&self) -> Version {
        self.version
    }

    /// Whether the buffer has changed since the snapshot was taken, so its
    /// version is no longer [`version`](Self::version).
    ///
    /// Never blocks and takes no lock. It sees every change that happens
    /// before the call, in the sense of Rust's memory model: for example a
    /// change the UI thread made before sending this thread a message it has
    /// received. A change made at the same time as the call may or may not
    /// be seen. Once this returns `true` it returns `true` forever, because
    /// versions never go back.
    ///
    /// After the set is dropped, this keeps answering as of the set's last
    /// change.
    pub fn is_stale(&self) -> bool {
        // Relaxed: see `Entry::bump` in buffer_set.rs. Versions only grow, so
        // once a load sees a newer one, coherence keeps later loads from
        // seeing an older one.
        self.latest.load(Ordering::Relaxed) != self.version.0
    }
}

/// Writes the whole text, so `to_string()` returns it.
impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.buffer, f)
    }
}

impl TextBuffer for Snapshot {
    fn len_bytes(&self) -> usize {
        self.buffer.len_bytes()
    }

    fn len_chars(&self) -> usize {
        self.buffer.len_chars()
    }

    fn len_utf16(&self) -> usize {
        self.buffer.len_utf16()
    }

    fn len_lines(&self) -> usize {
        self.buffer.len_lines()
    }

    fn line(&self, line: usize) -> Result<String, TextError> {
        self.buffer.line(line)
    }

    fn slice(&self, range: Range<usize>) -> Result<String, TextError> {
        self.buffer.slice(range)
    }

    fn byte_to_char(&self, byte: usize) -> Result<usize, TextError> {
        self.buffer.byte_to_char(byte)
    }

    fn char_to_byte(&self, char: usize) -> Result<usize, TextError> {
        self.buffer.char_to_byte(char)
    }

    fn byte_to_utf16(&self, byte: usize) -> Result<usize, TextError> {
        self.buffer.byte_to_utf16(byte)
    }

    fn utf16_to_byte(&self, utf16: usize) -> Result<usize, TextError> {
        self.buffer.utf16_to_byte(utf16)
    }

    fn byte_to_line(&self, byte: usize) -> Result<usize, TextError> {
        self.buffer.byte_to_line(byte)
    }

    fn line_to_byte(&self, line: usize) -> Result<usize, TextError> {
        self.buffer.line_to_byte(line)
    }

    fn byte_to_point(&self, byte: usize) -> Result<Point, TextError> {
        self.buffer.byte_to_point(byte)
    }

    fn point_to_byte(&self, point: Point) -> Result<usize, TextError> {
        self.buffer.point_to_byte(point)
    }

    fn byte_to_utf16_point(&self, byte: usize) -> Result<Utf16Point, TextError> {
        self.buffer.byte_to_utf16_point(byte)
    }

    fn utf16_point_to_byte(&self, point: Utf16Point) -> Result<usize, TextError> {
        self.buffer.utf16_point_to_byte(point)
    }
}
