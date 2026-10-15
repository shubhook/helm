# T-103: Cheap snapshots tagged with a buffer version

After: T-101. This task waits until that one is merged. It also builds on T-102's `BufferSet`, which is merged.

## Goal

Every buffer in a `BufferSet` gets a `Version` that grows by one each time a transaction is applied, undone or redone on it. `BufferSet::snapshot` returns a `Snapshot`: a read-only copy of the buffer's text tagged with that version. It costs O(1) to take, is `Send + Sync`, and implements `TextBuffer`, so parsing, LSP, search and agent code can read it on worker threads while the UI thread keeps editing (NF-9). A worker's result carries the version it read, and the UI thread drops it if the buffer has moved on.

PRD: NF-9

## Crates and public interface

Crates: `helm-text` (the implementation) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only).

The interface is already in `crates/helm-text/src/snapshot.rs` with doc comments. `Version::get` already works. Everything else has a `todo!()` body. Fill the bodies in. Don't change a signature, a type or a doc comment's contract, and don't change any T-101, T-102 or T-104 signature. If one looks wrong, say so in the PR description. `Snapshot`'s private fields are placeholders, so replace them freely. Delete the `#![expect(unused_variables, dead_code, ...)]` at the top of `snapshot.rs` once the stubs are gone. Clippy with `-D warnings` fails until you do.

This spec PR only added `snapshot.rs` and two lines in `lib.rs`, to stay out of the way of other threads. The implementation will need to change `buffer_set.rs`: each buffer's entry needs a version, and `apply` and `undo`/`redo` have to bump it. You can also move the two `impl BufferSet` methods from `snapshot.rs` into `buffer_set.rs`, or make `BufferSet`'s fields `pub(crate)`. Either is fine. Don't touch `buffer.rs`, `encoding.rs` or `crates/helm-editor`.

```rust
// lib.rs
pub use snapshot::{Snapshot, Version};

// snapshot.rs
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub(crate) u64);
impl Version { pub const fn get(self) -> u64; }

#[derive(Clone, Debug)]
pub struct Snapshot { /* private */ }
impl Snapshot {
    pub fn buffer(&self) -> BufferId;
    pub fn version(&self) -> Version;
    pub fn is_stale(&self) -> bool;
}
impl fmt::Display for Snapshot;   // writes the whole text
impl TextBuffer for Snapshot;

impl BufferSet {
    pub fn version(&self, id: BufferId) -> Option<Version>;
    pub fn snapshot(&self, id: BufferId) -> Option<Snapshot>;
}
```

Both `BufferSet` methods return `None` for a buffer the set doesn't have, like `get` does. No new error type.

### Where the version lives

In the `BufferSet`, per buffer, not on `Buffer`. `apply`, `undo` and `redo` are the only ways to change a buffer in a set (T-102), so the set sees every change and nothing can edit a buffer behind its version's back. A standalone `Buffer` still has `insert` and `delete` and no version. Workers only ever read buffers that belong to a set, so a standalone `Buffer` doesn't need one, and leaving it out keeps `buffer.rs` unchanged.

### Rules the implementation must follow

The doc comments on `Version` and `Snapshot` hold the full rules. In short:

- **Start.** `add` gives a buffer version 0.
- **What changes a version.** Each successful `apply`, `undo` and `redo` adds exactly one to the version of every buffer the transaction touched. That's one per transaction, however many edits it had in that buffer. An undo started from buffer B that reverts A and B bumps both. A no-op transaction still counts, since T-102 records it and it can be undone.
- **What doesn't.** A failed `apply`, an undo or redo that returns `Ok(None)`, `Blocked` or `UnknownBuffer`, and `get`, `version` and `snapshot`. Buffers a transaction didn't touch keep their versions. So do buffers whose redo history a new transaction drops, because their text doesn't change.
- **Snapshots.** `snapshot` costs O(1) whatever the buffer's size. It clones the rope (ropey's clone bumps a reference count on the root and shares every node) and whatever the staleness check needs. It doesn't copy text, take a lock, or allocate in proportion to the text. The snapshot's text and version never change afterwards. Its `TextBuffer` methods return exactly what the buffer's methods returned when the snapshot was taken, errors included. Cloning a snapshot is O(1) too.
- **Threads.** `Snapshot` is `Send + Sync + 'static`. Several threads can read one snapshot at once while the UI thread edits the buffer. A snapshot stays readable after its set is dropped, and the last thread to drop it frees its nodes.
- **Staleness.** `is_stale` is `true` exactly when the buffer's version has moved past the snapshot's. It takes no lock and never blocks. Once it returns `true`, it keeps returning `true`. It must see every change that happens before the call in Rust's memory model, for example a change the UI thread made before sending this thread a message it has received. After the set is dropped, it keeps answering as of the set's last change.
- **No panics.** Every `TextBuffer` method on a snapshot follows T-101's rules: any offset, including `usize::MAX`, returns a value.

