# T-108: Vim normal, insert and visual modes, with motions, operators and counts

After: T-106. This task waits until that one is merged. It is, and so are T-107 and T-110, which this task builds on.

## Goal

`helm-editor` gets Vim's keymap as data and the commands behind it: normal, insert, visual and visual-line modes, fifteen motions, the `d` and `c` operators, counts, the one-key edits (`x`, `D`, `s` and the rest), entering and leaving insert mode, and `u` and `ctrl-r` that undo a whole insert session in one step. Every command works at every cursor. With this, a Vim user can edit the way they expect, and ED-5's acceptance format, keystroke sequences checked against the buffer, grades it.

PRD: ED-5 (modes, motions, operators and counts; text objects, registers, `.` repeat and `:w` `:q` `:e` are T-109's), ED-6 (one registry holds both keymaps, so switching needs no restart).

## Crates and public interface

Crates: `helm-editor` (the implementation) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only). `helm-bench` already depends on `helm-editor`.

The interface is already in place, with doc comments: new `vim.rs` and `vim_commands.rs`, `Typing::Count` in `keymap.rs`, `KeyState::count` and `Dispatcher::resolve_counted` in `dispatch.rs`, `CommandContext::count`, `CommandRegistry::run_counted` and `CommandRegistry::with_vim_commands` in `command.rs`, and three undo-group methods at the end of the first `impl Editor` block in `editor.rs`. Each new function has a `todo!()` body. Fill them in. Don't change a signature, a type or a doc comment's contract. If one looks wrong, say so in the PR description. Private fields and helpers, including `vim_commands::register_all`, are placeholders, so replace or add them freely. Delete each `#[expect(unused_variables, ...)]` on a stub and the `#![expect(dead_code, ...)]` at the top of `vim_commands.rs` once they are filled, or clippy with `-D warnings` fails.

Three existing functions change behaviour, as their updated doc comments say, and the implementation must make them match: `Dispatcher::resolve` gains the count rule (rule 0) and drops counts in rules 2 and 3, `KeyState::cancel` also drops the count, and `Dispatcher::dispatch` runs commands through `run_counted`. Every editing command on `Editor` amends instead of applying while an undo group is open. Nothing else about an existing method changes, and every T-101 to T-107 and T-110 test must still pass.

Don't touch `crates/helm-text`. Don't change `editor_commands.rs` or `cursor_commands.rs`: T-106 and T-107 pin their lists. `with_all_commands` keeps its 36 commands, since T-107's tests pin that too, so the registry with everything is a new constructor.

```rust
// lib.rs
pub mod vim;
pub mod vim_commands;

// keymap.rs
pub enum Typing { Insert, Ignore, Count }

// dispatch.rs
impl KeyState {
    pub fn count(&self) -> Option<usize>;
}
impl Dispatcher {
    pub fn resolve_counted(&self, state: &mut KeyState, event: &KeyEvent, contexts: &[&str])
        -> (Resolution, Option<usize>);
}

// command.rs
pub struct CommandContext<'a> {
    pub editor: &'a mut Editor,
    pub set: &'a mut BufferSet,
    pub args: &'a CommandArgs,
    pub count: Option<usize>,                  // new
}
impl CommandRegistry {
    pub fn with_vim_commands() -> Self;        // with_all_commands and vim_commands::ALL: 91
    pub fn run_counted(&self, invocation: &Invocation, count: Option<usize>,
                       editor: &mut Editor, set: &mut BufferSet) -> Result<(), CommandError>;
}

// editor.rs
impl Editor {
    pub fn begin_undo_group(&mut self);
    pub fn end_undo_group(&mut self) -> bool;
    pub fn in_undo_group(&self) -> bool;
}

// vim_commands.rs
pub const LEFT, RIGHT, DOWN, UP, NEXT_WORD, PREV_WORD, WORD_END, NEXT_BIG_WORD,
          PREV_BIG_WORD, BIG_WORD_END, LINE_START, FIRST_NON_BLANK, LINE_END,
          FIRST_LINE, LAST_LINE, LINE: &str;                  // "vim.left", ..., the 16 motions
pub const SELECT_LEFT, ..., SELECT_LAST_LINE: &str;           // "vim.select_left", ..., 15
pub const DELETE, CHANGE: &str;                               // the operators
pub const DELETE_CHAR, DELETE_CHAR_BEFORE, DELETE_TO_LINE_END,
          CHANGE_TO_LINE_END, SUBSTITUTE, SUBSTITUTE_LINE: &str;  // x X D C s S
pub const INSERT, APPEND, INSERT_LINE_START, APPEND_LINE_END,
          OPEN_BELOW, OPEN_ABOVE, NORMAL: &str;               // i a I A o O, Escape
pub const VISUAL, EXIT_VISUAL, SWAP_ENDS, DELETE_SELECTION, CHANGE_SELECTION,
          DELETE_LINES, CHANGE_LINES: &str;                   // the visual commands
pub const UNDO, REDO: &str;
pub const ALL: [&str; 55];                                    // sorted

// vim.rs
pub const NAME: &str = "vim";
pub const NORMAL, INSERT, VISUAL, VISUAL_LINE, DELETE_PENDING, CHANGE_PENDING: &str;
pub fn keymap() -> Keymap;
```

