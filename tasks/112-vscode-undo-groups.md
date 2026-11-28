# T-112: Group VS Code typing into undo steps

After: T-109. This task waits until that one is merged. It is, as PR #42, and so are T-108 and T-110, which this task builds on, and T-114 (PR #45), which also changed `helm-editor`. This spec is written against main at `3566348`.

**Questions 1 to 4 under "Open questions for Khakha" block the implementation.** The tests pin an answer to each that VS Code's documentation doesn't settle. Questions 5 to 7 have a default the implementation follows unless Khakha says otherwise.

## Goal

In VS Code mode, a run of typing undoes in one step, not one character at a time. `helm-editor` decides where each step starts and ends: at a pause, a cursor move or selection change, a switch between typing and deleting, a newline, a deleted line break, a paste, and an undo or redo. Vim mode's groups (T-108) don't change. With this, `cmd-z` after typing a word takes back the word, and a keymap switch never folds one keymap's edits into the other's step.

PRD: ED-2 (undo and redo by transaction), ED-4 (VS Code's keymap works as a VS Code user expects).

## Crates and public interface

Crates: `helm-editor` (the implementation, in `src/**`, and new files in `tests/`) and `helm-bench` (a new benchmark file and its `[[bench]]` entry only). Nothing in `helm-text`.

The interface is in `crates/helm-editor/src/` with doc comments. Each new function has a `todo!()` body. Fill them in. Don't change a signature, a type or a doc comment's contract. If one looks wrong, say so in the PR description. `UndoGrouping` and `Editor::TYPING_PAUSE` are plain data and already work. Delete each `#[expect(..., reason = "T-112 interface stub; ...")]` once its stub is filled, or clippy with `-D warnings` fails. The rules are in the "Typing groups (T-112)" section of `Editor`'s type docs. This file restates them below with where each comes from.

Existing items change behaviour as their updated doc comments say, and the implementation must make them match: `Dispatcher::dispatch` and `dispatch_in` set the grouping before running a command, `vscode::keymap()` carries `UndoGrouping::Typing`, `Clone for Editor` copies the new state, and `end_undo_group` and `in_undo_group` cover typing groups. The editing, selection, undo and redo methods follow the "Typing groups" rules under `Typing` grouping and behave exactly as on main under `Explicit`. Every T-101 to T-114 test must still pass.

- **`UndoGrouping`**, a new public enum exported from the crate root, with two values. `Explicit` is today's behaviour: a group opens and closes only through `begin_undo_group` and `end_undo_group`, and every edit outside one is its own undo step. `Typing` is VS Code's policy below: the editor opens and closes groups itself around runs of typing and deleting. `Explicit` is the default.
- **`Editor::set_undo_grouping` and `Editor::undo_grouping`.** Setting a different value ends any open group, explicit or typing. Setting the value the editor already has changes nothing. `Editor::new` starts at `Explicit`, so every T-105 to T-111 test that builds an editor sees no change.
- **`Editor::set_time`**, which takes a `std::time::Duration`: the time of the edits that follow, measured from any fixed origin. This is how `helm-editor` learns about time without a clock of its own. `helm-ui` calls it before dispatching each key, with the key event's timestamp if the platform gives one, or else the time it received the event from a monotonic clock. The editor stores the value and compares it with the time of the group's last edit. It never reads a clock. Until the first call every edit happens at time zero, so no pause ever ends a step. A time earlier than the last one counts as no pause.
- **`Editor::TYPING_PAUSE`**, a public associated `Duration` constant: how long a pause between two edits has to be to start a new step. One second (open question 1).
- **`Keymap::with_undo_grouping` and `Keymap::undo_grouping`.** A keymap carries the grouping its editors use. `Keymap::new` gives `Explicit`. `vscode::keymap()` returns a keymap with `Typing`. `vim::keymap()` keeps `Explicit`.
- **`Dispatcher::dispatch` and `Dispatcher::dispatch_in`** change behaviour, and their signatures stay. Before running a `Resolution::Run`, each sets the editor's grouping to the active keymap's with `set_undo_grouping`. A resolution that runs nothing leaves the grouping alone. `CommandRegistry::run` and `run_in` don't touch it. The grouping is set before `run_in` starts, because `run_in` ends Vim's `.` record when it finds the editor's group closed (T-109). So a switch to VS Code in the middle of a Vim insert ends that group and its record at the first VS Code key.
- **`Clone for Editor`**, which T-109 wrote by hand to give each copy its own identity, copies the new state too: the grouping, the time, the open group's kind, the time of its last edit and the move flag. A copy keeps an open typing group, as T-109's copy keeps an open explicit group.
- **`Editor::in_undo_group`** returns true while either kind of group is open. `end_undo_group` ends either kind and returns whether one was open. Their doc comments say so; their signatures stay.

No command id, binding, mode or `KeyEvent` field changes. `vscode::keymap()` keeps its 39 bindings and `cursor_commands::ALL` its 26 ids, as T-107's tests pin.

## The rules

VS Code's public documentation doesn't describe when typing splits into undo steps. I read the "Basic editing" page, the "Key bindings" page and the default keybindings reference on code.visualstudio.com. They list `undo` (`cmd-z`), `redo` (`shift-cmd-z`), `cursorUndo` (`cmd-u`) and `editor.action.clipboardPasteAction` (`cmd-v`), and say nothing about grouping. So every rule below is a Helm proposal. Where VS Code's source supports a rule, the rule says so, and it is still a proposal: the source is an implementation detail, not documented behaviour. I read the MIT-licensed `microsoft/vscode` files `cursorCommon.ts`, `cursor/cursor.ts`, `cursor/cursorTypeOperations.ts`, `cursor/cursorDeleteOperations.ts` and part of `cursor/cursorTypeEditOperations.ts` to understand the behaviour. I copied no code from them, and the implementer must not either.

### Edit kinds

Under `Typing` grouping, the editor gives each editing command a kind:

- **Typing:** `insert`, whatever its text, and `newline`.
- **Deleting backward:** `delete_backward`, and `delete_to` with `Left`, `Up`, `WordLeft`, `LineStart` or `TextStart`. VS Code's `cmd-backspace` and `alt-backspace` reach these.
- **Deleting forward:** `delete_forward`, and `delete_to` with `Right`, `Down`, `WordRight`, `LineEnd` or `TextEnd`.
- **Other:** every other edit this editor makes. That covers Vim's commands and T-109's puts if they run under `Typing` grouping, and any later command that edits through a private path.

### When an edit joins the open step

Under `Typing` grouping, with no explicit group open, a typing or deleting edit joins the open typing group only if all of these hold. Otherwise the editor ends that group, opens a new one and applies the edit as its first transaction.

1. **A typing group is open.** `end_undo_group`, a successful `undo` or `redo`, `begin_undo_group`, a grouping change and an other edit each end it.
2. **There was no move since the group's last edit.** A move is a call to `set_selections`, `move_cursors`, `extend_selections`, `select_all`, `collapse_selections`, `add_cursor_above`, `add_cursor_below`, `add_next_match` or `select_all_matches` that changes the selections. A move that changes nothing, such as Left at offset 0 or `set_selections` with the same selections, doesn't count. A move and a move back still count, so Left then Right ends the step. Any other change to the selections, such as a Vim motion run under `Typing` grouping, counts when the selections differ from those just after the group's last edit. Clamping at the start of an edit isn't a move. Helm proposal. VS Code's cursor controller, in the files I read, doesn't reset its last edit type on a cursor move, so VS Code may join `ab`, Left, `c` into one step. I didn't check whether its text model adds a stop there.
3. **The kind is the same as the group's last edit's kind.** Typing after deleting, deleting after typing, and deleting forward after deleting backward each start a step. Helm proposal, which matches the source: VS Code puts a stop between edits of different types.
4. **It isn't a newline.** `newline` always starts a step, and typing after it joins that step. So `abc`, Enter, `def` undoes to `abc` first, then to the empty text. Two Enters make two steps. Helm proposal, which matches the source: VS Code's Enter pushes a stop before itself and none after.
5. **It deletes no line break.** A delete whose range, at any cursor, starts and ends on different lines as `TextBuffer::byte_to_line` counts them (so it covers a LF, a CR or a CRLF) starts a step, and later deletes of the same kind join it. So Backspace from the start of a line joins that line to the one above in a new step, and the Backspaces after it join that step. Helm proposal, which matches the source: VS Code's `deleteLeft` and `deleteRight` push a stop when a range spans lines.
6. **No pause.** The time from the group's last edit to this one is less than `TYPING_PAUSE`. A time of exactly `TYPING_PAUSE` starts a step. The pause is measured from the last edit, not the group's first, so a long run of quick keys stays one step. Helm proposal. The VS Code files I read have no timer for undo. I didn't read its text model, so I can't rule one out there.

When an edit joins, it amends the group's transaction, with T-108's mechanics. If `amend` refuses because another transaction is now on top (an agent's edit, say), the edit applies a new transaction and the group goes on from it. That new transaction undoes as its own step, so an outside edit splits the run. VS Code's source resets its last edit type on an outside change, which also starts a new step.

### Other rules

- **An other edit is its own step.** It ends any open typing group, applies its own transaction, and leaves no group open after it.
- **Paste is its own step.** No VS Code paste command exists yet: clipboard commands belong to an ED-1 task. That task must make each paste one step. Calling `end_undo_group`, then `insert` with the pasted text, then `end_undo_group` does this with the methods above, and the tests show it works. A paste method of its own, of kind other, would work too. Helm proposal, which matches the source: VS Code's paste pushes a stop before and after itself.
- **An edit with nothing to do changes nothing.** Backspace at offset 0, or Delete at the end of the text, applies nothing, returns `Ok(None)`, and leaves the group, its last kind and its last time as they were. So `ab`, Delete at the end, `c` is one step. Helm proposal (open question 6).
- **Errors change nothing.** An `UnknownBuffer` error leaves the group, its kind and its time as they were, as T-108's rule for groups already says.
- **Several cursors are one step.** Every command already applies one transaction for all its cursors, so the rules above decide steps for the whole set. Cursors that merge during a run don't end the step, since the edit set those selections, not a move. This matches the source: VS Code pushes at most one stop per command for all cursors.
- **Undo and redo** put back the selections from just before the step's first edit, and from just after its last, as T-108 does for every group.

### Explicit groups, Vim mode and keymap switches

- **An explicit group overrides the typing rules.** `begin_undo_group` under either grouping ends an open typing group and opens an explicit one. While it is open, every edit folds into it whatever its kind, the pauses and the moves, as T-108 says. After `end_undo_group`, the next edit starts a new typing group.
- **Vim mode doesn't change.** `vim::keymap()` has `Explicit` grouping, so the dispatcher keeps Vim's editors at `Explicit`, and T-108's insert sessions stay one group each, whatever the pauses. Under `Explicit` grouping, `set_time` has no effect.
- **A keymap switch.** The dispatcher sets the editor's grouping from the new keymap before the first command it runs. A switch between VS Code and Vim changes the grouping, which ends any open group. So VS Code typing never folds into a Vim insert session, and a Vim insert session never folds into VS Code typing, even if `helm-ui` forgets T-108's rule. That rule stays: `helm-ui` still calls `end_undo_group` on every open editor right after `Dispatcher::set_keymap`. It is what ends the group when a switch keeps the grouping, such as reloading the same keymap.
- **ED-6.** The grouping follows the keymap, so the toggle applies to open editors from their next key with no restart.

### What T-109 and T-114 left, checked on main at `3566348`

The first draft of this file was written while T-109 was in progress, against main at `01f24e2`, and listed six assumptions about its code. I read the merged code at `7638344`, and again at `3566348` after T-114. T-114 changed bracket objects, the quote rule and comments on `EditorId` and puts. It didn't touch undo groups, `run_in`, the dispatcher or the change marks. All six hold:

1. T-109 doesn't change what `begin_undo_group`, `end_undo_group`, `in_undo_group`, `undo` and `redo` do to a group. Its counted inserts run their repeats through `CommandRegistry::run_unrecorded` inside the open explicit group.
2. `run_in` ends a `.` record when it finds the editor's group closed: `Session::end_closed_record` before the command, and `finish_record` after it if no group is open. The 16 ids in `vim_commands::CHANGES` are all `vim.*` ids, so no command `vscode::keymap()` binds starts a record, and a typing group never holds one open.
3. Vim's edits, puts included (`apply_puts`), reach the buffer through `Editor`'s private `apply`. They call `begin_undo_group` where an insert session starts. So under `Typing` grouping they are other edits.
4. `dispatch_in` is filled in: `resolve`, then `run_in` for a `Run`. The grouping step goes before `run_in`, as the interface says.
5. `Keymap` still derives only `Clone` and `Debug`. T-109 added a private `lines` field the same way this task adds one.
6. `vscode::keymap()` is still used only by `tests/vscode_keymap.rs` and `tests/vim_keymap.rs`. T-109's edits to `vim_keymap.rs`, under Khakha's grant, touch only Vim's lists and keymap, not the switching tests.

Two T-109 changes this file didn't foresee, now covered in "Crates and public interface": `Editor`'s `Clone` is written by hand, so the new fields must be copied there, and the order of the grouping step and `run_in`'s record check matters. `CommandArgs` gained `register`, and the `KeyEvent` struct literals that rule out a timestamp field are still in the same four test files and `benches/vim.rs`.

### What the existing tests allow

Checked by reading at `3566348`, and by running every `helm-editor` test with the throwaway implementation described under "Acceptance tests". None of the four files below changed since `01f24e2`, apart from Khakha's granted edit to `vim_keymap.rs`.

- `tests/undo_groups.rs`'s `edits_outside_a_group_are_their_own_steps` pins one step per ungrouped edit on a plain `Editor`. That is why `Explicit` is the default and the typing rules need `set_undo_grouping`.
- `tests/dispatch.rs`'s `typing_at_every_cursor` and `bound_keys_edit_and_undo` undo one key at a time through a keymap the test builds with `Keymap::new`. That keymap has `Explicit` grouping, so both pass unchanged.
- `tests/vscode_keymap.rs` uses `vscode::keymap()`. `select_all_then_delete_and_undo` makes one edit before `cmd-z`. In `word_and_line_deletes`, `alt-backspace` and `cmd-backspace` join into one deleting-backward step, so the first `cmd-z` restores the text and the second finds nothing to undo. The text it asserts is the same either way.
- `tests/vim_keymap.rs`'s two switching tests type one VS Code char at a time between Vim edits. Both still give their asserted texts and the three undo steps.

## Choices I made, and the ones I rejected

- **Time comes through `Editor::set_time`.** I rejected a timestamp field on `KeyEvent`: four protected test files (`counts.rs`, `session.rs`, `vim_keymap.rs`, `vim_t109_keymap.rs`) and `benches/vim.rs` build `KeyEvent` with struct literals, so a new field needs a grant for each. I rejected a timer in `helm-ui` that calls `end_undo_group` after a pause: it wakes the UI thread for every pause, it would end Vim's insert sessions unless `helm-ui` tracked the group's kind, and the rule could only be tested with a window. I rejected an `Instant`, since tests can't build a chosen one, and a clock inside `helm-editor`, which would make the property tests depend on how fast they run.
- **The grouping is a setting on the editor that the keymap supplies.** I rejected always-on typing rules: they break `edits_outside_a_group_are_their_own_steps` and the two `tests/dispatch.rs` tests above. I rejected having `helm-ui` call `set_undo_grouping` on a switch: it is one more rule for `helm-ui` to remember, and the dispatcher already holds both the keymap and the editor for each key (open question 4).
- **The policy lives in `Editor`, not in the dispatcher.** A mouse click reaches the editor through `set_selections` without passing the dispatcher, and it has to end the step. Only the editor sees every move and every edit.
- **A move is a selection method that changes the selections.** I rejected only comparing the selections with those after the group's last edit: Up then Down back to the same offset would keep the step, which reads as a bug. A flag set by each of the nine methods costs nothing per move, since each already knows whether it changed anything. Vim's motions set the selections in many crate-private places, so for them the rule falls back to the comparison. They don't run under `Typing` grouping through a keymap, so a Vim round trip isn't tested.
- **No word-by-word steps.** The part of VS Code's source I couldn't read in full seems to put a stop before the first space typed after a word, so `hello world` would undo `" world"` first. I couldn't confirm it, and the brief didn't ask for it (open question 5).

## Acceptance tests

Three new files, committed before the implementation. 39 tests. Every test carries `#[ignore = "T-112: not implemented yet"]`. No existing test file changes.

| File | What it proves |
| --- | --- |
| `crates/helm-editor/tests/typing_groups.rs` (25) | On `Editor` alone with `Typing` grouping. `Editor::new` is `Explicit`, and ungrouped edits stay one step each. A typed word undoes and redoes in one step, with the selections from before its first char and after its last, and every char returns the step's id. Three cursors in a CRLF text type as one step, and cursors that merge during a run of Backspaces keep it. Each of eleven moves ends the step: Left, TextEnd, Down, extending, `set_selections`, both add-cursor commands, both add-match commands, `select_all` and `collapse_selections`. A move that changes nothing keeps it, and a move and a move back, by motion or by `set_selections`, don't. Typing, Backspace, typing and Delete make four steps, and Backspace then Delete two. Backspace, `delete_to(WordLeft)` and `delete_to(LineStart)` join, so do Delete, `delete_to(WordRight)` and `delete_to(LineEnd)`, and a word delete in the other direction doesn't. Enter starts a step that typing after it joins, in LF and CRLF, and two Enters make two steps. Backspace and Delete over a LF, a CRLF and a lone CR start a step that later deletes join, and so does `delete_to(WordLeft)` across a line break. A pause of exactly `TYPING_PAUSE` starts a step, one nanosecond less doesn't, the pause counts from the last edit, a long run of quick keys stays one step, and a pause splits deletes too. With no `set_time`, and with time going backwards, nothing splits. Under `Explicit` grouping, `set_time` changes nothing, inside a group or out. Undo and redo end the step. `end_undo_group`, `insert` with two lines and `end_undo_group` make a paste its own step between two typing steps. An explicit group ends a typing group, and while it is open pauses, moves and kinds fold in. Changing the grouping ends either kind of group, and setting the same one doesn't. An edit with nothing to do keeps the step, and one before any typing opens nothing. `UnknownBuffer` from a delete or a newline changes nothing. An outside transaction splits the run. A Vim command run through the registry under `Typing` grouping is its own step and leaves no group open. A clone keeps the open step and its last edit's time. |
| `crates/helm-editor/tests/vscode_undo.rs` (10) | Through `Dispatcher::dispatch_in` with one `Session`, as `helm-ui` will run it. `vscode::keymap()` has `Typing` grouping and keeps its 39 bindings, and `vim::keymap()`, `Keymap::new` and `with_undo_grouping` give what they say. `dispatch` sets the editor's grouping on a run, and an unbound key and a pending chord leave it alone, and `dispatch_in` with Vim's keymap sets `Explicit`. A session of `hello world`, Enter, `x`, Left, `y` and Backspace undoes step by step with `cmd-z`, with the documented texts and cursors, and `shift-cmd-z` redoes it. Backspace, `alt-backspace` and `cmd-backspace` undo in one `cmd-z`. Typing after `cmd-z` starts a new step. `cmd-d`, `shift-cmd-l` and typing `total` at three cursors undoes in one `cmd-z` and gives back the three selections. `set_time` between keys splits a run at a pause. Switching to Vim without `end_undo_group` keeps VS Code's typing and Vim's `x` apart. Switching to VS Code in the middle of a Vim insert, also without it, keeps the two apart and finishes the insert's `.` record before VS Code's first key, so the record holds `i`, `x` and `y` and not `z`. T-108's three-step switching session, with two VS Code chars in place of one, still gives three steps. |
| `crates/helm-editor/tests/typing_group_properties.rs` (4) | proptest, 256 cases per property, over random texts of words, blanks, punctuation, `_`, é, 中, 😀, LF and CRLF, at up to three cursors, with up to 40 ops: typed chars, Enter, Backspace, Delete, word and line deletes, ten motions, waits of 0, 1 ms, 500 ms, just under `TYPING_PAUSE`, `TYPING_PAUSE` and three times it, times that go backwards, and `end_undo_group`. (1) With lone CRs and `begin_undo_group` added: every op leaves the same return value, text and selections under `Typing` as under `Explicit` grouping, so grouping changes only undo. (2) An oracle written from the rules marks the text before each edit that starts a step. Undoing returns each mark, newest first, then finds nothing, and redoing walks forward to the final text, then finds nothing. (3) Under `Explicit` grouping, with lone CRs, each edit that applies a transaction is one step whatever the time. (4) As (2), with `begin_undo_group` and `end_undo_group` mixed in, which the oracle treats as explicit groups. |

The oracle runs the same ops on a second editor under `Explicit` grouping to learn three facts: whether each edit applied a transaction, whether each motion changed the selections, and whether a delete lowered the line count, which is when it took a line break. Properties (2) and (4) leave out lone CRs, since deleting the char between a lone CR and a LF would join them into a CRLF and lower the line count without deleting a break. `typing_groups.rs` covers lone CRs by hand.

Against the stubs, `cargo test -p helm-editor --no-fail-fast -- --ignored` fails all 39. No test only reads plain data.

I checked the tests against a throwaway implementation before committing them. All 39 pass on it, and all 443 tests in `helm-editor` pass with it, so no T-101 to T-114 test changes behaviour. The four properties also pass at 20,000 cases each. That implementation isn't in this PR. I also ran eleven wrong versions of it against the three files. Each of these failed at least one test, and most failed several: no move rule, moves found by comparison only (so a move and a move back keep the step), no pause rule, the pause counted from the step's first edit, Enter joining the typing before it, no line-break rule, kinds ignored, a grouping change that keeps an open group, `dispatch_in` not setting the grouping, and an other edit joining the typing step. The eleventh, an other edit that leaves a group open after itself, passed every test. It changes nothing, since an other edit already ends the typing group before it applies.

**The rule for the implementer: the only allowed change to these three files is deleting the `#[ignore = "T-112: not implemented yet"]` lines.** Nothing else in them changes. That means no edited assertion, no new `#[ignore]` and no changed case count. Khakha checks the diff of these files before adding the `approved-protected` label. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-1 gives a keypress 5 ms from input to pixel. Deciding whether an edit joins the step is on that path, so it must stay small next to the edit itself.

Cost rules:

- **Under `Explicit` grouping** an edit costs what it costs on main, plus one check of the grouping.
- **Under `Typing` grouping** deciding costs a fixed amount per edit: comparing a kind, a time and the move flag. The line-break rule adds two `byte_to_line` lookups per deleted range, or nothing where the delete code already knows whether a range crosses a line. It never reads the deleted text. Comparing the selections with those after the step's last edit, for changes other than the nine moves, costs one pass over the selections. It may use the step's undo record instead of a copy. Joining costs what T-110's `amend` costs.
- **A move** sets a flag. `set_selections` compares the old and new selections, which costs no more than building the new ones.
- **No text is copied** and nothing is read from the buffer to decide a step, apart from the line-break check above.
- **No clock, timer or thread.** The editor never calls `Instant::now` or `SystemTime::now`.
- **The dispatcher** adds one comparison of the grouping per run.

Add a new file, `crates/helm-bench/benches/typing.rs`, with a `[[bench]] name = "typing"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds three criterion benchmarks in a group named `typing`, on the same 10 MiB text `benches/editor.rs` builds. Copy `LINES`, `floor_boundary` and `build_text` from `benches/cursor.rs` unchanged.

| Benchmark id | What one iteration does |
| --- | --- |
| `typing/type_100_chars_undo_10mb` | With `Typing` grouping and one cursor at the start of the middle `fn main() {` line: `set_time` 10 ms later, then `insert` of one char, 100 times, cycling through the chars of `"helm 😀中é"`, then one `undo`. |
| `typing/type_10_chars_undo_1000_cursors_10mb` | The same with 10 chars at 1,000 cursors placed as `benches/cursor.rs` places them. |
| `typing/vscode_session_undo_10mb` | `Dispatcher::dispatch` with `vscode::keymap()` of `hello`, two Backspaces, `lo world`, Enter, `x`, Left and `y` at one cursor, with `set_time` 10 ms later before each key, then `cmd-z` five times, once per step. |

Each iteration ends with the text and selections it started with, and the next edit drops the redo entries. Build the text, the set, the editor, the dispatcher and the events once, outside the timed code, and pass each result through `black_box`.

Budget: the default 5% from `budgets.toml`, with no per-benchmark override and no target. The base commit has no `typing` benchmarks, so `bench-compare` reports them as head-only in this task's PR and gates them from the next PR on. This task also changes `Editor`'s edit path and `Dispatcher::dispatch`, so the `editor/`, `cursor/`, `amend/`, `vim/`, `vim_objects/` and `keymap/` benchmarks must not get more than 5% slower.

As in T-101 to T-109, the implementation PR adds the benchmark file. The spec PR doesn't, since the benchmarks would call `todo!()` stubs.

## Out of scope

- Word-by-word steps for typed spaces: open question 5.
- `cursorUndo` (`cmd-u`) and undoing selection changes on their own, as T-105 left them: a later task.
- The paste, cut and copy commands and the clipboard (ED-1): a later task. It must follow the paste rule above.
- Auto-closing brackets, electric characters and auto-indent, which VS Code groups by their own rules: later, with SY-3.
- `helm-ui` calling `set_time` before each key, and any extra boundary `helm-ui` wants, such as on save or focus loss, through `end_undo_group`: `helm-ui`.
- A setting for `TYPING_PAUSE` or for turning the pause off (ST-1): later.
- Any change to Vim's grouping.
- Streaming an agent's edits into one transaction (AG-4): Phase 7.
- Removing the transaction clone in `Editor::apply` (T-111's finding 8): Stream K.

## Open questions for Khakha

Questions 1 to 4 block the implementation. Each rule is a Helm proposal that VS Code's documentation doesn't settle, and the tests pin it, so a different answer changes the tests before anyone implements them. Questions 5 to 7 don't block: the spec states a default for each, and the implementation follows it unless Khakha says otherwise.

1. **Keep the pause rule, and at what length? (blocking)** VS Code's docs describe no pause, and its cursor code has no timer for undo. I propose one second as a Helm rule, long enough that ordinary typing never splits. A different length changes only `TYPING_PAUSE`, since the tests name the constant. Dropping the rule removes `set_time`, the pause tests in both test files, and the waits in the properties.
2. **Should every move end the step? (blocking)** VS Code's cursor controller doesn't reset on a move in the files I read, so VS Code may join `ab`, Left, `c`. I propose that every move ends the step, since the brief lists it and it gives the same result for a mouse click as for an arrow key.
3. **Enter starts a step, and typing after it joins. (blocking)** This matches VS Code's source. The other choices are Enter as its own step, or Enter joining the typing before it.
4. **The keymap carries the grouping, and the dispatcher applies it. (blocking)** The other choice is `helm-ui` calling `set_undo_grouping` after each switch. That keeps `Keymap` and `Dispatcher` as they are, but adds a second rule for `helm-ui` beside T-108's, and `vscode_undo.rs` would change.
5. **Word-by-word steps. (default: a later task)** Include VS Code's apparent space rule, where `hello world` undoes `" world"` first, in this task, in a later one, or not at all? I couldn't confirm it from the docs or from the parts of the source I read in full.
6. **An edit with nothing to do keeps the step. (default: as specified)** VS Code may record the type of a delete that deleted nothing. I couldn't tell from the source. I propose keeping the step, which follows Helm's rule that a command with nothing to do changes nothing. A different answer changes one test.
7. **`TYPING_PAUSE` as a public constant. (default: a constant)** Tests need to name it. If ST-1 will make it a setting, it could be a field on `Typing` grouping instead. That means `UndoGrouping::Typing { pause }` and a different `Keymap` API.
