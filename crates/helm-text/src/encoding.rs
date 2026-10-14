//! Turning file bytes into text and back: the UTF-8 BOM, line ending
//! detection, and rejecting bytes that aren't UTF-8.

// The bodies below are stubs until T-104 is implemented. `expect` rather than
// `allow`, so the build fails once the stubs are gone and this line stays.
#![expect(
    unused_variables,
    reason = "T-104 interface stubs; the implementation deletes this"
)]

use std::fmt;

/// One line break: what a new line the user types is written as.
///
/// The default is [`LineEnding::Lf`], the line ending of a new file. See
/// [`LineEndings::newline`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineEnding {
    /// `\n`, used by macOS, Linux and most source repositories.
    #[default]
    Lf,
    /// `\r\n`, used by Windows.
    Crlf,
    /// A lone `\r`, used by classic Mac OS.
    Cr,
}

impl LineEnding {
    /// The line break as text: `"\n"`, `"\r\n"` or `"\r"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

/// Which line breaks a text uses, for showing to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineEndingStyle {
    /// The text has no line breaks.
    None,
    /// Every line break is this one.
    Uniform(LineEnding),
    /// The text has more than one kind of line break.
    Mixed,
}

/// How many line breaks of each kind a text has.
///
/// Line breaks are counted the way [`TextBuffer`](crate::TextBuffer) counts
/// lines: `\r\n` is one CRLF, a `\r` not followed by `\n` is a lone CR, and
/// a `\n` not preceded by `\r` is an LF. Nothing else is a line break. So
/// for any text, `lf + crlf + cr` is one less than
/// [`len_lines`](crate::TextBuffer::len_lines) of a buffer holding it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineEndings {
    /// Number of `\n` not preceded by `\r`.
    pub lf: usize,
    /// Number of `\r\n`.
    pub crlf: usize,
    /// Number of `\r` not followed by `\n`.
    pub cr: usize,
}

impl LineEndings {
    /// Counts the line breaks of `text`.
    pub fn count(text: &str) -> Self {
        todo!()
    }

    /// [`LineEndingStyle::None`] when every count is zero,
    /// [`LineEndingStyle::Uniform`] when exactly one count is non-zero, and
    /// [`LineEndingStyle::Mixed`] otherwise.
    pub fn style(&self) -> LineEndingStyle {
        todo!()
    }

    /// The line break to use for a new line the user types.
    ///
    /// The kind with the highest count. On a tie, [`LineEnding::Lf`] beats
    /// [`LineEnding::Crlf`], which beats [`LineEnding::Cr`]. So a text with
    /// no line breaks, such as a new or empty file, gets
    /// [`LineEnding::Lf`]: the convention on macOS, the platform Helm
    /// targets. `gofmt` always writes LF, and Prettier defaults to it.
    pub fn newline(&self) -> LineEnding {
        todo!()
    }
}

/// What a file's bytes hold besides its text: whether it starts with a
/// UTF-8 BOM, and the line breaks it had when it was opened.
///
/// [`FileFormat::decode`] returns one, the caller keeps it with the buffer,
/// and [`FileFormat::encode`] uses it to write the buffer back. The default
/// is the format of a new file: no BOM and no line breaks, so new lines use
/// [`LineEnding::Lf`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FileFormat {
    /// The file starts with the UTF-8 byte order mark `EF BB BF`.
    pub bom: bool,
    /// The line breaks in the file's text when it was decoded. Edits don't
    /// update this; call [`LineEndings::count`] on the current text for
    /// that.
    pub line_endings: LineEndings,
}

/// A file's text, borrowed from its bytes, and its [`FileFormat`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decoded<'a> {
    /// The file's text: its bytes without the BOM, if it had one. Line
    /// breaks are exactly as they were in the file.
    pub text: &'a str,
    /// The BOM and line breaks the file had.
    pub format: FileFormat,
}

