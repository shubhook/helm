# T-109: Vim text objects, registers, `.` repeat, counted inserts and `:w` `:q` `:e`

After: T-108. T-108 is merged (`4e92574`). This spec is written against main at `11010ce`, where Khakha's grants on PR #37 are in: the count is `CommandArgs::count`, Vim's motions are command ids with a crate-private enum, and `de` and `dE` at the end of the text delete the last char.

**Open questions 1 and 2 are answered.** Khakha granted the `tests/commands.rs` edit (question 1) and chose option (a) for `tests/vim_keymap.rs` (question 2); see "Open questions for Khakha". The spec PR compiled without either edit, as "Crates and public interface" explains, and the implementation PR makes both.

## Goal

`helm-editor` gets the rest of ED-5: text objects (`iw`, `aw`, `iW`, `aW`, the bracket and quote pairs, `ip`, `ap`) under `d`, `c`, `y` and in both visual modes; registers, with `y`, `p` and `P`, and `d` and `c` writing them; `.` repeat; counts on `i`, `a`, `I`, `A`, `o` and `O`; and `:w`, `:q` and `:e`. `helm-editor` still knows nothing about windows, files or the system clipboard. A command that needs one returns a `Request`, which `helm-ui` carries out. After this task every item in ED-5's list has a keystroke suite.

PRD: ED-5 (text objects, registers, `.` repeat, `:w` `:q` `:e`), ED-1 (the `+` and `*` registers are the macOS clipboard, and a 1 MB round trip keeps every byte), ED-6 (one registry and one dispatcher still hold both keymaps).

## Crates and public interface

Crates: `helm-editor` (the implementation) and `helm-bench` (a new benchmark file and its `[[bench]]` entry only).

The interface is in `crates/helm-editor/src/` with doc comments and `todo!()` bodies. Fill them in. Don't change a signature, a type or a doc comment's contract; if one looks wrong, say so in the PR description. Private fields and helpers are placeholders. Delete each `#[expect(..., reason = "T-109 interface stub; ...")]` once its stub is filled, or clippy with `-D warnings` fails. The `vim.rs` module docs hold the full rules, in the sections marked T-109.

```rust
// lib.rs
pub use session::{Register, Request, Session};

// session.rs (new)
pub struct Register { pub pieces: Vec<String>, pub linewise: bool }
impl Register { pub fn text(&self, newline: &str) -> String; }
pub enum Request {
    Save { path: Option<PathBuf>, force: bool },   // :w
    Close { force: bool },                         // :q
    Open { path: Option<PathBuf>, force: bool },   // :e
    SetClipboard { text: String },                 // writes to + and *
}
pub struct Session;                                // Clone, Debug, Default
impl Session {
    pub fn new() -> Self;                          // implemented: `run` builds one
    pub fn register(&self, name: char) -> Option<&Register>;
    pub fn set_clipboard(&mut self, text: Option<String>);
    pub fn last_change(&self) -> Option<&[Invocation]>;
}

// keymap.rs
pub enum Typing { Insert, Ignore, Count, Line }    // Line: new
pub enum Action { Run(CommandId), Operator(CommandId), Register(char) }  // Register: new
impl Binding { pub fn register(keys: &str, name: char) -> Result<Self, KeyParseError>; }

// dispatch.rs
impl KeyState {
    pub fn register(&self) -> Option<char>;
    pub fn line(&self) -> Option<&str>;
}
impl Dispatcher {
    pub fn dispatch_in(&self, state: &mut KeyState, event: &KeyEvent, contexts: &[&str],
                       editor: &mut Editor, set: &mut BufferSet, session: &mut Session)
        -> Result<(Resolution, Vec<Request>), CommandError>;
}

// command.rs
pub struct CommandContext<'a> {
    pub editor, pub set, pub args,                 // as before
    pub registry: &'a CommandRegistry,             // new
    pub session: &'a mut Session,                  // new
    pub requests: &'a mut Vec<Request>,            // new
}
impl CommandRegistry {
    pub fn run_in(&self, invocation: &Invocation, editor: &mut Editor, set: &mut BufferSet,
                  session: &mut Session) -> Result<Vec<Request>, CommandError>;
    pub fn mark_change(&mut self, id: &str);
    pub fn is_change(&self, id: &str) -> bool;
}
pub enum CommandError { ..., Ex { line: String, error: vim::ExError } }   // Ex: new

// vim.rs
pub const YANK_PENDING: &str = "yank_pending";
pub const COMMAND_LINE: &str = "command_line";
pub enum ExError { NotACommand, TrailingCharacters, Range }

// vim_commands.rs: 52 new ids
pub const YANK, YANK_LINE, PUT_AFTER, PUT_BEFORE, YANK_SELECTION, YANK_LINES,
          PUT_SELECTION, PUT_SELECTION_KEEP, PUT_LINES, PUT_LINES_KEEP, REPEAT, EX: &str;
pub const INNER_WORD, A_WORD, SELECT_INNER_WORD, SELECT_A_WORD, ...: &str;  // 4 per object, 10 objects
```

