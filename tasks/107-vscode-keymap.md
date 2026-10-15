# T-107: VS Code keymap, with cursor movement and selection

After: T-106. This task waits until that one is merged.

## Goal

`helm-editor` gets VS Code's default macOS keymap as data, and the commands it needs that don't exist yet: moving cursors by character, line, word, line start and end and the whole text, extending selections the same way with Shift, a sticky goal column for Up and Down, select all, Escape, and deleting by word or to either end of the line. With this, every editing feature built so far is reachable from the keyboard the way a VS Code user expects (ED-4).

PRD: ED-4

## Crates and public interface

Crates: `helm-editor` (the implementation) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only). `helm-bench` already depends on `helm-editor`.

The interface is already in place, with doc comments: a new `motion.rs`, `cursor_commands.rs` and `vscode.rs`, five new `Editor` methods at the end of the first `impl Editor` block in `editor.rs`, and `CommandRegistry::with_all_commands` in `command.rs`. Each new function has a `todo!()` body. Fill them in. Don't change a signature, a type or a doc comment's contract. If one looks wrong, say so in the PR description. Private fields and helpers, including `cursor_commands::register_all`, are placeholders, so replace or add them freely. Delete the `#[expect(unused_variables, ...)]` on each stub and the `#![expect(dead_code, ...)]` at the top of `cursor_commands.rs` once they are filled, or clippy with `-D warnings` fails.

Don't touch `crates/helm-text`. Don't change the behaviour of any existing `Editor` method, except that every `&mut self` method now drops the goal columns, as the `Editor` docs describe. Don't change `editor_commands.rs`: T-106's tests pin its list of ten ids.

```rust
// lib.rs
pub mod cursor_commands;
pub mod vscode;
pub use motion::Motion;

// motion.rs
pub enum Motion { Left, Right, Up, Down, WordLeft, WordRight, LineStart, LineEnd, TextStart, TextEnd }

// editor.rs
impl Editor {
    pub fn move_cursors(&mut self, set: &BufferSet, motion: Motion) -> Result<bool, EditorError>;
    pub fn extend_selections(&mut self, set: &BufferSet, motion: Motion) -> Result<bool, EditorError>;
    pub fn delete_to(&mut self, set: &mut BufferSet, motion: Motion)
        -> Result<Option<TransactionId>, EditorError>;
    pub fn select_all(&mut self, set: &BufferSet) -> Result<bool, EditorError>;
    pub fn collapse_selections(&mut self, set: &BufferSet) -> Result<bool, EditorError>;
}

// command.rs
impl CommandRegistry {
    pub fn with_all_commands() -> Self;   // editor_commands::ALL and cursor_commands::ALL
}

// cursor_commands.rs
pub const LEFT, RIGHT, UP, DOWN, WORD_LEFT, WORD_RIGHT, LINE_START, LINE_END,
          TEXT_START, TEXT_END: &str;                         // "cursor.left", ...
pub const SELECT_LEFT, ..., SELECT_TEXT_END: &str;            // "cursor.select_left", ...
pub const SELECT_ALL, COLLAPSE: &str;                         // "cursor.select_all", "cursor.collapse"
pub const DELETE_WORD_LEFT, DELETE_WORD_RIGHT,
          DELETE_LINE_START, DELETE_LINE_END: &str;           // "cursor.delete_word_left", ...
pub const ALL: [&str; 26];                                    // sorted

// vscode.rs
pub const NAME: &str = "vscode";
pub const MODE: &str = "default";
pub fn keymap() -> Keymap;
```

### Rules the implementation must follow

The doc comments on `Motion`, the new `Editor` methods and `vscode::keymap` hold the full rules. In short:

- **Every motion lands on a char boundary, never inside a CRLF.** CRLF counts as one character for Left, Right and the word motions. A stale head between the CR and the LF moves out of it.
- **Left and Right collapse a selection** to its start or end when moving, as VS Code does. Extending moves the head like any other motion.
- **Up and Down** go to the same column, in chars, on the line above or below, capped before that line's break. On the first line Up goes to offset 0, and on the last line Down goes to the end of the text.
- **Goal columns.** The first Up or Down sets each selection's goal to its head's column. Later Up and Down moves, through `move_cursors` or `extend_selections`, aim for the goal and keep it, including at the first and last lines. Any other `&mut self` method on `Editor` drops every goal. Columns are chars, not display columns, the same unit `add_cursor_above` uses.
- **Words.** Chars are word (`is_alphanumeric` or `_`), blank (whitespace that isn't a line break) or punctuation. WordRight crosses a line break if one starts at the head, and otherwise skips blanks, stops at a line break or the end, then skips one run of word or punctuation chars. WordLeft is the mirror.
- **LineStart** is VS Code's Home: the line's first non-blank, or the line's start if the head is already there. **LineEnd** is the end of the line's text, before its break.
- **`delete_to`** deletes from each cursor to where the motion takes it, or a non-empty selection's range, as one transaction, joined and placed like `delete_backward`.
- **`select_all`** leaves one forward selection of the whole text. **`collapse_selections`** is Escape: it keeps only the primary, and with one selection left it makes it a cursor at its head.
- **Commands.** `with_all_commands` registers the ten `editor_commands` and the 26 `cursor_commands`, each with a non-empty title. Each cursor command calls the `Editor` method and motion its name gives. `with_editor_commands` doesn't change.
- **The keymap.** One mode, `default`, with `Typing::Insert`, and exactly the 39 bindings in the table on `vscode::keymap`, each a plain `Action::Run` with no mode, context or mode switch. Every command it binds is in `with_all_commands`.
- **No whole-text copies.** Motions read only the text near each head, and the lines Up and Down cross. Don't call `to_string` on the buffer.

### The bindings

macOS bindings, written from VS Code's default keyboard shortcuts as I know them. I didn't check them against VS Code's published keybindings file, so check any that look off:

| Keys | Command |
| --- | --- |
| `enter`, `backspace`, `delete` | `editor.newline`, `editor.delete_backward`, `editor.delete_forward` |
| `cmd-z`, `shift-cmd-z` | `editor.undo`, `editor.redo` |
| `cmd-d`, `shift-cmd-l` | `editor.add_next_match`, `editor.select_all_matches` |
| `alt-cmd-up`, `alt-cmd-down` | `editor.add_cursor_above`, `editor.add_cursor_below` |
| `left`, `right`, `up`, `down` | `cursor.left`, `cursor.right`, `cursor.up`, `cursor.down` |
| `alt-left`, `alt-right` | `cursor.word_left`, `cursor.word_right` |
| `home`, `cmd-left` | `cursor.line_start` |
| `end`, `cmd-right` | `cursor.line_end` |
| `cmd-up`, `cmd-down` | `cursor.text_start`, `cursor.text_end` |
| `shift-` plus each of the twelve keys in the five rows above | the matching `cursor.select_` command |
| `cmd-a`, `escape` | `cursor.select_all`, `cursor.collapse` |
| `alt-backspace`, `alt-delete` | `cursor.delete_word_left`, `cursor.delete_word_right` |
| `cmd-backspace`, `cmd-delete` | `cursor.delete_line_start`, `cursor.delete_line_end` |

T-106 asked T-107 to list any binding where folding Shift into chars matters (VS Code's `cmd+shift+[` becomes `cmd-{`). None of these does: every shifted binding is on a named key or a letter.

### Choices I made, and the ones I rejected

- **One `Motion` enum, not one `Editor` method per direction.** Move, extend and delete are three methods that take a motion, so 10 motions give 26 commands without 26 methods. T-108's Vim motions and operators can reuse the same enum and `delete_to`. Adding Vim's motions (`w` versus `e`, `0` versus `^`) means adding variants, not methods.
- **Cursor commands live in a new module with a `cursor.` prefix.** T-106's tests pin `editor_commands::ALL` to its ten ids and `with_editor_commands` to exactly those. Adding the new commands there would mean editing a protected test. A second module and `with_all_commands` leave T-106's grader alone. The cost is two registries to choose from. `helm-ui` should use `with_all_commands`.
- **Goal columns live on the `Editor`, not on `Selection`.** `Selection` is plain `Copy` data compared with `==` all over T-105's tests. A goal field would change what equality means there. The editor drops goals on any other `&mut self` call, which is simpler than tracking which calls move a head.
- **Up on the first line goes to offset 0 and keeps the goal.** So Up then Down returns to the column you started in.
- **Word boundaries stop at line breaks.** WordRight at the end of a line moves just past the break, then the next press goes to the end of the first word. As far as I know, VS Code's `cursorWordEndRight` instead jumps to the end of the next word in one press. I didn't check that against VS Code. Stopping at the break is simpler to state. Only a few rows in `word_right_stops_at_the_end_of_each_run` and `word_left_stops_at_the_start_of_each_run` pin this.
- **`cmd-backspace` deletes to `LineStart`, smart Home included.** From the first non-blank it deletes the indentation, and from after it it deletes back to the first non-blank. VS Code's `deleteAllLeft` may differ on indentation. I didn't check it.
- **`delete_to(LineEnd)` at the end of a line deletes nothing.** It doesn't join the next line. Use `delete` for that.
- **Escape is one command, not two context-scoped bindings.** VS Code binds Escape twice, under `editorHasMultipleSelections` and `editorHasSelection`. `collapse_selections` does both in one, so `helm-ui` doesn't have to compute those contexts.

### Dependencies

None. No new dependency, no change to any `Cargo.toml` except the `[[bench]]` entry, and `Cargo.lock` is unchanged.

## Acceptance tests

Three new files, committed before the implementation. 57 tests:

| File | What it proves |
| --- | --- |
| `crates/helm-editor/tests/motions.rs` (36) | Every motion as tables of before and after texts with markers: Left and Right over ASCII, é, 中, 😀, LF, CRLF and lone CR, out of a CRLF, collapsing a selection; extending, back to a cursor; whether a call changed anything. Up and Down: columns in chars, capped before the line break, first and last lines, CRLF. Goal columns through a short line, at the first and last line, shared between move and extend, kept per cursor, and dropped by another motion, typing, `set_selections` and undo. Cursors merging with the primary kept. WordRight and WordLeft over words, blanks, punctuation, `_`, é, 中, 😀 and each line break. LineStart's toggle, indentation, blank and empty lines. LineEnd before each line break. TextStart and TextEnd. `delete_to` for each deleting motion, over a selection, with nothing to delete, at two cursors as one undo step, joining overlaps, and keeping cursors that delete nothing. `select_all` and `collapse_selections`. Clamping stale offsets, and `UnknownBuffer` changing nothing. |
| `crates/helm-editor/tests/vscode_keymap.rs` (15) | `cursor_commands::ALL` is the 26 documented ids. `with_all_commands` holds both lists, 36 ids, sorted, with titles, and `with_editor_commands` is unchanged. Each of the 26 cursor commands, run through the registry, does what its `Editor` call does. The keymap's name and single mode, and exactly the 39 documented bindings, each a plain run. Every bound command is registered. Each binding resolves to its command. Sessions through the dispatcher: typing and arrows, Home and `cmd-left`, goal columns by key, Shift+Option+Left then typing over the word, `cmd-a` then Delete then `cmd-z`, word and line deletes and their undo, `cmd-d` three times then Escape twice, three cursors moving with End and Option+Left, and Option text still typing. |
| `crates/helm-editor/tests/motion_properties.rs` (6) | proptest, 256 cases per property, over random text of words, blanks, punctuation, each line break, each char width and any char. (1) Every motion from any valid position lands on a char boundary outside any CRLF. (2) Right then Left, and Left then Right, return to the start away from the ends. (3) WordRight and WordLeft always move until the end they head for. (4) Up to 12 cursors moved at once equal each cursor moved alone, in normal form, with the same primary. (5) Extending a cursor keeps its anchor and puts the head where moving would. (6) `delete_to` from one cursor matches a `String` oracle, leaves the cursor at the range's start, and undoes in one step. |

Each test carries `#[ignore = "T-107: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-editor --no-fail-fast -- --ignored` fails all 57 (36 + 15 + 6).

I checked the tests against a throwaway implementation before committing them. All 57 pass on it, and the properties also pass at 20,000 cases each. That implementation isn't in this PR.

**The rule for the implementer: the only allowed change to these three files is deleting the `#[ignore = "T-107: not implemented yet"]` lines.** Nothing else in them changes. That means no edited assertion, no new `#[ignore]`, and no change to a case count. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-1 gives a keypress 5 ms from input to pixel, and a motion at many cursors is on that path. Add a new file, `crates/helm-bench/benches/cursor.rs`, with a `[[bench]] name = "cursor"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds two criterion benchmarks in a group named `cursor`, on the same 10 MiB text `benches/editor.rs` builds:

| Benchmark id | What one iteration does |
| --- | --- |
| `cursor/word_right_1000_cursors_10mb` | `move_cursors(Motion::WordRight)` at 1,000 cursors spread evenly across the text, then `move_cursors(Motion::WordLeft)`. |
| `cursor/down_up_1000_cursors_10mb` | `move_cursors(Motion::Down)` at 1,000 cursors spread evenly across the text, then `move_cursors(Motion::Up)`. |

Set the cursors once, outside the timed code, each at the start of a line so every iteration starts and ends in the same place. Pass each result through `black_box`.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `cursor` benchmarks, so `bench-compare` reports them as head-only in this task's PR and gates them from the next PR on. No absolute limit is gated, but both should be far under a millisecond. A motion that copies the 10 MiB text would show here, so don't. Say in the PR what the benchmarks measured.

As in T-101 to T-106, the implementation PR adds the benchmark file. This spec PR doesn't.

## Out of scope

- Clipboard: cut, copy and paste (ED-1). It needs the macOS clipboard in `CommandContext`, and gets its own task.
- Tab and Shift+Tab, indent and outdent. They need indentation settings (tabs or spaces, width). Until then Tab is unbound and types nothing.
- Line commands: move line up or down, copy line, delete line, toggle comment, join lines.
- Page Up and Page Down, and moving by visual line in wrapped text. Both need the viewport, which is `helm-ui`'s (Phase 2).
- Display columns for tabs and wide characters. Goal columns are in chars until `helm-ui` can measure text.
- macOS Emacs-style keys (`ctrl-a`, `ctrl-e`, `ctrl-k` and the rest).
- Mouse clicks and drags: `helm-ui`.
- Vim's motions, operators and counts: T-108, which can reuse `Motion` and `delete_to`.
- Loading keymaps or a keybindings file from TOML, and the settings toggle (ST-1): later tasks.
- Commands outside the editor, such as save, open and the command palette: `helm-ui`.
