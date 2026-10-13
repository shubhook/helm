# T-101: Buffer trait over ropey, with offset conversions

After: T-005. This task waits until that one is merged.

## Goal

`helm-text` gets a read-only `TextBuffer` trait and a `Buffer` type that implements it on top of ropey, with plain inserts and deletes. Every offset converts between bytes, chars, lines and UTF-16 code units, with the line rules LSP uses, and a bad offset returns an error value instead of panicking. ropey stays private to `helm-text`, so swapping it later touches one crate.

PRD: NF-7

## Crates and public interface

Crates: `helm-text` (the implementation) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only).

The interface is already in `crates/helm-text/src/` with `todo!()` bodies and doc comments. Fill in the bodies. Don't change a signature, a type or a doc comment's contract; if one looks wrong, say so in the PR description. Delete the `#![expect(unused_variables, dead_code, ...)]` at the top of `buffer.rs` once the stubs are gone. Clippy with `-D warnings` fails until you do.

```rust
// lib.rs
pub use buffer::Buffer;
pub use error::TextError;

pub struct Point { pub line: usize, pub column: usize }       // column in bytes
pub struct Utf16Point { pub line: usize, pub column: usize }  // column in UTF-16 code units
impl Point { pub const fn new(line: usize, column: usize) -> Self; }
impl Utf16Point { pub const fn new(line: usize, column: usize) -> Self; }

pub trait TextBuffer {
    fn len_bytes(&self) -> usize;
    fn len_chars(&self) -> usize;
    fn len_utf16(&self) -> usize;
    fn len_lines(&self) -> usize;
    fn line(&self, line: usize) -> Result<String, TextError>;
    fn slice(&self, range: Range<usize>) -> Result<String, TextError>;
    fn byte_to_char(&self, byte: usize) -> Result<usize, TextError>;
    fn char_to_byte(&self, char: usize) -> Result<usize, TextError>;
    fn byte_to_utf16(&self, byte: usize) -> Result<usize, TextError>;
    fn utf16_to_byte(&self, utf16: usize) -> Result<usize, TextError>;
    fn byte_to_line(&self, byte: usize) -> Result<usize, TextError>;
    fn line_to_byte(&self, line: usize) -> Result<usize, TextError>;
    fn byte_to_point(&self, byte: usize) -> Result<Point, TextError>;
    fn point_to_byte(&self, point: Point) -> Result<usize, TextError>;
    fn byte_to_utf16_point(&self, byte: usize) -> Result<Utf16Point, TextError>;
    fn utf16_point_to_byte(&self, point: Utf16Point) -> Result<usize, TextError>;
}

// buffer.rs
#[derive(Clone, Debug, Default)]
pub struct Buffer { rope: ropey::Rope }   // private field
impl Buffer {
    pub fn new() -> Self;
    pub fn insert(&mut self, byte: usize, text: &str) -> Result<(), TextError>;
    pub fn delete(&mut self, range: Range<usize>) -> Result<(), TextError>;
}
impl From<&str> for Buffer;
impl fmt::Display for Buffer;   // writes the whole text
impl TextBuffer for Buffer;

// error.rs
pub enum TextError {
    ByteOutOfBounds { offset: usize, len: usize },
    CharOutOfBounds { offset: usize, len: usize },
    Utf16OutOfBounds { offset: usize, len: usize },
    LineOutOfBounds { line: usize, len: usize },
    ColumnOutOfBounds { line: usize, column: usize },
    NotCharBoundary { offset: usize },
    InsideSurrogatePair { offset: usize },
    InvalidRange { start: usize, end: usize },
}
```

The trait is read-only so that T-103's snapshots can implement it too. Editing stays on `Buffer`.

### Rules the implementation must follow

The crate docs in `lib.rs` hold the full rules. In short:

- **Valid offsets.** A byte offset is valid when it is at most the length and on a char boundary. A UTF-16 offset is valid when it is at most the length and not between the halves of a surrogate pair. The length itself is always valid.
- **Line breaks.** LF, CRLF and lone CR, nothing else. CRLF is one break. U+2028, U+2029, NEL (U+0085), VT and FF are ordinary characters. A text with `k` breaks has `k + 1` lines. `line(n)` includes the line's own break.
- **Columns.** A column is valid when the offset it names is still inside its line: at most the line's length on the last line, before the line's last byte on every other line. The position between a CR and its LF is a valid column, because it is a valid byte offset and editing there must work. The position after a break is column 0 of the next line.
- **Error order.** Ranges check `start > end` first (`InvalidRange`), then `end` past the end (`ByteOutOfBounds` naming `end`), then `start`, then `end` for char boundaries. Points check the line, then the column against the line, then the char or surrogate boundary. For a point, `NotCharBoundary` and `InsideSurrogatePair` name the absolute offset the point resolved to, not the column.
- **No panics.** Any `usize`, including `usize::MAX` for an offset, line or column, returns a value. Watch for overflow in `line_start + column`.
- **Failed edits change nothing.** A rejected `insert` or `delete` leaves the buffer as it was.
- **No ropey types in public signatures.** `line` and `slice` return `String`, not `RopeSlice`.