### Rules the implementation must follow

The module docs of `vim.rs` hold the full rules for every command, and the doc comments on the new dispatcher and editor items hold theirs. In short:

- **Counts.** In a `Typing::Count` mode with no chord pending, `1` to `9` always add a digit, and `0` adds one only when a count is pending, before any binding is looked at. The count goes with the next `Run`. A count typed before an operator moves onto it, and the two multiply, saturating. Escape, an unbound key, a broken chord, a second operator, a binding with no action, `KeyState::cancel` and a keymap switch all drop it. `resolve` gives exactly what `resolve_counted` gives, minus the count.
- **Normal-mode positions.** A cursor is on a char: never at the end of a line that has text, never inside a CRLF. Vim's commands settle every cursor onto such a position before and after they run, except where they enter insert mode.
- **Visual selections** are stored as the bytes they cover. Vim's visual cursor is the char at the head end, and the anchor char is at the other. Visual-line mode keeps the same selection, and its `d` and `c` act on whole lines.
- **Motions** land where `vim.rs` says, with counts that repeat (or, for `gg` and `G`, name a line) and stop early when a step stops moving. `j` and `k` keep T-107's goal columns.
- **Operators.** `d` and `c` act on exclusive, inclusive or linewise ranges, with Vim's documented special cases for `dw`, `cw` and exclusive ranges that end at a line start. Linewise delete at the last line takes the line break before it. With several cursors, ranges are joined and applied as one transaction.
- **Insert mode.** `i`, `a`, `I`, `A`, `o`, `O`, `c` and its one-key forms open an undo group. Escape closes it and steps each cursor back one char. One `u` then undoes the whole session.
- **Undo groups.** While a group is open, the first edit applies a transaction and every later one amends it with `BufferSet::amend`. If amend refuses because another transaction landed on top, the edit applies a new transaction and the group goes on from it. `undo` and `redo` end a group.
- **The keymap.** Six modes, in the order `normal`, `insert`, `visual`, `visual_line`, `delete_pending`, `change_pending`, and exactly the 115 bindings in the table on `vim::keymap`, every one scoped to a mode with no context. Every command it binds is in `with_vim_commands`.
- **No whole-text copies.** Motions and operators read only the text near each cursor and the lines they cross. Don't call `to_string` on the buffer. The benchmarks below catch a motion that looks up the line index for every char.

### What T-106 and T-107 left to this task, and how it takes it

