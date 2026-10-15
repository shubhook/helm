# T-105: Selections and multiple cursors

After: T-102. This task waits until that one is merged.

## Goal

`helm-editor` gets `Selection`, `Selections` and an `Editor` that edits one buffer of a `BufferSet` at every cursor at once. Typing, newline and deletion at N cursors apply as one transaction, so one undo reverts them all and puts the cursors back where they were. Add next match, add cursor above and below, and select all matches (ED-3) add selections. Every offset is in bytes, the unit `helm-text` edits in.

PRD: ED-3

## Crates and public interface

Crates: `helm-editor` (the implementation) and `helm-bench` (the new benchmark file, its `[[bench]]` entry, and a `helm-editor` path dependency in `crates/helm-bench/Cargo.toml` only).

The interface is already in `crates/helm-editor/src/` with doc comments. `Selection`'s methods, `Selections::single`, `as_slice`, `primary` and `primary_index`, `Editor::new`, `buffer`, `format`, `selections` and `set_selections`, and `EditorError` with its `From<HistoryError>` and `Display` already work. `Selections::new`, `Selections::push` and every other `Editor` method have `todo!()` bodies. Fill them in. Don't change a signature, a type or a doc comment's contract. If one looks wrong, say so in the PR description. `Editor`'s private fields are placeholders: replace them freely. Delete the `#![expect(unused_variables, dead_code, ...)]` at the top of `lib.rs` once the stubs are gone. Clippy with `-D warnings` fails until you do.

Don't touch `crates/helm-text`. T-103 is being written there at the same time, and this task needs nothing new from it.

```rust
// lib.rs
pub use editor::Editor;
pub use error::EditorError;
pub use selection::{Selection, Selections};

// selection.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Selection { pub anchor: usize, pub head: usize }
impl Selection {
    pub const fn new(anchor: usize, head: usize) -> Self;
    pub const fn cursor(offset: usize) -> Self;
    pub const fn start(&self) -> usize;
    pub const fn end(&self) -> usize;
    pub const fn range(&self) -> Range<usize>;
    pub const fn is_empty(&self) -> bool;
    pub const fn is_reversed(&self) -> bool;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selections { selections: Vec<Selection>, primary: usize }   // private fields
impl Selections {
    pub fn single(selection: Selection) -> Self;
    pub fn new(selections: Vec<Selection>, primary: usize) -> Option<Self>;
    pub fn push(&mut self, selection: Selection);
    pub fn as_slice(&self) -> &[Selection];
    pub fn primary(&self) -> Selection;
    pub fn primary_index(&self) -> usize;
}

// editor.rs
#[derive(Clone, Debug)]
pub struct Editor { /* private */ }
impl Editor {
    pub fn new(buffer: BufferId, format: FileFormat) -> Self;
    pub fn buffer(&self) -> BufferId;
    pub fn format(&self) -> FileFormat;
    pub fn selections(&self) -> &Selections;
    pub fn set_selections(&mut self, selections: Selections);

    pub fn insert(&mut self, set: &mut BufferSet, text: &str) -> Result<Option<TransactionId>, EditorError>;
    pub fn newline(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError>;
    pub fn delete_backward(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError>;
    pub fn delete_forward(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError>;

    pub fn add_next_match(&mut self, set: &BufferSet) -> Result<bool, EditorError>;
    pub fn select_all_matches(&mut self, set: &BufferSet) -> Result<bool, EditorError>;
    pub fn add_cursor_above(&mut self, set: &BufferSet) -> Result<bool, EditorError>;
    pub fn add_cursor_below(&mut self, set: &BufferSet) -> Result<bool, EditorError>;

    pub fn undo(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError>;
    pub fn redo(&mut self, set: &mut BufferSet) -> Result<Option<TransactionId>, EditorError>;
}

// error.rs
pub enum EditorError {
    UnknownBuffer { buffer: BufferId },
    Blocked { transaction: TransactionId, buffer: BufferId },
}
impl From<HistoryError> for EditorError;
```

### Why the editor holds an id, not the buffer