Two existing functions change, as their doc comments now say. `CommandRegistry::run` runs with a fresh `Session` and drops the requests, so T-106 to T-108's callers see no difference; it is already filled in. `Dispatcher::resolve` gains the register rule, the command-line rule (rule 5) and a pending register in rules 2 and 3, and `KeyState::cancel` drops a pending register and leaves the command line. Their code is unchanged in this PR except for one `todo!()` arm for `Action::Register`, which no existing binding reaches. Every T-101 to T-108 and T-110 test still passes.

### How this compiles without the two pending grants

- **The register argument (open question 1).** `resolve` must hand the pending register to the command, and `.` must replay it. The natural place is a `CommandArgs::register` field, but adding one breaks the two `CommandArgs` literals in `tests/commands.rs` (lines 266 and 289). So this PR adds no field. The docs say where the register goes depends on question 1, and every test reaches registers only through keys and `Session::register`, never by naming the field. The tests hold whichever way question 1 goes.
- **T-108's pinned lists (open question 2).** `tests/vim_keymap.rs` pins `vim::keymap()` at six modes and 115 bindings, `vim_commands::ALL` at 55 ids (`[&str; 55]`) and `with_vim_commands()` at 91 commands. So this PR adds the 52 ids as constants outside `ALL`, documents the 391 bindings and two modes on `vim::keymap` as T-109's, and registers nothing. The new tests reach the keymap and registry through one helper, `tests/vim_harness/mod.rs`, which names `vim::keymap()` and `with_vim_commands()`. Under option (a) that helper stays; under option (b) its two lines name the new functions.

### What the existing code and tests allow

Checked with grep at `11010ce`:

- `CommandContext` is built in one place, `CommandRegistry::run`. No test or benchmark builds one, so its new fields break nothing.
- `Command` literals appear in `tests/commands.rs` and `tests/counts.rs`, so change marks live in the registry, not in `Command`.
- No test or benchmark matches `Typing`, `Action`, `Resolution` or `CommandError` exhaustively, so the new variants break nothing.
- No T-108 test types a count before `i`, `a`, `I`, `A`, `o` or `O`, or presses `y`, `p`, `P`, `.`, `:` or `"` in a counting mode. `vim_visual.rs`'s `unbound_keys_do_nothing_in_visual_modes` presses `Vi` and checks only the text and selections; with T-109 `i` starts a chord there, and the test still passes (checked on the throwaway).
- Since PR #38's head, main changed only `resolve`'s two fast paths (`fd838b4`). Rule 3's skip must also check a pending register.

## Rules the implementation must follow

The `vim.rs` module docs hold the full rules: "Text objects", "Registers", "`.` repeat", "Counted inserts" and "The command line", all marked T-109. `Typing::Line`, `Action::Register`, `Dispatcher::resolve`, `KeyState`, `CommandRegistry::run_in`, `Session` and `Request` document theirs. In short:

- **Objects** give a charwise or linewise range from each cursor, with counts, and act under `d`, `c`, `y` and in visual modes. Brackets keep their own lines when a block spans lines, and that inner block is linewise. Quotes stay in one line. Paragraphs are runs of blank or non-blank lines.
- **Registers** follow `:help registers`, with one piece per cursor. `+` and `*` write a `SetClipboard` request, and `helm-ui` keeps them current with `Session::set_clipboard`. Puts keep the register's bytes exactly.
- **`.`** replays the last change's `Invocation`s through the registry, with their full `CommandArgs`, so the count already in `CommandArgs` comes back and a char field added later for `f`, `t` and `r` would too. A count replaces the first invocation's count. A visual change isn't recorded.
- **Counted inserts** repeat the typed invocations on Escape, in the same undo group.
- **`:w` `:q` `:e`** are typed into `KeyState`'s line and parsed by `vim.ex` into requests.

### Cost per cursor

Every new command keeps T-108's rules: read only the text near each cursor and the lines it crosses, never call `to_string` on the buffer, and never look up the line index for every char. In detail:

- `iw`, `aw`, `iW` and `aW`, with any count, read forward and back from the cursor a window at a time, with T-108's readers, and look at the line index only where they need a line's start or end. A count costs the text it crosses.
- The quote objects read the cursor's line only, from its start, since the pairing counts from there. That is one `line_to_byte` lookup per cursor, plus the line up to the closing quote.
- The bracket objects read outward from the cursor a window at a time and use no line-index lookups while they scan. The block rule for `i{` adds a fixed number of lookups at the end. They cost the text between the cursor and the pair they find. With no enclosing pair, that is the whole text on each side (open question 6).
- `ip` and `ap` cost a fixed number of line-index lookups per line they cross, plus reading each crossed line up to its first non-blank to decide whether it is blank. A count costs the lines it crosses.
- Linewise objects turn lines into bytes with a fixed number of line-index lookups per span, as T-108's linewise operators do.
- `y` copies the yanked bytes and nothing else. It applies no transaction and doesn't change the buffer's version. `yy` with any count is a fixed number of line-index lookups per cursor.
- `p` and `P` apply one transaction for all cursors. A count builds each cursor's text once, `n` copies long. It never applies `n` transactions.
- A register holds one `String` per cursor that wrote it.
- `.` costs what running its invocations again costs. Recording costs one `Invocation` clone per command while a change is open, and reads no text.
- A counted insert folds its repeats into the open group with `BufferSet::amend`, so `100ix<Esc>` costs about what typing 100 chars costs in T-110's amend benchmark.
- The command line touches no buffer. Each key costs the line's length at most.
- `Session::set_clipboard` compares the new text with the last text Helm wrote. That costs at most the text's length, once per pasteboard change, not per key.

## Choices I made, and the ones I rejected

- **Requests returned to `helm-ui`, not callbacks.** A command pushes plain data and returns. I rejected a `Clipboard` or `Files` trait passed into commands: it puts platform calls inside the command path, makes `.` and the property tests depend on outside state, and gives the clipboard and the files two different routes. I also rejected making `:w` an id that `helm-ui` runs itself, as `CommandRegistry`'s docs allow: the parsing and Vim's rules would then live in the one crate that can't be tested headless.
- **`helm-ui` keeps the clipboard registers current.** The other way to read the clipboard is a request that asks for the text and a second call that finishes the paste. That splits `"+p` into two steps, and `.` and counts would have to survive between them.
- **Registers and the last change in a `Session` passed in, not in `Dispatcher`.** `dispatch` takes `&self`, and keeping registers there would need interior mutability.
- **`.` records `Invocation`s, not keys.** Keys depend on the keymap and the mode at replay time. Invocations already hold everything the keys resolved to, so a future char argument comes along for free.
- **Change marks in the registry, not a field on `Command`.** `tests/commands.rs` builds `Command` literals, so a new field would break it.
- **Register names by chord (`" a`) instead of reading the next key as an argument.** Khakha put the char-argument mechanism, which `f`, `t` and `r` need, in a later task. Chords cost 201 bindings, all data, and no new `KeyState` rule beyond holding the register. When the later task adds char arguments, it can replace the chords.
- **The command line lives in `KeyState`, behind `Typing::Line`.** Then `:w<CR>` is a keystroke row in a table test, which is ED-5's acceptance format. A command line owned by `helm-ui` would put that out of reach.
- **Pastes keep their bytes.** Converting line breaks to the file's format on paste would break ED-1's round trip, and Vim doesn't convert either.
- **The unnamed register stays separate from the clipboard**, as in Vim with default options. See open question 5.