- **Operators as keymap state.** T-106's `Action::Operator` and the pending operator in `KeyState` stay as they are. T-108 says what `vim.delete` and `vim.change` do with `args.motion`: each motion id in `vim_commands` names a motion, and any other id does nothing.
- **Counts in `KeyState` and `CommandArgs`.** The count is in `KeyState`, as planned. It isn't in `CommandArgs`: adding a field there breaks two struct literals in `crates/helm-editor/tests/commands.rs`, a T-106 acceptance test this task may not edit. So the count travels next to the invocation instead: `Dispatcher::resolve_counted` returns it, `CommandRegistry::run_counted` takes it, and the command reads `CommandContext::count`. `resolve`, `run` and `Resolution` keep their T-106 signatures. The PR description asks Khakha whether to grant that edit and move the count into `CommandArgs` instead.
- **A command that asks for a mode switch.** Not needed. `d` and `c` each set their own pending mode, and the motion bindings in `change_pending` switch to insert mode with T-106's `then_mode`. The cost is 16 more bindings, one set of motion keys per operator. A mode request would have needed a new field the command writes back through, a new return path from the registry, and a rule for when the keymap lacks the mode.
- **One `Motion` enum (T-107).** T-107 planned Vim's motions as new `Motion` variants. Two protected T-107 test files, `tests/chunk_motions.rs` and `tests/chunk_crossings.rs`, match on `Motion` with no wildcard arm, so a new variant stops them compiling. So Vim's motions are command ids, with a crate-private enum behind them. The implementation should still build them on T-107's machinery: the chunk readers in `motion.rs`, `Reads`, the goal-column handling in `Editor`, and `delete_each`, which `delete_to` wraps. `delete_to` itself fits only Vim's exclusive charwise ranges, so the operators need `delete_each`'s range form rather than `delete_to`'s single motion. The PR description asks Khakha whether to grant a wildcard arm in those two files so the motions can become `Motion` variants.
- **`BufferSet::amend` (T-110).** Used now. T-105's "Grouping keystrokes" and T-110's out-of-scope list left wiring `amend` into `Editor` to a later `helm-editor` task after T-106, and this is one. Vim needs it: `u` undoing one typed character at a time would fail ED-5's keystroke suite. Vim's grouping policy has no timers (a group is exactly one insert session), so it fits this task. VS Code's policy (pauses, cursor moves, switching between typing and deleting) is still a later task, and can reuse the same three `Editor` methods.

### Choices I made, and the ones I rejected

- **The mode lives only in `KeyState`.** Commands don't know the mode, so visual mode has its own ids (`vim.select_*`), as T-107 has `cursor.left` and `cursor.select_left`. I rejected passing the mode in `CommandContext`, which ties commands to mode names, and treating any non-empty selection as visual mode, which breaks on a selection made by another keymap or by the mouse.
- **Visual selections hold the bytes they cover.** `helm-ui` draws the selection it gets, and `d` deletes exactly it. I rejected storing Vim's cursor and anchor chars, which would make every reader of the selection convert.
- **Visual-line mode keeps a charwise selection.** Its line commands expand it to lines when they run, and `helm-ui` draws whole lines in that mode. Storing whole lines would lose the cursor's column, which `j`, `k` and `w` need.
- **Digits `1` to `9` always count in a counting mode.** A keymap can't bind them there. Vim works the same way, and it keeps the count rule out of the binding table.
- **Lines are the text's own lines.** `"ab\n"` has an empty last line the cursor can reach, as in T-107. Vim reads the final line break as the end of the last line. Changing that would make Vim's and VS Code's editors disagree on the same buffer.
- **Word classes are T-107's.** `中文` is one word, where Vim gives CJK ideographs their own class.
- **`ctrl-r` puts the cursor at the start of the selections from after the change.** After redoing `ihello<Esc>` the cursor is just past `hello`, where Vim puts it on `h`. Mapping the "before" selections through the redone edits would match Vim with one cursor but needs the edits' offsets, which `BufferSet::redo` doesn't return.
- **Left out on purpose:** `$` in a visual mode doesn't take the line break, `$` sets no sticky end of line for later `j` and `k`, and `cc` keeps no indentation (no `autoindent` until there are indentation settings). Each is a small, separate change later.

I checked the rules against Vim's documentation as I remember it (`:help exclusive`, `:help exclusive-linewise`, `:help word`, `:help cw`, `:help dd`), not against a running Vim. The rows most likely to differ from Vim are in `db_and_de`, `dw_deletes_to_the_next_word_but_not_past_the_line_end` and `ctrl_r_redoes`.

### Dependencies

None. No new dependency, no change to any `Cargo.toml` except the `[[bench]]` entry, and `Cargo.lock` is unchanged.

## Acceptance tests

Six new files, committed before the implementation. 80 tests:

| File | What it proves |
| --- | --- |
| `crates/helm-editor/tests/counts.rs` (17) | Through a small keymap with every kind of mode: digits make a count that goes with the next `Run` and clears. `resolve` gives the same resolution and state as `resolve_counted` over 14 key histories. `0` is a binding alone and a digit after a count. Digits beat a binding on `1`. Insert and ignoring modes don't count, and a counting mode types nothing. Digits with modifiers are keys. A count waits through a chord, a digit inside a chord is a key, and a broken chord drops the count. Operator counts multiply (six cases), and a count before an operator moves onto it. Escape, an unbound key, a second operator and a binding with no action drop the count, and a mode-switching `Run` takes it. `cancel` drops it and says so. A keymap switch drops it. Forty nines saturate. `dispatch` and `run_counted` pass the count to the command, and `run` passes `None`. |
| `crates/helm-editor/tests/undo_groups.rs` (11) | On `Editor` alone: typing, newline and deletes in a group undo and redo in one step with the right selections. Every edit returns the group's transaction id, and a command with nothing to do still returns `None`. Edits outside a group are their own steps. An empty group changes nothing. Moving and setting selections keep a group open. `undo` and `redo` end it. Beginning again starts a new group. An outside transaction on top makes the next edit a new step that the group continues. Three cursors in a CRLF text undo as one. A group drops redo. An `UnknownBuffer` error in a group changes nothing and the group goes on. |
| `crates/helm-editor/tests/vim_normal.rs` (28) | Keystrokes through `vim::keymap()` and `with_vim_commands`, as tables of marked text before and after. `h` and `l` within a line, over é, 中, 😀 and CRLF. `j` and `k` columns, goal columns through short lines, first and last lines and counts. `w`, `b`, `e`, `W`, `B` and `E` over words, punctuation, `_`, blank lines, CRLF and counts. `0`, `^`, `$`, `gg` and `G` with counts. Settling a cursor left at a line end. `dw` and its line-end rule, `db` and `de` with the exclusive-linewise rule, `d` with line motions, `x` and `X`, `dd`, `dj`, `dk`, `dG` and `dgg`, counts that multiply, and forty-digit counts. The pending modes. `cw` as `ce`, `cc`, `cj`, `C`, `s` and `S`. `i`, `a`, `I`, `A`, `o` and `O`, insert-mode typing, Enter, Backspace and Delete, and Escape stepping back. `u` undoing whole inserts and changes, `3u`, and `ctrl-r` with counts. Commands at several cursors. |
| `crates/helm-editor/tests/vim_visual.rs` (11) | `v` and `V` selecting the char under each cursor, empty lines and CRLF included. Motions moving the visual cursor and flipping direction past the anchor, with goal columns and counts. `o` swapping ends. Escape and `v` leaving on the visual cursor. `d`, `x`, `c` and `s` in both visual modes, linewise at the first and last lines, and the mode each leaves in. Switching between `v` and `V`. Undo after a visual edit. Unbound keys doing nothing. |
| `crates/helm-editor/tests/vim_keymap.rs` (7) | `vim_commands::ALL` is the 55 documented ids. `with_vim_commands` holds all 91 commands with titles, and `with_all_commands` and `with_editor_commands` are unchanged. The six modes in order with their typing. Exactly the 115 documented bindings. Every bound command is registered. Each motion key resolves in normal, both visual and both pending modes to the right command and mode. Switching between VS Code's and Vim's keymaps mid-session edits as each one should (ED-6). |
| `crates/helm-editor/tests/vim_properties.rs` (6) | proptest, 256 cases per property, over random text of words, blanks, punctuation, each line break, each char width and any char. (1) Any sequence of motion keys with counts, at up to three cursors, keeps every cursor on a normal-mode position and never edits. (2) A count moves like pressing the motion that many times, for the ten repeating motions. (3) Any insert session, entered by any of `i a I A o O` and typing chars, Enter, Backspace and Delete, undoes in one `u`, and there is nothing older to undo. (4) Any up to three edits from `x`, `X`, `D`, `dd`, `d{motion}`, `c{motion}…<Esc>`, `S`, `C`, `s`, `cc` and visual `d`, `x`, `c` and `s`, at up to two cursors, leave every cursor on a normal-mode position, and as many `u` as edits restore the original text. (5) `{n}dd` matches a line oracle, LF or CRLF, including where the cursor goes. (6) A visual `d` deletes exactly the selection that `v` and its motions showed. |

Each test carries `#[ignore = "T-108: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-editor --no-fail-fast -- --ignored` fails 79 of the 80. The one that passes is `vim_command_ids_are_the_documented_ones`, because the id constants are plain data and already in place, as T-107's were.

I checked the tests against a throwaway implementation before committing them. All 80 pass on it, every existing test in the workspace still passes with it, and the six properties also pass at 20,000 cases each, three runs with fresh seeds. That implementation isn't in this PR. Running the properties at 20,000 cases found one real rule this spec now states: a cursor whose own range is empty but which sits inside another cursor's range ends where that range began.

