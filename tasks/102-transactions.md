# T-102: Transactions with undo and redo, across several buffers

After: T-101. This task waits until that one is merged.

## Goal

`helm-text` gets a `BufferSet` that owns buffers and changes them only through transactions: lists of edits that apply, undo and redo as one unit. One transaction can touch several buffers, so an LSP rename (LS-5) or an agent's change (AG-4) across many files undoes in one step. Each buffer keeps its own undo order, and a transaction that touched several buffers is undone in all of them or in none.

PRD: ED-2

## Crates and public interface

Crates: `helm-text` (the implementation) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only).

The interface is already in `crates/helm-text/src/` with doc comments. `Edit`, `Transaction` and the error types are plain data and already work. `BufferSet`'s methods have `todo!()` bodies. Fill them in. Don't change a signature, a type or a doc comment's contract, and don't change any T-101 signature. If one looks wrong, say so in the PR description. `BufferSet`'s private fields are placeholders: replace them freely. Delete the `#![expect(unused_variables, dead_code, ...)]` at the top of `buffer_set.rs` once the stubs are gone. Clippy with `-D warnings` fails until you do.

```rust
// lib.rs
pub use buffer_set::BufferSet;
pub use error::{HistoryError, TextError, TransactionError};
pub use transaction::{BufferId, Edit, Transaction, TransactionId};

// transaction.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferId(pub(crate) u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionId(pub(crate) u64);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Edit { pub buffer: BufferId, pub range: Range<usize>, pub text: String }
impl Edit {
    pub fn insert(buffer: BufferId, byte: usize, text: impl Into<String>) -> Self;
    pub fn delete(buffer: BufferId, range: Range<usize>) -> Self;
    pub fn replace(buffer: BufferId, range: Range<usize>, text: impl Into<String>) -> Self;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Transaction { edits: Vec<Edit> }   // private field
impl Transaction {
    pub fn new() -> Self;
    pub fn push(&mut self, edit: Edit);
    pub fn edits(&self) -> &[Edit];
}
impl From<Vec<Edit>> for Transaction;
impl FromIterator<Edit> for Transaction;

// buffer_set.rs
#[derive(Debug)]
pub struct BufferSet { /* private */ }
impl BufferSet {
    pub fn new() -> Self;
    pub fn add(&mut self, buffer: Buffer) -> BufferId;
    pub fn get(&self, id: BufferId) -> Option<&Buffer>;
    pub fn apply(&mut self, transaction: Transaction) -> Result<TransactionId, TransactionError>;
    pub fn undo(&mut self, buffer: BufferId) -> Result<Option<TransactionId>, HistoryError>;
    pub fn redo(&mut self, buffer: BufferId) -> Result<Option<TransactionId>, HistoryError>;
}
impl Default for BufferSet;

// error.rs, next to TextError
pub enum TransactionError {
    Empty,
    UnknownBuffer { edit: usize, buffer: BufferId },
    InvalidRange { edit: usize, error: TextError },
    Overlap { edit: usize, other: usize },
}
pub enum HistoryError {
    UnknownBuffer { buffer: BufferId },
    Blocked { transaction: TransactionId, buffer: BufferId },
}
```

An insert is an edit with an empty range and a delete is an edit with empty text, so `Edit` is one replace. That is also the shape of an LSP `TextEdit`.

### Where the undo history lives

In the `BufferSet`, which owns its buffers. Inside it, each buffer has its own undo and redo stack, and a transaction sits on the stacks of every buffer it touched.

- **Owned, not referred to.** `get` lends buffers out read-only, so the only ways to change one are `apply`, `undo` and `redo`. Nothing can edit a buffer behind the history's back, and every rule below holds by construction. If the history only referred to buffers owned elsewhere, every undo would first need a version check to catch outside edits, and T-103's versions don't exist yet.
- **One stack per buffer, not one for the whole set.** With a single stack, pressing undo in file A would revert the typing you just did in file B. Editors don't work that way. VS Code keeps per-file undo plus workspace-wide edits, and Helm does the same. The cost is the blocking rule below.

### Rules the implementation must follow

The `BufferSet` docs hold the full rules. In short:

- **Offsets.** Every edit's range is in bytes of its buffer's text before the transaction, not after the edits listed before it. This is how an LSP `WorkspaceEdit` and a diff both describe their changes, so neither needs rebasing. In `"abcdef"`, deleting `0..2` and `2..4` in one transaction gives `"ef"`.
- **Placement.** Edits on one buffer must not overlap: ranges `a` and `b` overlap when `a.start < b.end && b.start < a.end`. Edits are placed by range start. At the same start, inserts come first in transaction order, then the one non-empty range. So the listing order only matters for inserts at one offset.
- **Checks.** `apply` checks every edit before it changes anything, in order, and returns the first problem. An empty transaction is `Empty`. For each edit: an unknown buffer (`UnknownBuffer`), then a range that `slice` would reject on the old text (`InvalidRange`, carrying the same `TextError`), then overlap with an earlier edit on the same buffer (`Overlap`, naming the earliest). On any error, no buffer and no history changes.
- **Undo and redo.** `undo(buffer)` reverts the transaction on top of `buffer`'s undo stack in every buffer it touched and moves it to their redo stacks. `redo(buffer)` does the reverse. Each returns the transaction's id, or `Ok(None)` when the stack is empty. Texts come back exactly, byte for byte.
- **A later edit blocks an undo.** A transaction can be undone only when it is on top of the undo stack of every buffer it touched. If a later transaction touched one of those buffers and is still applied, undo returns `HistoryError::Blocked` naming the transaction and the lowest-id blocking buffer, and changes nothing. Undoing that buffer first unblocks it. Redo follows the same rule with the redo stacks. The last transaction applied is never blocked, so undoing whatever isn't blocked, over and over, always gets every buffer back to where it started.
- **A new transaction drops redo.** `apply` drops the redo stacks of the buffers it touches. A dropped transaction leaves the redo stack of every buffer it touched, together with everything below it there, since those were applied after it and can't be redone without it. That repeats until nothing more drops. Other buffers keep their redo.
- **No-op transactions count.** A transaction whose edits leave the text as it was is still applied, recorded and undoable.
- **No panics.** Any offset, including `usize::MAX`, returns an error value.