T-102's `BufferSet` owns its buffers, and the only way to change one is a transaction. So `Editor` keeps a `BufferId` and every command takes the set. That keeps undo in one place: an agent's multi-file edit and a keystroke land on the same per-buffer history, and the editor never edits behind the history's back. Commands that only change selections take `&BufferSet`, so they can't edit anything.

### Rules the implementation must follow

The doc comments in `selection.rs` and `editor.rs` hold the full rules. In short:

- **Selections.** A selection is an anchor and a head, in bytes. A cursor is a selection with `anchor == head`. `head < anchor` points left.
- **Normal form.** `Selections` is never empty, is sorted by start, and has one primary. Two given selections join when they overlap (`b.start() < a.end()`) or touch (`b.start() == a.end()`) with at least one of them empty. Joining is transitive, so a cursor where two selections touch joins all three. Two non-empty selections that only touch stay apart. That is what makes "select all matches" of `a` in `"aaa"` give three selections, not one. The joined selection points the way the primary does if the primary is in it and isn't empty, otherwise the way the first non-empty one in input order does. The primary is whatever the old primary joined.
- **Clamping.** Before a command reads the selections, it moves every offset past the end to the end and every offset inside a character back to that character's first byte. The position between CR and LF stays, since it is a valid byte offset. Clamping reads only the offsets it checks and never copies the text. Stale selections never cause an error or a panic.
- **One transaction per command.** `insert`, `newline`, `delete_backward` and `delete_forward` build one edit per selection, with ranges in the text before the command, and apply them as one `Transaction`. A command with nothing to do (empty text at cursors only, backspace with every cursor at 0) applies nothing and returns `Ok(None)`.
- **Where selections land.** After typing, every selection is a cursor right after its copy of the text. After a delete, a selection that deleted something is a cursor where its range started, and one that didn't stays put. Every offset moves by the bytes added and removed before it. Then the set goes back to normal form, and the primary is what the old primary became.
- **Newline.** `newline` inserts `self.format().newline().as_str()`, so a CRLF file gets `"\r\n"` and a new file gets `"\n"`. Never a hard-coded `"\n"`. No indentation.
- **Deleting.** A non-empty selection deletes its range. A cursor deletes one Unicode scalar value before it (backward) or after it (forward), or both bytes of a CRLF next to it. Between CR and LF, backward deletes the CR and forward deletes the LF. Ranges that overlap, such as two cursors on either side of an LF, are joined before the transaction is built, because `BufferSet::apply` rejects overlapping edits.
- **`insert` doesn't convert line breaks.** Text goes in exactly as given. Converting pasted line breaks is a later task.
- **Words.** A word character is `char::is_alphanumeric` or `_`. The word around a cursor runs left while the character before is one and right while the character after is one. So é and 中 are word characters and 😀 is not.
- **Add next match.** With a cursor as primary, the primary becomes its word and nothing else changes. With a non-empty primary, the search is an exact, case-sensitive match of its bytes. Every occurrence counts, including ones that overlap each other: first those starting at or after the primary's end, then, wrapping around, those starting before it. The first one that overlaps no existing selection is added, pointing forward, as the new primary.
- **Select all matches.** Replaces every selection with each match of the primary's text (or its word), found left to right without overlaps, each pointing forward. The primary is the first match that overlaps the range searched for.
- **Add cursor above and below.** Every selection adds a cursor on the line above or below its head. The column is in chars from the line's start, capped at the line's text without its line break. On a shorter line the cursor goes to the end of that line's text. The first or last line adds nothing. The cursor added for the old primary becomes the primary.
- **Undo and redo.** `Editor::undo` calls `BufferSet::undo` on the editor's buffer. If this editor applied that transaction, the selections go back to exactly what they were just before the command, after clamping, with the same primary. `redo` restores the selections from just after it. A transaction this editor didn't apply, such as an agent's edit, leaves the selections alone. A blocked undo or redo returns `EditorError::Blocked` and changes nothing. Moving or adding selections is not a transaction and is not undone.
- **No panics.** Any offset, including `usize::MAX`, is clamped. An editor handed a set without its buffer returns `EditorError::UnknownBuffer` from every command.

### Choices I made, and the ones I rejected