- **An object edit's undo cursor is the range's start.** T-108's undo puts back the cursors from before an edit, which for its motions is where the change began. For `ciw` from inside a word it isn't, and Vim puts the cursor at the word's start. So an object edit records a cursor at each range's start as its "before". T-108's pinned undo rows are all motions and keep passing.
- **The tests reach the keymap and registry through one helper.** `tests/vim_harness/mod.rs` names `vim::keymap()` and `with_vim_commands()` once. That leaves open question 2 open: whichever way Khakha answers, one line changes. Nothing in this PR adds a T-109 id to `vim_commands::ALL`, a binding to `vim::keymap()` or a command to `with_vim_commands()`, since each would fail T-108's pinned tests now.

## Acceptance tests

Seven new test files and one shared helper module, committed before the implementation. 74 tests: 73 carry `#[ignore = "T-109: not implemented yet"]`, and `the_new_ids_are_the_documented_ones` passes now, since the id constants are plain data.

| File | What it proves |
| --- | --- |
| `tests/vim_harness/mod.rs` | Not a test. Marked text, Vim key notation (`<lt>` for a literal `<`), and a `Vim` that dispatches each key through `dispatch_in` with a `Session` and keeps every request. The only place the new tests name the keymap and registry. |
| `tests/session.rs` (11) | With a small keymap and test commands, no Vim. A register waits for the next `Run`, is replaced by a second, moves onto an operator, and goes with a count in either order. It drops on an unbound key, Escape, a broken chord, a binding with no action, `cancel`, an operator pending, and a keymap switch. The command line collects typed text, Backspace edits it, Ctrl types nothing, Enter runs with the line as its text, and Escape, Backspace on an empty line, `cancel` and a keymap switch leave it. `dispatch_in` returns requests in order, `dispatch` drops them, and `run_in` passes the registry and session to nested runs and returns errors with no requests. Marks. Recording: unmarked commands record nothing, a marked one with no group is the whole change, one that opens a group records until it closes, a failed run isn't recorded, and a group closed from outside ends the record at the next run. |
| `tests/vim_objects.rs` (12) | Tables for every object under `d`, `c` and `y` and in both visual modes. Word classes, white space on either side, line ends, CJK and emoji, and counts on either side of the operator. Brackets nested, on a bracket, across lines, with counts, the block-of-lines rule, a closing bracket alone on its line, and no pair. Quotes escaped, with the cursor on a quote, before the first pair, or between pairs, and on another line. Paragraphs over blank runs, white-space-only lines, first and last paragraphs, counts and CRLF. `c` undoing in one step with the cursor at the range's start. Switching between visual modes. Several cursors, and lines longer than a rope chunk. |
| `tests/vim_registers.rs` (15) | `y` with motions, `yy`, `Y` and counts, with no transaction. `p` and `P`, charwise and linewise, with counts, multi-line text and the last line. `-`, `1` to `9` shifting, `0`, named, append (charwise and linewise), `_` and unnamed. `d`, `c`, `x` and visual `d` writing. Visual `y`, `p` and `P`. Several cursors with matching and non-matching pieces, linewise pieces, and one undo step. An empty register. `+` and `*` pushing `SetClipboard`, reading `set_clipboard`, keeping pieces of text Helm wrote, a 1 MB round trip with CRLF, lone CR, every char width and a BOM char (ED-1), and registers shared across editors. |
| `tests/vim_repeat.rs` (15) | `.` after `x`, counts, `dw`, `2d3w`, `dd`, `D`, `X`, `daw`, `cw` and `ciw` with typing, `A`, `o`, `O`, `i`, `a`, `s`, `S`, `p`, `P` and `"1p...`. `.` with a count replacing the recorded one. One undo step per `.`. Motions, yanks, `u` and `ctrl-r` not recorded. A visual change not recorded. `.` with no change. Several cursors. The record's invocations and counts, and the record while typing. Another editor on the same session. A record ended by a group closed from outside. Counted `i`, `a`, `I`, `A`, `o` and `O` with Enter, Backspace and CRLF, `.` and `2.` after one, several cursors, and `c`, `s`, `S` and `C` keeping T-108's count. Replaying whole `CommandArgs`. |
| `tests/vim_ex.rs` (8) | Each line form and the requests it makes, with the buffer unchanged and normal mode after. Leading colons, white space, empty lines, paths with spaces. Each error, with no request. `KeyState::line` while typing. Escape and Backspace leaving. A count before `:` dropped. `:` commands not changes. A keymap switch or `cancel` leaving the line. |
| `tests/vim_t109_keymap.rs` (8) | The 52 new ids. Every new command registered with a title, and every T-108 one still there. Which commands are marked as changes. The two new modes and their typing. Exactly the 391 documented new bindings after T-108's 115, every bound command registered. Each object chord resolving under `d`, `c`, `y`, `v` and `V` to the right command and mode. Each register chord in normal and both visual modes, not in insert mode or under an operator. A broken object chord cancelling the operator. |
| `tests/vim_t109_properties.rs` (5) | proptest, 256 cases each, over random text of words, blanks, punctuation, brackets, quotes, each line break, each char width and any char. (1) `.` after a change and a move does what typing the change again does. (2) Yanking an object's visual selection and putting it back over the same selection, and `yy` then `Vp`, change nothing. (3) `{n}i` types `n` copies and one `u` restores the text. (4) Any text through the clipboard comes back byte for byte. (5) Up to three objects under `d` at up to two cursors leave every cursor on a normal-mode position, and as many `u` as edits restore the text. |