I considered two other rules for the blocked case and rejected both:

- Undo the touched buffers that aren't blocked and leave the rest. This splits the transaction, which ED-2 forbids.
- Undo the later transactions first, automatically. That reverts work in other files the user didn't ask to undo.

VS Code refuses in the same case: its undo service won't undo a workspace edit across files when one of them has changed since. Offering "undo in this file only" after a refusal is a UI choice for a later phase, and it would need a new transaction, not a split one.

### Dependencies

None new. `proptest` is already a dev-dependency from T-101.

## Acceptance tests

Three files, committed before the implementation:

| File | What it proves |
| --- | --- |
| `crates/helm-text/tests/transactions.rs` | Applying transactions. Added buffers keep their text, and ids increase. Single-buffer inserts, deletes and replaces. Offsets read against the text before the transaction, in any listing order. An offset that is valid only after an earlier edit is rejected. Inserts at one offset keep their order, and inserts at either end of a replaced range land outside it. Edits across é, 中, 😀 and inside a CRLF. One transaction renaming across two buffers while a third stays untouched, and equal ranges in different buffers. No-op edits apply. The errors: `Empty`, `UnknownBuffer`, every `InvalidRange` kind (matching what `slice` returns), `Overlap` naming the earliest edit, the check order, and a failed transaction across three buffers changing none of them. |
| `crates/helm-text/tests/undo.rs` | Undo and redo of one transaction, empty stacks returning `Ok(None)`, undo and redo chains, exact text restored around CRLF and wide characters. One undo from any of five buffers reverting all four a transaction touched. Histories are per buffer. No-op transactions are recorded. A new transaction drops redo in its own buffers, keeps it in unrelated ones, removes a dropped shared transaction from every redo stack, and drops what was applied after it. The blocking rule for undo and for redo, including which buffer the error names and that a blocked call changes nothing. Unknown buffers. A failed `apply` keeps redo. |
| `crates/helm-text/tests/transaction_properties.rs` | proptest over three buffers with a plain `String` per buffer as the oracle, plus a model of the undo and redo stacks written straight from the rules above. Random starting texts mix ASCII, é, 中, 😀, every line break and any `char`. Up to 40 random steps mix transactions of 0 to 4 edits across random buffers (some with reversed, out-of-bounds or mid-character ranges, some overlapping), undos and redos. After every step, the result (id, `Ok(None)` or the exact error) matches the model, and every buffer equals its oracle `String`. A second property then undoes everything that isn't blocked until nothing is left, and checks that every buffer is back to its starting text with an empty undo stack. 256 cases per property. |

Each test carries `#[ignore = "T-102: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-text -- --ignored` fails all 35 T-102 tests.

**The rule for the implementer: the only allowed change to these three files is deleting the `#[ignore = "T-102: not implemented yet"]` lines.** Nothing else in them changes. No edited assertion, no new `#[ignore]`, no changed case count or text size. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-7: no rope edit may get more than 5% slower than its base commit. Adding to `benches/text.rs` would be an edit to a protected file, so add a new file instead: `crates/helm-bench/benches/transactions.rs`, with a `[[bench]] name = "transactions"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds one criterion benchmark in a group named `transactions`:

| Benchmark id | What one iteration does |
| --- | --- |
| `transactions/apply_undo_100_edits_10mb` | Applies one 100-edit transaction to a 10 MB buffer in a `BufferSet`, then undoes it. |

Use the same 10 MiB text as `benches/text.rs`: copy its `LINES`, `floor_boundary` and `build_text` into the new file unchanged, so both files measure the same text. Edit `i` (0 to 99) sits at the char boundary at or below `i * len / 100`. Even-numbered edits insert `"helm 😀"` there. Odd-numbered edits delete the one character that starts there. The edits are far apart, so none overlap. Build the text, the set and the transaction once, outside the timed code. Clone the transaction in `iter_batched` setup (`BatchSize::SmallInput`), since `apply` takes it by value. Each iteration's undo returns the buffer to the starting text, and the next `apply` drops the redo entry, so every iteration starts from the same state.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `transactions` benchmark, so `bench-compare` reports it as head-only in this task's PR and gates it from the next PR on.

## Out of scope

- Snapshots and buffer versions: T-103.
- Restoring selections and cursors on undo, and grouping keystrokes into one undo step: `helm-editor`, from T-105.
- Removing a buffer from a set (closing a file), and what that does to transactions that touched it: a later task.
- Limits on history size or memory, and keeping undo history across restarts: later tasks.
- "Undo in this file only" after a blocked multi-buffer undo: a UI choice for Phase 7.
- Converting LSP UTF-16 positions into byte ranges for an `Edit`: `helm-lsp` does that with T-101's conversions before it builds a transaction.