### Undo gets a new version

Undo and redo give the buffer a new version number, even though the text they leave is byte for byte the text of an earlier version. I considered going back to the old number, so results computed for that text would be valid again, and rejected it:

- **A history isn't a line.** Start at 0, apply to get 1, undo back to "0", then apply something different. That gives a second version 1 with different text. Any result still in flight for the first version 1 would be accepted against the wrong text. Avoiding that means version ids that name nodes in a tree, and then they can no longer be ordered.
- **`is_stale` would stop being final.** A worker that saw `true` could later see `false`, and an early-abort check only helps if its answer holds.
- **LSP forbids it.** `VersionedTextDocumentIdentifier.version` "will increase after each change, including undo/redo". With growing numbers, `helm-lsp` can send the buffer version as is.

The cost is that a result computed for text an undo has brought back gets dropped and recomputed. Reusing such results by comparing content is a possible optimization for a later task. It has no place in the version number.

### How a background reader knows its snapshot is stale

There are two checks, and the second one decides.

1. **On the worker, optional.** Before and during long work, the worker calls `snapshot.is_stale()` and gives up if it returns `true`. This is only to save work. A `false` can be out of date as soon as it returns.
2. **On the UI thread, always.** The worker sends its result tagged with `snapshot.buffer()` and `snapshot.version()`. The UI thread owns the set, so when the result arrives it compares the tag with `set.version(id)` and drops the result if they differ. Nothing can change the buffer while that comparison runs, so it's exact. This is the "workers tag results with the version they read, and the UI drops stale ones" rule from the plan's architecture section.

One way to implement `is_stale` is a counter shared by the set entry and its snapshots, such as an `Arc<AtomicU64>` that the set stores each new version into. `Relaxed` loads and stores are enough. The counter publishes no data, because the text moves inside the snapshot. The "sees every change that happens before the call" rule follows from coherence on one atomic, given the happens-before edge the caller's own channel or join already provides. If you use stronger orderings or a different design, explain why in the PR.

### Threading review

Khakha reads this code himself (plan, "Threading bugs pass tests"). In the PR description, say which types are shared between threads, which orderings you used and why, and confirm there's no `unsafe` and no lock. If you add `unsafe`, each block needs a `// SAFETY:` comment, but nothing here should need it, since ropey's `Rope` is already `Send + Sync`.

### Dependencies

None new. The stress test uses `std::thread::scope`, `std::sync::mpsc` and a small splitmix64 generator written in the test file. `proptest` is already a dev-dependency.

## Acceptance tests

Three files, committed before the implementation:

| File | What it proves |
| --- | --- |
| `crates/helm-text/tests/snapshots.rs` | 22 hand-picked cases. New buffers start at 0, including one added after others changed. Unknown buffers have no version or snapshot. `apply` adds one per touched buffer, not one per edit, and leaves untouched buffers alone. Undo and redo add one each and never return to an earlier number. Versions only grow across apply, undo, a new transaction and redo. A new transaction after an undo gets a fresh version, and its snapshot's text differs from the undone one's. A multi-buffer undo started from either buffer bumps both. A no-op transaction counts. No failure changes a version: `Empty`, `UnknownBuffer`, `InvalidRange`, `Overlap`, `Ok(None)`, unknown-buffer undo and redo, a blocked undo, a blocked redo, `get`, `version` or `snapshot`. A snapshot carries its buffer id and version and keeps its text through apply, undo and redo. Every `TextBuffer` method on a snapshot, at every offset, line and column, valid or not, including `usize::MAX`, matches a fresh `Buffer` of the same text, for the empty text too. `Display`, clones, and two snapshots at one version. `is_stale` turns true only when its own buffer changes, stays true after an undo restores the text, and stays false through failed calls. Snapshots outlive the set, are read by four threads at once, move to another thread, and 1,000 snapshots of a 1 MiB buffer keep their text through edits. A compile-time check that `Snapshot` is `Send + Sync + Clone + Debug + Display + TextBuffer + 'static`. |
| `crates/helm-text/tests/snapshot_stress.rs` | The stress test. The main thread runs 1,500 seeded random steps (applies across one to three of three buffers, undos and redos) on 8 to 16 KiB buffers with mixed-width characters and every line break. After each step it checks every version against a model, checks that its last snapshot of each changed buffer is stale and each unchanged one isn't, and snapshots every changed buffer plus one random one. Every snapshot goes, through `Arc`, to all four reader threads, so several threads read the same one at once. Each reader holds 48 snapshots. It checks each one's full text, lengths and a line when it arrives and when it's dropped, and its id, version, length and a sampled slice after every message. A reader that receives version `v` of a buffer asserts every held snapshot of that buffer older than `v` is stale. After the last step, the main thread sends the final versions, and every reader checks that staleness is exact. Three seeds, plus a test where a reader keeps 200 snapshots, the set is dropped, and the reader reads them and frees them on its own thread. |
| `crates/helm-text/tests/snapshot_properties.rs` | proptest over three buffers with random texts (short ones, or repeated up to a few KiB so ropes have several nodes) and up to 40 random steps of transactions (0 to 3 edits, some overlapping or empty), undos and redos. After every step, every version matches a model built from the rules above, and every snapshot taken so far keeps its id, version and text and is stale exactly when its version is behind. That still holds after the set is dropped. A second property checks every `TextBuffer` method of every snapshot against a `Buffer` clone taken at the same moment, at sampled lines and offsets plus both ends. 256 cases for the first property and 128 for the second, which is slower because every call goes through ropey in a debug build. |

Each test carries `#[ignore = "T-103: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-text -- --ignored` fails all 28 T-103 tests (22 + 4 + 2).

**The rule for the implementer: the only allowed change to these three files is deleting the `#[ignore = "T-103: not implemented yet"]` lines.** Nothing else in them changes. No edited assertion, no new `#[ignore]`, no changed step count, seed, thread count or case count. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

The stress test checks what can be checked, but a threading bug can pass it. Passing it doesn't replace the review above.

## Performance budget

NF-9 asks that the UI thread never wait, so taking a snapshot must cost the same on a 10 MiB buffer as on an empty one, and holding one must not make edits much slower. NF-7 covers the edit. Add a new file, `crates/helm-bench/benches/snapshots.rs`, with a `[[bench]] name = "snapshots"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds two criterion benchmarks in a group named `snapshots`:

| Benchmark id | What one iteration does |
| --- | --- |
| `snapshots/snapshot_1024x_10mb` | Takes 1,024 snapshots of a 10 MiB buffer in a `BufferSet`, passing each through `black_box` and dropping it right away. One snapshot is a few nanoseconds, too short for a 5% gate on a shared runner, so 1,024 of them make up one iteration. |
| `snapshots/apply_undo_holding_snapshot_10mb` | Takes a snapshot of the 10 MiB buffer, applies a one-edit transaction that inserts `"helm 😀"` at the char boundary at or below the middle, undoes it, then drops the snapshot. This measures the copy-on-write cost an edit pays while a worker holds a snapshot. |

Use the same 10 MiB text as `benches/text.rs`: copy its `LINES`, `floor_boundary` and `build_text` into the new file unchanged, so every file measures the same text. Build the text, the set and the transaction once, outside the timed code. For the second benchmark, clone the transaction in `iter_batched` setup (`BatchSize::SmallInput`), since `apply` takes it by value, and take the snapshot inside the timed routine. Each undo returns the buffer to the starting text, and the next `apply` drops the redo entry, so every iteration starts from the same state.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `snapshots` benchmarks, so `bench-compare` reports them as head-only in this task's PR and gates them from the next PR on. Adding a version counter to `apply` also runs in `transactions/apply_undo_100_edits_10mb`, which this task must not slow by more than 5%.

## Out of scope

- Reusing results across an undo when the text matches an earlier version (content hashes or similar): a later task, if profiling asks for it.
- Versions for a standalone `Buffer` outside a set.
- Limiting how much memory old snapshots keep alive: a later task, once Phase 3 shows how long parses hold them.
- The worker pool, channels and result routing: `helm-syntax`, `helm-lsp` and `helm-project` in later phases. This task only gives them the snapshot and the version.
- Converting `Version` to LSP's 32-bit `integer` version: `helm-lsp` in Phase 5.
- The debug-build assertion on blocking calls from the UI thread (NF-9): `helm-ui` in Phase 2.