### Dependencies

This spec PR already adds both to `crates/helm-text/Cargo.toml`. Don't change the versions or features.

- `ropey = "=1.6.1"` with `default-features = false, features = ["cr_lines", "simd"]`. 1.6.1 (18 October 2023) is the latest 1.x release. The only newer release, 2.0.0-beta.1, is a beta with a different API, so it is no base for a stable trait. 1.6.1 is MIT, which `deny.toml` allows. The features follow ropey's own docs ("Line Breaks" in the crate root docs): the default `unicode_lines` feature would count U+2028, NEL, VT and FF as line breaks, and `cr_lines` adds lone CR on top of the LF and CRLF ropey always counts. ropey treats CRLF as one break and never splits one across chunks. Turning off default features also turns off `simd`, so it is listed again.
- `proptest = "=1.12.0"`, dev-dependency only, for the property tests. It is the latest release.

## Acceptance tests

Three files, committed before the implementation:

| File | What it proves |
| --- | --- |
| `crates/helm-text/tests/examples.rs` | Lengths, `line`, `slice` and every conversion on hand-picked text: the empty buffer, ASCII, é (2 bytes), 中 (3 bytes), 😀 (4 bytes, a surrogate pair in UTF-16), mixed widths across lines, LF, CRLF, lone CR, LF followed by CR, CR followed by CRLF, a trailing line break of each kind, and U+2028, U+2029, NEL, VT and FF not breaking lines. Inserts and deletes, including ones that split a CRLF, join one back, or turn a CRLF into a lone CR. Clones stay independent. |
| `crates/helm-text/tests/errors.rs` | Each error variant for each method that can return it: byte offsets past the end (including `usize::MAX`) and inside 2-, 3- and 4-byte characters, char and UTF-16 offsets past the end, a UTF-16 offset inside a surrogate pair, lines past the end, columns past the end of a line (LF, CRLF and last line, in both column units), columns that split a character or a surrogate pair, reversed ranges, and the error order above. A rejected insert or delete leaves the buffer unchanged. |
| `crates/helm-text/tests/properties.rs` | proptest with a plain `String` as the oracle, over random text of up to about 1.5 KB mixing ASCII, é, 中, 😀, every line break, Unicode breaks and any `char`. Every length, every line and every conversion at sampled offsets match the `String`; every conversion round-trips; every invalid byte or UTF-16 offset returns the right error; slices match; and up to 40 random inserts and deletes leave the buffer equal to the same edits on the `String`. 128 cases per property, 32 sampled offsets plus both ends per case, to keep the file to a few seconds in a debug build. |

Each test carries `#[ignore = "T-101: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-text -- --ignored` fails all 36.

**The rule for the implementer: the only allowed change to these three files is deleting the `#[ignore = "T-101: not implemented yet"]` lines.** Nothing else in them changes: no edited assertion, no new `#[ignore]`, no changed case count or text size. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-7: no rope edit or offset lookup may get more than 5% slower than its base commit. Add a new file, `crates/helm-bench/benches/text.rs`, with a `[[bench]] name = "text"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds three criterion benchmarks in a group named `text`:

| Benchmark id | What one iteration does |
| --- | --- |
| `text/insert_middle_10mb` | Inserts a short string at the char boundary nearest the middle of a 10 MB buffer. Clone the buffer in `iter_batched` setup so every iteration starts from the same 10 MB text. |
| `text/byte_to_line_10mb` | Calls `byte_to_line` on 1,024 valid byte offsets spread evenly across the 10 MB buffer. |
| `text/utf16_to_byte_10mb` | Calls `utf16_to_byte` on 1,024 valid UTF-16 offsets spread evenly across the 10 MB buffer, none inside a surrogate pair. |

The text is 10 MiB of mixed ASCII and multibyte characters, built without randomness so base and head measure the same thing: repeat a fixed set of lines that mixes ASCII code with é, 中 and 😀 and uses both LF and CRLF, then cut it at the last char boundary at or below 10 MiB. Build the text, the buffer and the offset lists once, outside the timed code.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override.

Leave `benches/harness.rs` and its placeholder benchmark alone, even though its doc comment says T-101 replaces it. Removing it is a separate change.

Because the base commit has no `text` benchmarks, `bench-compare` reports them as head-only in this task's PR and gates nothing for them yet. From the next PR on, they are the rope baseline that Phase 1's gate asks for.

## Out of scope

- Transactions, undo and redo, and edits across several buffers: T-102.
- Snapshots and buffer versions: T-103.
- Detecting and preserving line endings and a BOM on save: T-104. This task only counts line breaks.
- Clamping an LSP position whose column is past the end of its line, which the LSP spec allows: `helm-lsp` in Phase 5. `helm-text` rejects it with `ColumnOutOfBounds`.
- File IO and non-UTF-8 files: `helm-project`.
- Moving to ropey 2.0: a later task, once 2.0 is stable.