- **Touching non-empty selections stay apart.** VS Code does the same: its cursor collection merges touching selections only when one is empty. The other option, joining anything that touches, turns "select all matches" of `aa` in `"aaaa"` into one selection, which then types one copy instead of two.
- **The column for add above and below is in chars, and it isn't sticky.** Display columns need tab width and the width of wide characters like 中, which is `helm-ui`'s job. A remembered goal column (keep column 5 across a short line) belongs with cursor movement in T-106 and T-107. The tests never chain above or below through a short line, so adding a goal column later won't break them.
- **Matches are plain substrings.** VS Code switches to whole-word matching when "add next match" starts from a cursor. Whole-word and case options belong with the find commands.
- **Selections clamp instead of erroring.** An agent can edit a buffer under an open editor. Mapping selections through someone else's edits needs the transaction's edits, which `BufferSet` doesn't expose. Clamping is the safe fallback until then.

### Dependencies

`helm-text` was already a dependency of `helm-editor`. This spec PR adds one dev-dependency to `crates/helm-editor/Cargo.toml`:

- `proptest = "=1.12.0"`, dev-dependency only, for the property tests. It's the version `helm-text` already pins, so `Cargo.lock` gains no new package.

The benchmark needs `helm-editor = { path = "../helm-editor" }` in `crates/helm-bench/Cargo.toml`'s `[dependencies]`. Add it in the implementation PR, next to `helm-text`.

## Acceptance tests

Four files, committed before the implementation:

| File | What it proves |
| --- | --- |
| `crates/helm-editor/tests/selections.rs` | `Selection`'s accessors. `Selections::new` rejects empty input and a primary out of range, sorts, joins equal cursors, overlapping and nested selections, a cursor at either end of a selection, and a cursor where two selections touch. Two non-empty selections that only touch stay apart. Join chains in any input order. The joined direction follows the primary, then input order. `push` makes the pushed selection primary. Offsets up to `usize::MAX` aren't checked. |
| `crates/helm-editor/tests/editing.rs` | Typing at three cursors as one transaction that one `BufferSet::undo` reverts. Typing over selections pointing either way and over touching selections. Offsets across é, 中 and 😀. Empty text at cursors applies nothing. Typed line breaks go in unconverted, including between CR and LF. Newline in CRLF, LF, lone CR, mixed and new files, decided by the editor's format and not the text. Backspace and delete over each character width, around CRLF (after, before and between CR and LF), at the start and end, with joined overlapping ranges, and over selections. Stale selections clamped, merged by clamping, and kept by a command with nothing to do. Every command returning `UnknownBuffer`. Undo and redo restoring selections, direction and primary, through a chain of commands. Clamped selections are what undo restores. Moving selections isn't undone. An outside transaction leaves the selections alone. A blocked undo changes nothing. |
| `crates/helm-editor/tests/multi_cursor.rs` | Add next match: the word first, then the next match, wrapping, only touching the primary, no word between punctuation, `_` and digits in words, words with é and 中 but not 😀, a cursor inside é clamped first, a selection's own text as the search, a leftwards primary, case-sensitivity, a match across CRLF, overlapping occurrences skipped or used. Select all matches: from a word, replacing other selections, never overlapping, no word, then typing and undoing, multibyte text. Add cursor above and below: building a column, columns in chars over 😀, é, 中 and a short line, CRLF and lone CR, the head of a selection, merging into an existing selection, a one-line buffer. |
| `crates/helm-editor/tests/properties.rs` | proptest, 256 cases per property, over random text mixing ASCII, é, 中, 😀, LF, CR, CRLF and any `char`, with up to 16 random selections on char boundaries. (1) ED-3: typing at N cursors gives the same text as N single-cursor editors typing one after another from the last cursor to the first, and the same text and cursors as a `String` oracle. (2) The same for newline, with the file's newline. (3, 4) Backspace and delete match a `String` oracle written from the rules above, text and cursors, and return `Ok(None)` exactly when nothing is deleted. (5) Any list of selections, in any order, normalizes to the documented form: sorted, no overlaps, touching only when both are non-empty, nothing lost, every group connected, each input in the right group, and the right direction and primary. (6) A random session of up to 23 commands mixing typing, newline, deletes, new selections and the four ED-3 commands, then undoing everything, restores each text and selection set in reverse, and redoing restores them forwards. |

