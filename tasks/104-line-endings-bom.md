# T-104: Line ending and BOM detection, preserved on save

After: T-101. This task waits until that one is merged.

## Goal

`helm-text` gets `FileFormat`, which splits a file's bytes into its text plus what's needed to write them back: whether the file starts with a UTF-8 BOM, and how many LF, CRLF and lone CR line breaks it has. Encoding the text with that format gives back the file's bytes exactly, even when the file mixes line endings. Bytes that aren't UTF-8 return an error value: the offset where invalid UTF-8 starts, or "this is UTF-16".

PRD: ED-7

## Crates and public interface

Crates: `helm-text` (the implementation, in `src/encoding.rs` only) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only).

The interface is already in `crates/helm-text/src/encoding.rs` with doc comments. `LineEnding::as_str`, `FileFormat::newline`, the derives and `DecodeError`'s `Display` already work. `LineEndings::count`, `LineEndings::style`, `LineEndings::newline`, `FileFormat::decode` and `FileFormat::encode` have `todo!()` bodies. Fill them in. Don't change a signature, a type or a doc comment's contract, and don't change any T-101 or T-102 signature. If one looks wrong, say so in the PR description. Delete the `#![expect(unused_variables, ...)]` at the top of `encoding.rs` once the stubs are gone. Clippy with `-D warnings` fails until you do.

T-102 was being implemented in `buffer_set.rs` while this spec was written. Keep this task to `encoding.rs` so the two don't conflict: don't edit `buffer.rs`, `buffer_set.rs`, `transaction.rs`, `error.rs` or `lib.rs`.

```rust
// lib.rs
pub use encoding::{
    ByteOrder, DecodeError, Decoded, FileFormat, LineEnding, LineEndingStyle, LineEndings,
    UTF16_SAMPLE_LEN,
};

// encoding.rs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LineEnding { #[default] Lf, Crlf, Cr }
impl LineEnding { pub const fn as_str(self) -> &'static str; }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineEndingStyle { None, Uniform(LineEnding), Mixed }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct LineEndings { pub lf: usize, pub crlf: usize, pub cr: usize }
impl LineEndings {
    pub fn count(text: &str) -> Self;
    pub fn style(&self) -> LineEndingStyle;
    pub fn newline(&self) -> LineEnding;
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FileFormat { pub bom: bool, pub line_endings: LineEndings }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Decoded<'a> { pub text: &'a str, pub format: FileFormat }

impl FileFormat {
    pub fn decode(bytes: &[u8]) -> Result<Decoded<'_>, DecodeError>;
    pub fn encode(&self, text: &str) -> Vec<u8>;
    pub fn newline(&self) -> LineEnding;
}

pub const UTF16_SAMPLE_LEN: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ByteOrder { LittleEndian, BigEndian }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecodeError {
    InvalidUtf8 { offset: usize },
    Utf16 { byte_order: ByteOrder, bom: bool },
}
```

`decode` borrows the text from the input bytes instead of copying it. A caller builds a `Buffer` from `decoded.text`, which is the one copy it needs anyway.

### The buffer keeps the file's line endings

`decode` returns the text with every line break exactly as it is in the file. It doesn't convert CRLF or lone CR to LF. `encode` writes the BOM if there was one and then the text, and never converts line breaks either.

The other option is to normalize to LF on open and convert back on save. It fails ED-7 for a file with mixed line endings: once every break is LF, nothing records which ones were CRLF, so the save has to pick one style and rewrites lines the user never touched. Fixing that needs a per-line record of the original break, which every edit then has to keep in step. Keeping the bytes as they are costs nothing:

- `Buffer` already counts LF, CRLF and lone CR as line breaks (T-101), so line numbers and LSP positions are right with CRLF in the text.
- Language servers get exactly the text that is on disk. A server that reads a file from disk and one that gets it from Helm compute the same positions for it.
- Diffs and agent edits (AG-3) apply to the bytes on disk, so they line up.

What it costs: every new line has to be written in the file's style when it goes into the buffer, because the save won't fix it. `FileFormat::newline` gives that style, and `helm-editor` uses it when the user presses Enter. Converting pasted text, and a command to change a file's line endings, are `helm-editor`'s job in later tasks.

### Rules the implementation must follow

The doc comments in `encoding.rs` hold the full rules. In short:

- **Round trip.** For any `bytes` that `decode` accepts, `decoded.format.encode(decoded.text) == bytes`.
- **Counting.** `\r\n` is one CRLF, `\r` not followed by `\n` is a lone CR, and `\n` not preceded by `\r` is an LF. Nothing else counts, so `lf + crlf + cr + 1` equals `Buffer::len_lines` of the same text. `style` is `None` with no breaks, `Uniform` with one kind, and `Mixed` otherwise.
- **Typed newline.** `newline` is the kind with the highest count. On a tie, LF beats CRLF, which beats CR. With no line breaks at all (a new file, an empty file or a one-line file) it is LF. Helm targets macOS, where LF is the convention, `gofmt` always writes LF, and Prettier defaults to LF. `FileFormat::default()` is a new file's format: no BOM, LF.
- **`decode`'s checks, in order.**
  1. Bytes starting with `EF BB BF` have a BOM. It's removed and `bom` is true. Only the first one is removed; a second stays in the text as U+FEFF. The rest must be valid UTF-8, and the UTF-16 checks are skipped.
  2. `FF FE` or `FE FF` at the start is a UTF-16 BOM: `Utf16 { bom: true }`, little- or big-endian. A UTF-32 little-endian BOM starts with `FF FE` and is reported the same way.
  3. Bytes that look like UTF-16 without a BOM are `Utf16 { bom: false }`. They do when the length is even, the first `UTF16_SAMPLE_LEN` (1024) bytes, or all of them if fewer, hold at least two code units, and every one of those code units has exactly one zero byte, always the second (little-endian) or always the first (big-endian).
  4. Anything else must be valid UTF-8, or the result is `InvalidUtf8 { offset }`.
