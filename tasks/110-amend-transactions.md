# T-110: Amend a transaction, so a run of keystrokes undoes in one step

After: T-102 and T-103. This task waits until both are merged. Both are.

## Goal

`BufferSet` gets `amend`, which folds new edits into a transaction that is already applied. The buffer changes at once, as with `apply`, but undo still treats the transaction as one step: one undo returns every buffer it touched to the text from before it, and one redo brings back the amended text. This is what lets the editor turn a run of typed characters into one undo step, and it starts from the method T-105 sketched in "Grouping keystrokes: a candidate for a later task".

PRD: ED-2

## Crates and public interface

Crates: `helm-text` (the implementation) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only).

The interface is already in `crates/helm-text/src/` with doc comments. `AmendError` is plain data and already works, `Display` and `Error` included. `BufferSet::amend` has a `todo!()` body. Fill it in. Don't change a signature, a type or a doc comment's contract, and don't change any T-101 to T-105 signature. If one looks wrong, say so in the PR description. Delete the `#[expect(unused_variables, ...)]` on `amend` once the stub is gone. Clippy with `-D warnings` fails until you do.

You will need to change `BufferSet`'s private fields and helpers. `Record`, `Change` and `Splice` are private, so reshape them freely, and move code between `apply` and `amend` if they share it. Don't touch `buffer.rs`, `encoding.rs` or `crates/helm-editor`.

```rust
// lib.rs
pub use error::{AmendError, HistoryError, TextError, TransactionError};

// buffer_set.rs
impl BufferSet {
    pub fn amend(&mut self, id: TransactionId, transaction: Transaction) -> Result<(), AmendError>;
}

// error.rs, next to TransactionError and HistoryError
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AmendError {
    UnknownTransaction { transaction: TransactionId },
    Undone { transaction: TransactionId },
    Untouched { edit: usize, buffer: BufferId },
    NotOnTop { edit: usize, buffer: BufferId },
    Transaction(TransactionError),
}
impl fmt::Display for AmendError;
impl std::error::Error for AmendError;   // source() is the wrapped TransactionError
```

`amend` returns `()` on success because the id doesn't change: the caller already has it. Every check `apply` makes comes back as `AmendError::Transaction`, with the same `TransactionError` and the same edit indexes `apply` would give, so a caller that already handles `apply`'s errors handles these too.

The spec PR also changed two doc comments to keep them true. `BufferSet`'s type docs now list `amend` among the ways to change a buffer, and `Version`'s docs in `snapshot.rs` say an amend adds one to each buffer it touches.

### Rules the implementation must follow

The doc comment on `amend` holds the full rules. In short:

- **When amending is allowed.** `id` must be applied, not undone, and on top of the undo history of every buffer the new edits touch. The new edits may only touch buffers `id` touched, though not necessarily all of them. A later transaction on a buffer the new edits don't touch doesn't stop the amend. It still blocks undoing `id`, under T-102's rule.
- **Undone, dropped or covered.** If `id` was undone and not redone, `amend` returns `Undone`. If a later `apply` or `amend` dropped it from the redo histories, or it comes from another set, `amend` returns `UnknownTransaction`. If a later transaction is on top of a buffer an edit names, `amend` returns `NotOnTop` naming that edit and buffer. In each case nothing changes, and the editor starts a new undo step with `apply` instead. A transaction that was undone and then redone is on top again and can be amended.
- **Offsets.** Every range is in bytes of the buffer's text after `id`. Since `id` is on top of that buffer's undo history, that is the buffer's current text. Within one amend, ranges refer to that text, not to the text after the edits listed before them, and placement and overlap follow `apply`'s rules.
- **Checks.** Everything is checked before anything changes, in this order, returning the first problem: `id` unknown (`UnknownTransaction`), `id` undone (`Undone`), no edits (`Transaction(Empty)`), then for each edit in order: a buffer not in the set (`Transaction(UnknownBuffer)`), a buffer `id` didn't touch (`Untouched`), a buffer where `id` isn't on top (`NotOnTop`), a range `slice` rejects on the current text (`Transaction(InvalidRange)`, with `slice`'s `TextError`), and overlap with an earlier edit on the same buffer (`Transaction(Overlap)`, naming the earliest). On any error, no buffer, history or version changes.
- **One undo step.** After any number of amends, undoing `id` from any buffer it touched restores every one of those buffers, byte for byte, to its text from before `id` was first applied. Redoing it restores each to its text after the last amend. A buffer `id` touched that no amend touched gets back exactly what `id` alone did to it.
- **Same id.** `id` keeps its id and its place in every undo history. `undo` and `redo` return it as before, and `amend` uses up no id, so the next `apply` still gets a larger one. T-105's `Editor` keys its record of selections by the id `apply` returned, and that record keeps working.
- **Redo.** `amend` drops the redo histories of the buffers its edits touch, with the same knock-on drops as `apply`. Buffers `id` touched that this amend doesn't touch keep their redo. A failed amend drops nothing.
- **Versions and snapshots.** A successful amend adds exactly one to the version of each buffer its edits touch, however many edits it had there. Other buffers keep their versions, including ones `id` touched that this amend didn't. Snapshots of the bumped buffers go stale as usual, and their text doesn't change. A later undo or redo of `id` adds one to every buffer `id` touched, as for any transaction. A no-op amend, such as an empty insert, still counts: it bumps versions and drops redo, as a no-op `apply` does.
- **Cost.** An amend costs about what applying its edits costs, plus work in proportion to the change `id` already holds for those buffers. It must not grow with the buffer's length, apart from the text the edits replace, and must not keep a list of amends to replay on undo. After 100 single-character amends of a typing run, undoing `id` should be one splice per buffer, as if the whole run had been one insert. Copying a buffer's text into a `String` to diff it doesn't meet this.
- **No panics.** Any offset, including `usize::MAX`, returns an error value.

### Composing the change

Each `Change` in a `Record` holds `forward` splices (text before `id` to text after it) and `inverse` splices (the reverse). Amending buffer B with new edits `E`, which refer to B's current text, needs a new `forward` from the text before `id` to the text after `E`, and its inverse.

One way that meets the cost rule: walk `forward` and `E` together in order of position. Map each `E` range back through `forward` to a range of the text before `id`. Where an `E` range touches or overlaps the output of a `forward` splice, merge them into one splice. Where it falls in text `id` didn't change, the text it replaces is the same before and after `id`, so you can read it from the buffer before splicing `E` in. The new `inverse` follows from the new `forward` and the replaced text. Merging splices that touch keeps a typing run at one splice. Any approach is fine if it meets the rules and the cost rule. Explain the one you chose in the PR.

### Choices I rejected

- **Return a new id from `amend`.** The editor's undo record is keyed by the id `apply` returned, so every amend would have to re-key it. Nothing gains from a new id.
- **Amend an undone transaction.** It sits in redo histories, and amending would either redo it silently or change text the user can't see. After an undo, the next keystroke should start a new step anyway.
- **Amend under a later transaction by rebasing.** Folding edits into `id` below a later transaction `t2` means undoing `id` later must also move `t2`'s splices. That is a history rewrite with no use in grouping keystrokes: once something else lands on the buffer, the group has ended.
- **Require `id` on top of every buffer it touched.** Only the buffers the new edits touch matter. The composition in buffer A depends only on A's history, and undoing `id` is still blocked by T-102's rule until the later transaction in B is undone.
- **Buffer keystrokes in the editor and apply them when the group ends.** T-105 already rejected this: the buffer must show each keystroke at once, because language servers and agents read the buffer.

### Dependencies

None new. `proptest` is already a dev-dependency.

## Acceptance tests

Two files, committed before the implementation:

| File | What it proves |
| --- | --- |
| `crates/helm-text/tests/amend.rs` | 30 hand-picked cases. Typing a word as one apply and four amends undoes and redoes in one step. `amend` returns `Ok(())`, the id stays the same through undo and redo, and the next `apply` still gets a larger id. Offsets read against the text after the transaction, an offset valid only before it is rejected, and ranges within one amend read against the text before the amend, in any listing order. Amends that delete text the transaction inserted, delete across its replacement, or backspace past it. Edits far from the transaction, inserts at one offset keeping their order, wide characters and an insert inside a CRLF. No-op amends. Amending after a redo, and amending an earlier transaction once the later one is undone. Amends across two buffers, amends that touch only some of the transaction's buffers, and a later transaction in another buffer that doesn't stop the amend but still blocks the undo. The errors: `NotOnTop` (with nothing applied when a later edit fails), `Undone` (redo still works after), `UnknownTransaction` for a dropped transaction and for an id from another set, `Untouched`, every wrapped `apply` error, every `InvalidRange` matching `slice` on the text after the transaction, and the full check order. Redo dropped in the amended buffers with knock-on drops, kept in buffers the amend doesn't touch and in unrelated ones, dropped by a no-op amend, and kept by a failed one. Versions bump once per amend per touched buffer and not at all on failure. Snapshots go stale only for the buffers an amend touches. |
| `crates/helm-text/tests/amend_properties.rs` | proptest over three buffers with random texts mixing ASCII, é, 中, 😀, every line break and any `char`. The first property applies a random first transaction to two sets, then runs a chain of up to 15 random amends (0 to 4 edits, some with out-of-bounds, reversed, mid-character, overlapping or untouched-buffer edits). One set amends; the other applies each amend's edits as its own transaction. After every step both sets hold the same texts and versions, and each error is `Untouched` or the other set's `apply` error, wrapped. Then one undo from a random touched buffer returns the amending set to the starting texts with a single bump per touched buffer and an empty undo history, while the other set needs one undo per transaction. One redo then brings back the final texts. The second property runs up to 40 random steps of applies, amends of the latest or any earlier transaction (some undone, dropped or covered), undos and redos, against an oracle that keeps each transaction's texts before it and after its last amend. Every result, error, text and version matches after every step. The third undoes everything that isn't blocked and checks every buffer is back to its starting text. 256 cases per property. |

Each test carries `#[ignore = "T-110: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-text -- --ignored` fails all 33 T-110 tests (30 + 3).

**The rule for the implementer: the only allowed change to these two files is deleting the `#[ignore = "T-110: not implemented yet"]` lines.** Nothing else in them changes. No edited assertion, no new `#[ignore]`, no changed case count, chain length or step count. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-7: no rope edit may get more than 5% slower than its base commit. Add a new file, `crates/helm-bench/benches/amend.rs`, with a `[[bench]] name = "amend"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds one criterion benchmark in a group named `amend`:

| Benchmark id | What one iteration does |
| --- | --- |
| `amend/type_100_chars_undo_10mb` | On a 10 MiB buffer in a `BufferSet`, applies a transaction that inserts one character at the char boundary at or below the middle, amends it 100 times, each amend inserting one character right after the one before, then undoes it once. |

Use the same 10 MiB text as `benches/text.rs`: copy its `LINES`, `floor_boundary` and `build_text` into the new file unchanged, so every file measures the same text. The 101 characters cycle through the chars of `"helm 😀中é"`, so the run mixes 1-, 2-, 3- and 4-byte characters. Build the text, the set, the first transaction and the 100 amend transactions once, outside the timed code. Clone them in `iter_batched` setup (`BatchSize::SmallInput`), since `apply` and `amend` take transactions by value. The undo returns the buffer to the starting text, and the next `apply` drops the redo entry, so every iteration starts from the same state.

This benchmark is the cost rule's guard. An `amend` that copies the buffer's text, or an undo that replays 100 amends, shows up here as a large number on its first run, though the gate only compares head with base.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `amend` benchmark, so `bench-compare` reports it as head-only in this task's PR and gates it from the next PR on. Changes to `apply` or the history types also run in `transactions/apply_undo_100_edits_10mb` and both `snapshots/` benchmarks, which this task must not slow by more than 5%.

## Out of scope

- Wiring `amend` into `Editor`: deciding when a group starts and ends (a pause, a cursor move, switching between typing and deleting, a newline), keeping the "before" selections of the group's first command, and updating T-105's undo record. A later `helm-editor` task, after T-106.
- Amending an undone transaction, or one under a later transaction. Both are refused, and the rejected choices above say why.
- Splitting an amended transaction back into its steps, or merging two transactions that are already applied.
- Streaming an agent's edits into one transaction with `amend` (AG-4): Phase 7 decides.
- Limits on history size or memory: a later task, with T-102's.