Each test carries `#[ignore = "T-105: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-editor --no-fail-fast -- --ignored` fails all 60 (12 + 22 + 20 + 6).

**The rule for the implementer: the only allowed change to these four files is deleting the `#[ignore = "T-105: not implemented yet"]` lines.** Nothing else in them changes. No edited assertion, no new `#[ignore]`, no changed case count or text size. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-7: no rope edit may get more than 5% slower than its base commit, and typing at many cursors is a rope edit per cursor. NF-1 (keypress to pixel under 5 ms) is measured from Phase 2; this benchmark is its headless part. Add a new file, `crates/helm-bench/benches/editor.rs`, with a `[[bench]] name = "editor"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds one criterion benchmark in a group named `editor`:

| Benchmark id | What one iteration does |
| --- | --- |
| `editor/type_undo_1000_cursors_10mb` | Types `"x"` at 1,000 cursors in a 10 MiB buffer with `Editor::insert`, then undoes it with `Editor::undo`. |

Use the same 10 MiB text as `benches/text.rs`: copy its `LINES`, `floor_boundary` and `build_text` into the new file unchanged, so every file measures the same text. Cursor `i` (0 to 999) sits at the char boundary at or below `i * len / 1000`. Build the text, the set, the editor and its selections once, outside the timed code. The undo puts back the text and the 1,000 cursors, and the next `insert` drops the redo entry, so every iteration starts from the same state. Pass both results through `black_box`.

The implementation must not copy the buffer's text to type, delete or clamp. `select_all_matches` and `add_next_match` may read the whole text for now. Moving search to a worker is a later task (NF-9).

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `editor` benchmark, so `bench-compare` reports it as head-only in this task's PR and gates it from the next PR on.

## Grouping keystrokes: a candidate for a later task

Typing "hello" today makes five transactions and five undo steps. Editors group a run of typing into one undo step. That needs a `helm-text` method that doesn't exist, so T-105 leaves grouping out and makes one transaction per command.

The method would go on `BufferSet`:

```rust
/// Folds `transaction` into transaction `id`, so one undo reverts both.
///
/// `id` must be on top of the undo history of every buffer `transaction`
/// touches, and `transaction` must touch only buffers `id` touched.
/// `transaction`'s ranges refer to the text after `id`, as for `apply`.
/// Like `apply`, it drops redo and checks every edit before changing
/// anything.
pub fn amend(&mut self, id: TransactionId, transaction: Transaction) -> Result<(), AmendError>;
```

`AmendError` would carry `apply`'s `TransactionError` cases plus "`id` isn't on top of buffer B" and "touches buffer B, which `id` didn't". Internally, `amend` composes the stored forward and inverse splices of `id` with the new edits per buffer, so undo restores the text from before `id` in one step.

With it, `Editor` would amend instead of apply while a group is open, and keep the "before" selections of the group's first command. When a group ends (a pause, a cursor move, switching between typing and deleting, a newline) is `helm-editor`'s policy for that later task.

I considered buffering keystrokes in the editor and applying them as one transaction when the group ends. That fails because the buffer must show each keystroke at once, and language servers and agents read the buffer, not the editor.

## Out of scope

- Grouping keystrokes into one undo step: a later task, after the `BufferSet::amend` method above.
- Cursor movement, a sticky goal column, word and line motions, and extending selections with the keyboard: T-106 to T-108.
- Key bindings for these commands: T-107 (VS Code) and T-108 (Vim).
- Undoing selection changes on their own (VS Code's "cursor undo"): a later task.
- Whole-word and case-insensitive matching, and find and replace: later, with the find commands.
- Grapheme clusters. Backspace deletes one Unicode scalar value, so a flag emoji or e plus a combining accent takes two presses: a later task, which may need `unicode-segmentation`.
- Converting line breaks in pasted text, auto-indent on newline (SY-3), and auto-closing brackets: later tasks.
- Mapping selections through edits made outside the editor, and two editors on one buffer (split panes, FP-5): a later task. T-105 clamps.
- Limits on the editor's record of selections per transaction: with T-102's history limits, later.
- Display columns, tab width and wide characters: `helm-ui`.