T-108's plan had a separate `String` oracle file for objects. This spec covers the same ground with table rows on lines over 1,000 bytes, which cross rope chunks, and with property 5. The implementer's rule stays T-108's: **the only allowed change to these files is deleting the `#[ignore]` lines.**

### The throwaway check

I checked the tests against a throwaway implementation, not in this PR and not committed. On it:

- All 74 T-109 tests pass, and the five properties also pass at 20,000 cases each. Running them at 20,000 cases found two rules this spec now states: a closing bracket alone on its line keeps its line break, as Vim does (measured), and a visual object can't end on the line break of a line with text, which is why property 2 yanks the visual selection.
- Every other workspace test passes (473 in all) except exactly three in T-108's `vim_keymap.rs`: `with_vim_commands_holds_every_command` (91), `the_keymap_has_vims_modes` (six modes) and `the_keymap_has_exactly_the_documented_bindings` (115). Those are open question 2's pins.
- It used a `CommandArgs::register` field, so `tests/commands.rs` compiled only with its two `count: None` lines changed to `..CommandArgs::default()`, which is open question 1's proposed edit.

The throwaway reads objects from a `String` copy of the text, which the cost rules forbid, so it says nothing about speed.

### Measured against Vim

I ran every table row of `vim_objects.rs`, `vim_registers.rs` and `vim_repeat.rs` with one cursor and no CR through Vim 9.1 (`vim -u NONE -N -es`, `binary`, `backspace=indent,eol,start`), feeding the keys with `feedkeys(..., "xt")` and comparing the text, the cursor and, for register rows, the register and its type. Rows with several cursors, a CR, the clipboard or `:` were skipped (11 rows), since Vim can't run them or reads them another way. Of 204 rows run, 4 differ, all on purpose:

- `d5iw` past the end of the line: nothing happens in both, but Vim moves the cursor to the last char it reached. Helm leaves it.
- `ci(` with no pair: Vim stays in normal mode, so the typed `X` runs as a command. Helm enters insert mode, as T-108's `cj` on the last line does (T-108's open question 5, kept by Khakha).
- `xvld.` and `xVjd.`: Vim repeats the visual delete; Helm repeats the `x` before it (open question 4).

Two rows I first wrote wrong (`d2i"` and `Ox<esc>j.`) now pin what Vim does. A small Python script in my scratch directory drove Vim; neither it nor its output is in the PR.

## Performance budget

NF-1 gives a keypress 5 ms from input to pixel. As in T-108, these benchmarks time only the command, from the key event or invocation to the changed buffer, selections and session, so a time here is a part of NF-1's 5 ms, not a measure of it.