impl FileFormat {
    /// Splits a file's bytes into its text and its format.
    ///
    /// The text keeps every line break as it is in the file: LF, CRLF and
    /// lone CR, mixed in any way. Nothing is normalized, so for any `bytes`
    /// that decode, encoding the text with the returned format gives back
    /// `bytes` exactly. To write a new line in the file's style, use
    /// [`FileFormat::newline`].
    ///
    /// The checks, in order:
    ///
    /// 1. If `bytes` starts with the UTF-8 BOM `EF BB BF`, the BOM is
    ///    removed, [`FileFormat::bom`] is true, and the rest must be valid
    ///    UTF-8. Only the first BOM is removed: a second one stays in the
    ///    text as U+FEFF. The UTF-16 checks below are skipped, since the
    ///    file says it is UTF-8.
    /// 2. If `bytes` starts with `FF FE` or `FE FF`, the UTF-16 little- or
    ///    big-endian BOM, the result is [`DecodeError::Utf16`] with `bom`
    ///    true. A UTF-32 little-endian BOM starts with `FF FE` and is
    ///    reported the same way.
    /// 3. If `bytes` looks like UTF-16 without a BOM, the result is
    ///    [`DecodeError::Utf16`] with `bom` false. It does when its length
    ///    is even, its first [`UTF16_SAMPLE_LEN`] bytes (or all of it, if
    ///    shorter) hold at least two 2-byte code units, and every one of
    ///    those code units has exactly one zero byte, always the second
    ///    (little-endian) or always the first (big-endian). That is ASCII
    ///    or Latin-1 text written as UTF-16. Valid UTF-8 text with no NUL
    ///    characters never matches. UTF-16 text with other characters
    ///    nearly always fails the UTF-8 check instead.
    /// 4. Otherwise `bytes` must be valid UTF-8, or the result is
    ///    [`DecodeError::InvalidUtf8`].
    ///
    /// Encodings other than UTF-8 are out of scope for `helm-text`.
    pub fn decode(bytes: &[u8]) -> Result<Decoded<'_>, DecodeError> {
        todo!()
    }

    /// The bytes to save for `text`: the UTF-8 BOM if [`FileFormat::bom`]
    /// is true, then `text` as UTF-8.
    ///
    /// Line breaks are written exactly as they are in `text` and never
    /// converted, whatever [`FileFormat::line_endings`] says. They are
    /// already right: decoding kept the file's own, and new lines get the
    /// file's style when they are typed, through [`FileFormat::newline`].
    /// Converting at save time would rewrite lines the user never touched,
    /// and has no single answer for a file with mixed line breaks.
    pub fn encode(&self, text: &str) -> Vec<u8> {
        todo!()
    }

    /// The line break to use for a new line the user types:
    /// [`LineEndings::newline`] of [`FileFormat::line_endings`].
    pub fn newline(&self) -> LineEnding {
        self.line_endings.newline()
    }
}

/// How many bytes from the start of a file [`FileFormat::decode`] looks at
/// to decide whether a file without a BOM is UTF-16.
pub const UTF16_SAMPLE_LEN: usize = 1024;

/// The order of the two bytes of a UTF-16 code unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ByteOrder {
    /// Low byte first, BOM `FF FE`. What Windows writes.
    LittleEndian,
    /// High byte first, BOM `FE FF`.
    BigEndian,
}

/// Why [`FileFormat::decode`] rejected a file's bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecodeError {
    /// The bytes aren't valid UTF-8.
    InvalidUtf8 {
        /// Byte offset in the file, counting the BOM if there is one, where
        /// the first invalid sequence starts. Every byte before it is valid
        /// UTF-8. A sequence cut off by the end of the file starts at its
        /// first byte.
        offset: usize,
    },
    /// The bytes are UTF-16.
    Utf16 {
        /// The byte order the BOM or the zero bytes show.
        byte_order: ByteOrder,
        /// The file starts with a UTF-16 BOM.
        bom: bool,
    },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::InvalidUtf8 { offset } => {
                write!(f, "invalid UTF-8 starting at byte {offset}")
            }
            Self::Utf16 { byte_order, bom } => {
                let order = match byte_order {
                    ByteOrder::LittleEndian => "little-endian",
                    ByteOrder::BigEndian => "big-endian",
                };
                let how = if bom { "with" } else { "without" };
                write!(f, "the file is {order} UTF-16 {how} a BOM, not UTF-8")
            }
        }
    }
}

impl std::error::Error for DecodeError {}