- **The invalid UTF-8 offset** is where the first invalid sequence starts, counted in the file's bytes, BOM included. It's what `std::str::Utf8Error::valid_up_to` gives, plus 3 when there's a BOM. A character cut off by the end of the file starts at its first byte.
- **No panics.** Any byte string returns a value.

Rule 3 is deliberately narrow. It catches ASCII and Latin-1 text written as UTF-16, the case that is otherwise valid UTF-8 full of NULs and would open as garbage. UTF-16 with other characters almost always fails the UTF-8 check in rule 4, so it is still an error, just a less specific one. Real UTF-8 source files don't contain NUL, so rule 3 never fires on one.

### Dependencies

None new. `proptest` is already a dev-dependency from T-101.

## Acceptance tests

Two files, committed before the implementation:

| File | What it proves |
| --- | --- |
| `crates/helm-text/tests/encoding.rs` | The empty file, a BOM alone, a BOM before LF and CRLF content, two BOMs (only the first removed), U+FEFF after the start staying text. LF, CRLF and lone CR files keep their breaks in the text and report `Uniform` with the right typed newline. Mixed endings, with and without a BOM, round-trip byte for byte and report `Mixed`. Tie order for `newline`, `style` from counts, CR before CRLF, LF before CR, and U+2028, U+2029, NEL, VT and FF not counting. Counts agree with `Buffer::len_lines`. A final newline kept, and a missing one not added, for each kind. `encode` adds the BOM and never converts line breaks. A typed line using `newline()` saves as the file's style. `FileFormat::default()`. Invalid UTF-8 at the start (stray continuation byte, 0xFF, overlong, a BOM cut short), in the middle (bad continuation, CESU-8 surrogate, a byte after 😀, past U+10FFFF) and at the end (cut-off 中 and 😀, a trailing continuation byte), with offsets counting the BOM. UTF-16 and UTF-32 BOMs, UTF-16 without a BOM in both byte orders, near misses that must decode as UTF-8, and the 1024-byte sample. |
| `crates/helm-text/tests/encoding_properties.rs` | proptest, 256 cases per property. (1) Random valid UTF-8 of up to about 1.5 KB mixing ASCII, é, 中, 😀, LF, CR, CRLF, U+FEFF, Unicode breaks and any `char` except NUL, with or without a BOM, decodes to the right text, BOM flag, counts, style and typed newline, and encodes back to the same bytes. The counts also match `Buffer::len_lines`. (2) Random valid UTF-8 that includes NULs and UTF-16-looking runs either round-trips or is `Utf16 { bom: false }`, matching rule 3 exactly. (3) Random bytes, with prefixes that are a BOM, a UTF-16 BOM or part of a BOM, plus near-UTF-16 runs with a broken unit or an odd length, decode to exactly what an oracle written from the rules above says, and round-trip when they decode. (4) Latin-1 text written as UTF-16 in either byte order, with or without a BOM, is rejected with the right byte order and BOM flag. |

Each test carries `#[ignore = "T-104: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-text -- --ignored` fails all 35 T-104 tests (31 + 4).

**The rule for the implementer: the only allowed change to these two files is deleting the `#[ignore = "T-104: not implemented yet"]` lines.** Nothing else in them changes. No edited assertion, no new `#[ignore]`, no changed case count or text size. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-7: no rope edit or offset lookup may get more than 5% slower than its base commit, and opening and saving a file sit on the same path. Add a new file, `crates/helm-bench/benches/encoding.rs`, with a `[[bench]] name = "encoding"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds one criterion benchmark in a group named `encoding`:

| Benchmark id | What one iteration does |
| --- | --- |
| `encoding/decode_encode_bom_10mb` | Decodes 10 MiB of file bytes that start with a UTF-8 BOM, then encodes the decoded text with the decoded format. |

Use the same 10 MiB text as `benches/text.rs`: copy its `LINES`, `floor_boundary` and `build_text` into the new file unchanged, so both files measure the same text. The file bytes are `EF BB BF` followed by that text. Its lines mix LF and CRLF, so decoding counts both kinds. Build the bytes once, outside the timed code. Pass the decode result and the encoded bytes through `black_box` so neither is optimized away.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `encoding` benchmark, so `bench-compare` reports it as head-only in this task's PR and gates it from the next PR on.

## Out of scope

- Reading and writing files, including the temp-file-and-rename save (NF-8): `helm-project`.
- Detecting outside changes to a file and reloading it (FP-6): `helm-project`.
- Encodings other than UTF-8, including opening UTF-16 files: `helm-project`, if ever. `helm-text` only reports them.
- Converting line breaks in pasted text, and a command to change a file's line endings or add or remove its BOM: `helm-editor`, in later tasks.
- Keeping `FileFormat::line_endings` up to date as the buffer is edited. Callers recount with `LineEndings::count` when they need current numbers.
- Large files that open lazily (FP-7): a later task. `decode` takes the whole file.