`benches/vim.rs` is T-108's and an existing file, so editing it is a protected change. T-109 adds a new file instead, `crates/helm-bench/benches/vim_objects.rs`, with a `[[bench]] name = "vim_objects"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`, holding a criterion group named `vim_objects`. It copies `LINES`, `floor_boundary` and `build_text` from `benches/cursor.rs` unchanged, so the text is the same 10 MiB, and places cursors the way `benches/vim.rs` does: one on each of 1,000 lines of the same kind, spread evenly through the text. `LINES` already holds every object the benchmarks need: `{` and `}` around each block, a quoted string, `(` in `println!`, `[` in the emoji line, and a blank line after each block.

| Benchmark id | What one iteration does | Target |
| --- | --- | --- |
| `vim_objects/diw_undo_1000_cursors_10mb` | `diw` then `u`, with the cursors at the start of `fn main() {` lines | none |
| `vim_objects/ci_quote_undo_1000_cursors_10mb` | `ci"`, `x`, Escape, `u`, with the cursors at the start of `let café` lines, before the quote | none |
| `vim_objects/di_brace_undo_1000_cursors_10mb` | `di{` then `u`, with the cursors at the start of `println!` lines, so each empties a block of four lines | none |
| `vim_objects/di_paren_no_pair_10mb` | `di(` at one cursor on the comment line nearest the middle of the text, which no `(` encloses, so the scan crosses the whole text | none |
| `vim_objects/dap_undo_1000_cursors_10mb` | `dap` then `u`, with the cursors on `fn main() {` lines | none |
| `vim_objects/yy_p_undo_1000_cursors_10mb` | `yy`, `p`, `u`, each cursor putting its own piece | none |
| `vim_objects/repeat_ciw_1000_cursors_10mb` | `.` then `u`, after `ciwhelm<Esc>` is recorded once outside the timed code | none |
| `vim_objects/insert_count_100_10mb` | `100ix<Esc>` then `u`, at one cursor, through the session dispatch | none |
| `vim_objects/clipboard_paste_1mb` | `Session::set_clipboard` with a 1 MB string, `"+P`, `u`, at one cursor | none |

Each iteration ends where it began: `u` puts back the cursors from before the edit, and `yy` changes nothing. Build the text, set, registry, dispatcher, session and events once, outside the timed code, and pass each result through `black_box`.

No benchmark has a fixed target. Khakha's answer 7 on PR #37 kept fixed targets as ungated review aids and gated new benchmarks by the 5% default only, and I have no CI baseline for any of these paths yet. If Khakha wants one, it will be a criterion mean on the CI `ubuntu-latest` `bench` job, ungated, with its evidence stated as T-108's were (open question 8). `di_paren_no_pair_10mb` is there to measure the worst case behind open question 6.

Budget: the default 5% from `budgets.toml`, with no per-benchmark override. The base commit has no `vim_objects` benchmarks, so `bench-compare` reports them as head-only in this task's PR and gates them from the next PR on. This task changes `resolve` (a pending register and a line), adds 391 bindings to Vim's keymap, and records every run while a change is open. So T-108's three `vim/` targets stay ungated review aids, reported from the artifact, and every `vim/`, `keymap/`, `cursor/` and `editor/` benchmark must not get more than 5% slower.

As in T-101 to T-108, the implementation PR adds the benchmark file. This spec PR doesn't, since the benchmarks would call `todo!()` stubs.

## Out of scope

- **`f`, `t`, `F`, `T`, `;`, `,` and `r`.** They need `KeyState` to read the next key as a char argument. Khakha placed them in their own task after T-109. `.` is built so that task can add a char field to `CommandArgs` and have it replayed with no change to `.`.
- Other text objects: sentences (`is`, `as`), tags (`it`, `at`), and objects from language servers or tree-sitter.
- Other registers: `.`, `:`, `%`, `#`, `/`, `=`, and `ctrl-r` in insert mode. `:registers`.
- `gp`, `gP`, `]p`, visual block mode, and blockwise registers.
- `.` after a visual change (open question 4).
- `:` commands other than the table above, ranges, `:x`, `ZZ` and `ZQ`, command-line history, completion and editing keys beyond Backspace, and `:` from a visual mode.
- Where a relative path is resolved, and how a failed request is shown: `helm-ui`.
- Linking the unnamed register to the clipboard (`clipboard=unnamed`): a setting for a later task.
- The other T-108 out-of-scope items stay out: other motions and operators, marks, macros, `autoindent`.