**The rule for the implementer: the only allowed change to these six files is deleting the `#[ignore = "T-108: not implemented yet"]` lines.** Nothing else in them changes. That means no edited assertion, no new `#[ignore]`, and no change to a case count. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-1 gives a keypress 5 ms from input to pixel, and a Vim command at many cursors is on that path. Add a new file, `crates/helm-bench/benches/vim.rs`, with a `[[bench]] name = "vim"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds three criterion benchmarks in a group named `vim`, on the same 10 MiB text `benches/editor.rs` builds. Copy `LINES`, `floor_boundary` and `build_text` from `benches/cursor.rs` unchanged.

| Benchmark id | What one iteration does | Target |
| --- | --- | --- |
| `vim/w_b_1000_cursors_10mb` | `CommandRegistry::run` of `vim.next_word`, then of `vim.prev_word`, at 1,000 cursors | mean under 500 µs |
| `vim/d3w_undo_1000_cursors_10mb` | `run_counted` of `vim.delete` with motion `vim.next_word` and count 3 at 1,000 cursors, then `run` of `vim.undo` | mean under 2.5 ms |
| `vim/insert_100_chars_undo_10mb` | `Dispatcher::dispatch` of `i`, 100 chars cycling through `"helm 😀中é"`, Escape and `u`, at one cursor | mean under 250 µs |

Place the 1,000 cursors as `benches/cursor.rs` does, at the start of `fn main() {` lines spread evenly through the text. That line has no indentation, so `w` then `b` returns each cursor to where it started. `u` puts back the cursors from before `3dw`, and the insert session's `u` puts back its single cursor, at the start of the middle one of those lines. So every iteration starts from the same state. Build the text, the set, the registry, the dispatcher and the events once, outside the timed code, and pass each result through `black_box`.

Each target is a criterion mean on the `ubuntu-latest` runner of the CI `bench` job, read from the bench job's artifact. The PR reports those CI numbers, and labels any local number as local. For scale, from the `bench-record` artifact for main at `fce6dd7`: T-107's `cursor/word_right_1000_cursors_10mb` (WordRight then WordLeft at the same 1,000 cursors) is 216 µs, and T-110's `amend/type_100_chars_undo_10mb` is 87 µs. On my machine (local), T-107's `delete_to(Motion::WordRight)` plus `undo` at the same cursors takes 1.0 ms, so most of the second target is the transaction, and the throwaway implementation's insert session took 122 µs. The throwaway's motions took 4.6 ms for the first benchmark, because its text reader looks up the line index for every char. That is what the first target is there to catch, so don't build the motions that way.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `vim` benchmarks, so `bench-compare` reports them as head-only in this task's PR and gates them from the next PR on. The CI gate doesn't check the targets above. Khakha checks them against the artifact. This task also changes `Dispatcher::resolve` and `Editor`'s transaction path, so `keymap/resolve_10k_keys_300_bindings` and the `editor/` and `cursor/` benchmarks must not get more than 5% slower.

As in T-101 to T-107, the implementation PR adds the benchmark file. This spec PR doesn't, since the benchmarks would call `todo!()` stubs.

## Out of scope

- Text objects (`iw`, `a(` and the rest), registers and `y`, `p` and `P`, `.` repeat, counts on `i`, `a`, `o` and the rest (`3ix<Esc>`), and `:w` `:q` `:e`: T-109. `d` and `c` write no register until then.
- Motions and commands that take a char after them: `f`, `F`, `t`, `T`, `;`, `,` and `r`. They need `KeyState` to treat the next key as an argument, which T-106's keymap doesn't have. A later task.
- Other motions: `%`, `{` and `}`, `(` and `)`, `ge`, `+` and `-`, `H`, `M` and `L`, `*` and `#`, `n` and `N`, and the scrolling keys (`ctrl-d` and the rest), which need the viewport.
- Other operators and edits: `y`, `>`, `<`, `=`, `gu`, `gU`, `g~`, `~` and `J`.
- Visual block mode (`ctrl-v`), replace mode (`R`), marks, the jump list, macros, and `:` commands other than T-109's.
- Insert-mode keys beyond typing, Enter, Backspace, Delete and Escape: arrows, `ctrl-w`, `ctrl-u`, `ctrl-o`.
- Vim's `timeoutlen`, `cpoptions`, `autoindent` and other options, and sticky end of line after `$`.
- Grouping VS Code's typing into undo steps: a later task that can reuse the undo-group methods.
- Showing the mode and the pending count, and closing open undo groups on a keymap switch: `helm-ui`.
- Loading the keymap from TOML and the settings toggle (ST-1): later tasks.