## Open questions for Khakha

Questions 1 and 2 blocked the implementation, and Khakha has answered both. Questions 3 to 8 don't block; Khakha kept the spec's default for each.

Khakha's answers, as written:

> Answers to tasks/109-vim-objects.md's open questions, from Khakha:
> - Question 1: Granted. In crates/helm-editor/tests/commands.rs, replace count: None with ..CommandArgs::default() in the two CommandArgs literals (lines 266 and 289). No assertion changes. Add CommandArgs::register.
> - Question 2: Option (a), granted. Edit crates/helm-editor/tests/vim_keymap.rs so vim_commands::ALL is 107 ids, with_vim_commands() is 143 commands, and the_keymap_has_vims_modes and the_keymap_has_exactly_the_documented_bindings describe the full keymap. T-108's 115 documented binding rows stay unchanged. Add the T-109 ids, bindings and commands to ALL, vim::keymap() and with_vim_commands(), and keep tests/vim_harness/mod.rs naming them.
> - Questions 3 to 8: keep the spec's defaults.

1. **`CommandArgs::register` and the `tests/commands.rs` literals (answered: granted).** The register must travel with the invocation so `.` replays it, as the count does. Would you grant replacing `count: None` with `..CommandArgs::default()` in the two `CommandArgs` literals of `tests/commands.rs` (lines 266 and 289), with no assertion changed? Then the register now, and the char argument for `f`, `t` and `r` later, need no further grant. The alternatives are granting `register: None` now and asking again later, or no grant, in which case the register travels outside `CommandArgs` (in `CommandContext`, with `.` recording it beside each invocation) and `Resolution::Run` can't carry it, so `resolve` would need another way to hand it out.

   Khakha granted the edit: the register is `CommandArgs::register`.
2. **T-108's pinned keymap and lists (answered: option (a)).** T-109 adds two modes, 391 bindings, 52 ids and 52 commands.
   - (a) Grant edits to `tests/vim_keymap.rs` so `vim_commands::ALL` grows to 107 ids, `with_vim_commands` to 143 commands, and `the_keymap_has_vims_modes` and `the_keymap_has_exactly_the_documented_bindings` describe the full keymap; the documented 115 rows stay as they are. I recommend this: one keymap, one id list, one registry.
   - (b) No test edits: `vim::keymap()`, `ALL` and `with_vim_commands()` keep T-108's contents, and T-109 adds a second keymap function, a second id list and a fourth registry constructor for `helm-ui`. Every later Vim task would add another layer.

   Khakha chose (a): `ALL` is 107 ids, `with_vim_commands()` 143 commands, and `vim::keymap()` eight modes and 506 bindings, T-108's 115 rows first and unchanged.
3. **Request shape (default: as stubbed).** `Save`, `Close`, `Open` and `SetClipboard`, with `PathBuf` paths and a `force` flag, and `:q` closing the editor while `helm-ui` decides about the window.
4. **`.` after a visual change (default: out of T-109).** Vim repeats it over the same amount of text from the cursor (`:help visual-repeat`). Include it now, or a later task?
5. **Unnamed register and the clipboard (default: kept apart).** As in Vim by default; `clipboard=unnamed` would be a later setting.
6. **Bracket objects with no pair (default: unbounded, measured).** In a 10 MB file the scan crosses the whole text, possibly past NF-1's 5 ms. `vim_objects/di_paren_no_pair_10mb` measures it. Keep it unbounded, as T-108 did for huge counts, or cap the scan (say 1 MB each way) and treat a capped scan as no pair?
7. **The name `Session` (default: `Session`).** It may be confused with agent sessions (AG-10). `EditorSession` is the alternative.
8. **Speed targets (default: none).** Do you want ungated review-aid targets for any of the nine benchmarks, as criterion means on the CI `ubuntu-latest` bench job, once main's bench baseline holds the `vim/` group?
