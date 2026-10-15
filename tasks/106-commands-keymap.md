# T-106: Command registry and a keymap engine with modes

After: T-105. This task waits until that one is merged.

## Goal

`helm-editor` gets a command registry, where every command has a stable string id and runs against an `Editor` and its `BufferSet`, and a keymap engine that turns headless key events into commands. A keymap is data: modes, plus bindings from keys and chords to command ids, scoped by mode and an optional context. VS Code (T-107) and Vim (T-108) will each be one keymap. Switching the active keymap takes effect in every open editor on its next key, with no restart (ED-6).

PRD: ED-4, ED-6

## Crates and public interface

Crates: `helm-editor` (the implementation) and `helm-bench` (the new benchmark file and its `[[bench]]` entry only). `helm-bench` already depends on `helm-editor`.

The interface is already in four new files in `crates/helm-editor/src/` (`key.rs`, `command.rs`, `keymap.rs`, `dispatch.rs`) plus `editor_commands.rs`, with doc comments. `CommandRegistry::commands` and the `editor_commands` constants already work. Everything else has a `todo!()` body. Fill them in. Don't change a signature, a type or a doc comment's contract. If one looks wrong, say so in the PR description. Private fields and the private helpers (`parse_key`, `is_valid_id`, `register_all`, `insert`) are placeholders, so replace them freely. Each new file starts with `#![expect(unused_variables, dead_code, ...)]`. Delete it once that file's stubs are gone, or clippy with `-D warnings` fails.

Don't touch `crates/helm-text`, since T-110 is being written there. Don't change `editor.rs`, `selection.rs` or `error.rs`. This task needs nothing new from them.

```rust
// lib.rs
pub mod editor_commands;
pub use command::{Command, CommandArgs, CommandContext, CommandError, CommandFn, CommandId,
                  CommandRegistry, Invocation, RegisterError};
pub use dispatch::{Dispatcher, KeyState, Resolution};
pub use key::{Key, KeyEvent, KeyParseError, KeySequence, Keystroke, Modifiers};
pub use keymap::{Action, Binding, Keymap, KeymapError, Mode, ModeId, Typing};

// key.rs
pub enum Key { Char(char), Enter, Tab, Backspace, Delete, Escape, Left, Right, Up, Down,
               Home, End, PageUp, PageDown, F(u8) }
pub struct Modifiers { pub ctrl: bool, pub alt: bool, pub shift: bool, pub cmd: bool }
impl Modifiers { pub const NONE, CTRL, ALT, SHIFT, CMD: Self; }

pub struct Keystroke { /* private: key, modifiers, in normal form */ }
impl Keystroke {
    pub fn new(key: Key, modifiers: Modifiers) -> Self;
    pub fn key(&self) -> Key;
    pub fn modifiers(&self) -> Modifiers;
    pub fn parse(text: &str) -> Result<Self, KeyParseError>;
}
impl Display for Keystroke;

pub struct KeySequence { /* private, never empty */ }
impl KeySequence {
    pub fn new(keys: Vec<Keystroke>) -> Option<Self>;
    pub fn parse(text: &str) -> Result<Self, KeyParseError>;
    pub fn as_slice(&self) -> &[Keystroke];
}
impl From<Keystroke> for KeySequence;
impl Display for KeySequence;

pub struct KeyEvent { pub keystroke: Keystroke, pub text: Option<String> }
impl KeyEvent {
    pub fn new(keystroke: Keystroke) -> Self;
    pub fn with_text(keystroke: Keystroke, text: impl Into<String>) -> Self;
    pub fn parse(text: &str) -> Result<Self, KeyParseError>;
}
pub enum KeyParseError { Empty, MissingKey { keystroke: String }, RepeatedModifier { keystroke: String },
                         UnknownKey { keystroke: String }, ShiftedChar { keystroke: String } }

// command.rs
pub struct CommandId(Arc<str>);   // private field
impl CommandId { pub fn as_str(&self) -> &str; }
impl From<&str>, From<String>, Display for CommandId;

pub struct CommandArgs { pub text: Option<String>, pub motion: Option<CommandId> }
pub struct Invocation { pub command: CommandId, pub args: CommandArgs }
impl Invocation {
    pub fn new(command: impl Into<CommandId>) -> Self;
    pub fn insert(text: impl Into<String>) -> Self;
}
pub struct CommandContext<'a> { pub editor: &'a mut Editor, pub set: &'a mut BufferSet, pub args: &'a CommandArgs }
pub type CommandFn = fn(CommandContext<'_>) -> Result<(), CommandError>;
pub struct Command { pub id: CommandId, pub title: &'static str, pub run: CommandFn }

pub struct CommandRegistry { /* private */ }
impl CommandRegistry {
    pub fn new() -> Self;
    pub fn with_editor_commands() -> Self;
    pub fn register(&mut self, command: Command) -> Result<(), RegisterError>;
    pub fn get(&self, id: &str) -> Option<&Command>;
    pub fn commands(&self) -> impl Iterator<Item = &Command>;
    pub fn run(&self, invocation: &Invocation, editor: &mut Editor, set: &mut BufferSet) -> Result<(), CommandError>;
}
pub enum RegisterError { InvalidId { id: CommandId }, Duplicate { id: CommandId } }
pub enum CommandError { UnknownCommand { id: CommandId }, MissingText { id: CommandId }, Editor(EditorError) }
impl From<EditorError> for CommandError;

// editor_commands.rs
pub const INSERT, NEWLINE, DELETE_BACKWARD, DELETE_FORWARD, ADD_NEXT_MATCH, SELECT_ALL_MATCHES,
          ADD_CURSOR_ABOVE, ADD_CURSOR_BELOW, UNDO, REDO: &str;   // "editor.insert", ...
pub const ALL: [&str; 10];                                        // sorted

// keymap.rs
pub struct ModeId(Arc<str>);   // private field; as_str, From<&str>, From<String>, Display
pub enum Typing { Insert, Ignore }
pub struct Mode { pub id: ModeId, pub typing: Typing }
impl Mode { pub fn new(id: impl Into<ModeId>, typing: Typing) -> Self; }
pub enum Action { Run(CommandId), Operator(CommandId) }
pub struct Binding {
    pub keys: KeySequence,
    pub mode: Option<ModeId>,
    pub context: Option<String>,
    pub action: Option<Action>,
    pub then_mode: Option<ModeId>,
}
impl Binding {
    pub fn run(keys: &str, command: impl Into<CommandId>) -> Result<Self, KeyParseError>;
    pub fn operator(keys: &str, command: impl Into<CommandId>) -> Result<Self, KeyParseError>;
    pub fn switch_mode(keys: &str, mode: impl Into<ModeId>) -> Result<Self, KeyParseError>;
    pub fn in_mode(self, mode: impl Into<ModeId>) -> Self;
    pub fn when(self, context: impl Into<String>) -> Self;
    pub fn then_mode(self, mode: impl Into<ModeId>) -> Self;
}
pub struct Keymap { /* private */ }
impl Keymap {
    pub fn new(name: impl Into<String>, modes: Vec<Mode>) -> Result<Self, KeymapError>;
    pub fn name(&self) -> &str;
    pub fn modes(&self) -> &[Mode];
    pub fn mode(&self, id: &str) -> Option<&Mode>;
    pub fn initial_mode(&self) -> &ModeId;
    pub fn bind(&mut self, binding: Binding) -> Result<(), KeymapError>;
    pub fn bindings(&self) -> &[Binding];
}
pub enum KeymapError { NoModes, DuplicateMode { mode: ModeId }, UnknownMode { mode: ModeId } }

// dispatch.rs
pub struct KeyState { /* private: mode, pending keys, pending operator, keymap generation */ }
impl KeyState {
    pub fn mode(&self) -> &ModeId;
    pub fn pending(&self) -> &[Keystroke];
    pub fn operator(&self) -> Option<&CommandId>;
    pub fn cancel(&mut self) -> bool;
}
pub enum Resolution { Pending, Run(Invocation), Done, Unbound, Cancelled }
pub struct Dispatcher { /* private: registry, keymap, generation */ }
impl Dispatcher {
    pub fn new(registry: CommandRegistry, keymap: Keymap) -> Self;
    pub fn registry(&self) -> &CommandRegistry;
    pub fn keymap(&self) -> &Keymap;
    pub fn set_keymap(&mut self, keymap: Keymap) -> Keymap;
    pub fn key_state(&self) -> KeyState;
    pub fn sync(&self, state: &mut KeyState);
    pub fn resolve(&self, state: &mut KeyState, event: &KeyEvent, contexts: &[&str]) -> Resolution;
    pub fn dispatch(&self, state: &mut KeyState, event: &KeyEvent, contexts: &[&str],
                    editor: &mut Editor, set: &mut BufferSet) -> Result<Resolution, CommandError>;
}
```

The error types also implement `Display` and `std::error::Error`.

### How the pieces fit

`helm-ui` owns one `Dispatcher`, which holds the active keymap and the registry. Each editor view owns an `Editor` and a `KeyState`. A key press goes:

1. The platform event becomes a `KeyEvent` in `helm-ui` (Phase 2). No GPUI type reaches `helm-editor`.
2. `Dispatcher::resolve(&mut state, &event, contexts)` updates the editor's `KeyState` and returns a `Resolution`.
3. For `Resolution::Run(invocation)`, `helm-ui` runs it. Ids the registry holds go to `CommandRegistry::run`. Ids `helm-ui` owns, such as saving a file, it runs itself. `Dispatcher::dispatch` does steps 2 and 3 for editor commands, which is what the tests use.

Typing is not a special case. A key no binding matches resolves to `Invocation::insert(text)`, an ordinary run of `editor.insert`, so typed text and bound commands take the same path and the same undo history.

### Rules the implementation must follow

The doc comments hold the full rules. In short:

- **Keystrokes.** `Key::Char` holds the char after Shift and the layout, before Alt, Ctrl and Cmd. Shift is folded into chars. `Keystroke::new` turns an ASCII lowercase letter with Shift into the uppercase letter and drops Shift from every other char. Named keys keep Shift. So `shift-a`, `A` and Shift+A are one keystroke, and `shift-tab` isn't `tab`.
- **Notation.** Modifiers `ctrl-`, `alt-`, `shift-`, `cmd-` in any order, each at most once, then one char or a key name (`enter`, `tab`, `backspace`, `delete`, `escape`, `space`, `left`, `right`, `up`, `down`, `home`, `end`, `pageup`, `pagedown`, `f1` to `f20`). Lowercase only. `cmd--` is Cmd and minus. `shift-` before a char other than an ASCII letter is `ShiftedChar`, because which char Shift makes depends on the layout. `Display` writes `ctrl`, `alt`, `shift`, `cmd` in that order, and parsing its output gives the same keystroke. A sequence is keystrokes separated by whitespace.
- **Event text.** `KeyEvent::new` gives a `Key::Char` with none of Alt, Ctrl or Cmd its char as text. Anything else has no text. `helm-ui` uses `with_text` when the platform reports text the keystroke doesn't give, such as `å` for Option+A.
- **Command ids.** One or more `.`-separated segments of `[a-z0-9_]+`. `register` rejects other ids and duplicates, and adds nothing on an error. `commands()` lists commands sorted by id, which the command palette (ST-3) will use.
- **Editor commands.** `with_editor_commands` registers the ten ids in `editor_commands::ALL`, each running the `Editor` method of the same name, with a non-empty title. `editor.insert` types `args.text` and errors with `MissingText` without it. Running an id the registry doesn't hold is `UnknownCommand`. An `EditorError` comes back as `CommandError::Editor`.
- **Keymaps.** `Keymap::new` needs at least one mode and unique mode ids. The first mode is the initial mode. `bind` rejects a binding whose `mode` or `then_mode` names a mode the keymap lacks. Bindings keep the order they were added in. A binding may name a command id no registry holds.
- **Modes.** A mode is an id and a `Typing`: `Insert` (VS Code's one mode, Vim's insert mode) types unbound keys, `Ignore` (Vim's normal and visual modes) drops them.
- **Resolution.** `Dispatcher::resolve` documents five rules, applied in order: chord prefix, match, broken chord or operator, typing, unbound. A binding applies when its mode is unset or the state's mode, and its context is unset or one of the `contexts` passed in.
- **Which binding wins.** If the keys so far are a strict prefix of any applying binding, the chord waits (`Pending`), even when another applying binding matches them exactly. Among exact matches, a binding with a mode beats one without, then a binding with a context beats one without, then the later binding wins. So a keybindings file loaded after the defaults overrides them.
- **Broken chords.** A key that neither continues nor completes a pending chord cancels it, and that key is dropped too: `Cancelled`, nothing typed. VS Code does the same. Escape cancels any chord it doesn't complete.
- **Pending operators.** `Action::Operator(c)` holds `c` in the `KeyState`, remembers the mode it was set in, applies the binding's `then_mode`, and returns `Pending`. The next binding with `Action::Run(m)` returns `Run` of `c` with `args.motion = Some(m)`, and the mode goes back to the remembered one, then the motion binding's `then_mode` applies. A chord can be the motion. Any key that matches no binding, including Escape, cancels the operator and returns to the remembered mode. So does a second operator, without its `then_mode`, and so does a binding with no action.
- **Cancelling by hand.** `KeyState::cancel` drops the chord and the operator and goes back to the operator's mode. `helm-ui` calls it on focus loss or a click.
- **Typing.** If nothing matched and nothing was pending, a mode whose `Typing` is `Insert` turns an event with non-empty text and neither Ctrl nor Cmd held into `Run(Invocation::insert(text))`. Alt is allowed, so Option-typed characters work. Everything else is `Unbound`.
- **Switching keymaps (ED-6).** `set_keymap` swaps the keymap and bumps a generation number. Each `KeyState` stores the generation it last saw. The next `resolve` or `sync` on a stale state cancels its chord and operator as `cancel` does, keeps its mode if the new keymap has a mode with that id, and otherwise moves to the new initial mode. Then the key resolves in the new keymap as if nothing had been pending. Nothing pending ever runs across a switch.

### Choices I made, and the ones I rejected

- **Each editor holds its own `KeyState`, and switching is a generation check.** The alternative was a shared, lock-guarded active keymap that every editor reads. That puts a lock on the keypress path (NF-9 asserts on blocking calls from the UI thread) and still needs a way to reset each editor's pending chord. With a generation number, `set_keymap` is one assignment, no list of open editors is needed, and every editor catches up on its next key. `helm-ui` calls `sync` on each editor when it wants the status bar's mode right before the next key.
- **A chord prefix always waits, with no timeout.** Vim resolves `d` versus `dd` with `timeoutlen`. A timeout needs a clock in `helm-editor` and makes tests depend on time. VS Code waits, and Vim's ambiguous cases are better handled by operators (below). `helm-ui` can still call `cancel` after a delay later if wanted.
- **A broken chord drops the breaking key too.** This is VS Code's behaviour. Resolving the breaking key on its own would be friendlier for typos, but then `cmd-k x` would type `x`, which surprises in a different way. Easy to change later if Khakha prefers. Only `a_broken_chord_drops_every_key` and `a_broken_chord_types_nothing` pin it.
- **Mode beats context beats order.** VS Code has one mode and ranks only by order among bindings whose `when` holds. With modes added, a binding written for one mode should beat a binding for every mode, wherever either was added. Otherwise a global binding loaded late, say a user's `j`, silently takes over Vim's normal-mode `j`. A user who wants that writes the binding for normal mode. Contexts are a single name, not VS Code's `when` expressions. Expressions can come with the keybindings file.
- **Operators are state, not chords.** Binding `d w`, `d e`, `d $` and so on as chords multiplies every operator by every motion, and counts (`d3w`) would multiply again. Holding the operator and passing the motion as an argument keeps the keymap linear. A mode-scoped binding handles `dd` (in the tests, `d` in an `operator` mode runs a `vim.line` motion). What an operator does with its motion is T-108.
- **Keymaps may name unknown commands.** The registry only holds commands that run on an `Editor` and a `BufferSet`. Save, open file and the palette belong to `helm-ui`. Checking ids against the registry at `bind` time would reject every keymap that binds them.
- **Shift folds into chars, so Vim's `$` is written `$`.** Using unshifted keys (`shift-4`) breaks on any layout but US. The cost: VS Code's `cmd+shift+[` is written `cmd-{` here. T-107 should list any binding where that matters.
- **Commands are `fn` pointers, not closures.** They are `Copy`, `Send` and `Sync`, and they hold no state that could drift from the editor. A command that needs more context (the clipboard, for ED-1) can get a new field in `CommandContext`.
- **Commands return `()`.** Nothing reads a richer result yet. The tests check the buffer and the selections.

### Dependencies

None. No new dependency, no change to any `Cargo.toml`, and `Cargo.lock` is unchanged.

## Acceptance tests

Five new files, committed before the implementation. 63 tests:

| File | What it proves |
| --- | --- |
| `crates/helm-editor/tests/keys.rs` (12) | Modifiers parse in any order. Every key name, including `space` and `f1` to `f20`. Single chars taken as they are, `cmd--`. Shift folded into ASCII letters, dropped from other chars, kept on named keys. Every `KeyParseError` case, including `Enter`, `Cmd-k`, `f0`, `f21`, a control char, `shift-4` and `shift-space`. `Display` writes the canonical order, and round-trips every key with every modifier combination. Sequences split on any whitespace and reject empty text. The text plain keys type, none for Ctrl, Alt, Cmd and named keys, and `with_text` for Option+A. |
| `crates/helm-editor/tests/commands.rs` (11) | `CommandId` compares as text. `editor_commands::ALL` is the ten documented ids. `with_editor_commands` registers exactly those, sorted, each with a title. `register` accepts and rejects ids by the documented form, and rejects duplicates. Each of the ten commands, run through the registry, does what its `Editor` method does, CRLF included. `insert` without text is `MissingText`. Unknown ids and `EditorError::UnknownBuffer` come back as errors and change nothing. `run` passes `args.motion` through. The `Invocation` constructors. |
| `crates/helm-editor/tests/keymaps.rs` (25) | `Keymap::new` and `bind` errors, order and accessors. The `Binding` builders. A new state is idle in the initial mode. Single keys, two- and three-key chords, a broken chord dropping every key, Escape cancelling, a prefix beating an exact match. A 13-row precedence table over mode, context and order. Contexts scoping chords. Unbound keys typing in an `Insert` mode, and not with Ctrl or Cmd, with empty text, or in an `Ignore` mode. Mode switches with and without a command. Operators: waiting, running with a motion, `d d` through a mode-scoped binding, a chord as the motion, cancelling by an unbound key, Escape, a broken chord or a second operator, returning to the mode they were set in, and a no-action binding dropping one. `KeyState::cancel`. ED-6: a switch drops a pending chord and the next key resolves in the new keymap, reaches three editors' states at once, keeps a mode the new keymap has, drops a pending operator and its mode, does nothing on `sync` without a switch, and a state made after a switch starts in the new initial mode. |
| `crates/helm-editor/tests/dispatch.rs` (11) | Typed keys dispatched into an `Editor` through a VS Code-like sample keymap: typing ASCII, é, 中 and 😀, Enter in a CRLF file, Backspace and Delete, `cmd-z`, the `cmd-k cmd-z` chord and redo, select all matches then typing at three cursors and undoing, adding cursors with keys, a broken chord typing nothing, Option text. A Vim-like sample: `x`, `X`, `u`, `ctrl-r`, `i`, Escape and `o` editing the buffer. Switching keymaps mid-session, and two editors with their own states sharing one dispatcher through a switch. An unknown command errors and edits nothing. `dispatch` returns the resolution. |
| `crates/helm-editor/tests/keymap_properties.rs` (4) | proptest, 256 cases per property, over keymaps built from a 10-keystroke alphabet. (1) In a prefix-free keymap of up to 40 random sequences of one to three keys, typing whole bindings in any order gives `Pending` for every key but the last, then `Run` of that binding's command, with nothing left pending. (2) Up to 9 bindings on one key, each with a random mode and context, the state in a random mode and random contexts held: the winner matches an oracle that ranks mode, then context, then order, or `Unbound` if none applies. (3) Random keymaps with runs, operators, empty bindings and mode switches: after any key history and a switch to a keymap with different mode ids, the state resolves every later key exactly as a new state does, with the same mode, pending keys and operator, whether or not `sync` ran first. (4) Any non-control chars typed one key each into a random text at a random cursor give the text with them inserted there, and the cursor after them. |

Each test carries `#[ignore = "T-106: not implemented yet"]` so CI stays green until the implementation lands. Against the stubs, `cargo test -p helm-editor --no-fail-fast -- --ignored` fails all 63 (12 + 11 + 25 + 11 + 4).

**The rule for the implementer: the only allowed change to these five files is deleting the `#[ignore = "T-106: not implemented yet"]` lines.** Nothing else in them changes. That means no edited assertion, no new `#[ignore]`, and no change to a case count. Khakha checks the diff of these files before adding the `approved-protected` label, which the protected-paths check needs for any edit under `crates/*/tests/`. If a test looks wrong, say so in the PR description instead of changing it.

## Performance budget

NF-1 gives a keypress 5 ms from input to pixel, and resolving the key is the first step. Add a new file, `crates/helm-bench/benches/keymap.rs`, with a `[[bench]] name = "keymap"`, `harness = false` entry in `crates/helm-bench/Cargo.toml`. It holds one criterion benchmark in a group named `keymap`:

| Benchmark id | What one iteration does |
| --- | --- |
| `keymap/resolve_10k_keys_300_bindings` | Resolves 10,000 key events with `Dispatcher::resolve` against a keymap of 300 bindings, using one `KeyState` and the contexts `["editor"]`. |

The keymap has two modes, `insert` (`Typing::Insert`, the initial mode) and `normal` (`Typing::Ignore`), and these 300 bindings, each running a distinct command id:

- 144 single keystrokes: each of `a` to `z` and `0` to `9` with each of `ctrl-`, `alt-`, `ctrl-alt-` and `ctrl-cmd-`.
- 72 chords: `cmd-k` and then `cmd-j`, each followed by each of `a` to `z` and `0` to `9`.
- 36 bindings in `normal` mode: each of `a` to `z` and `0` to `9`, plain.
- 48 bindings in `insert` mode with context `editor`: `enter`, `tab`, `backspace`, `delete`, `escape`, `left`, `right`, `up`, `down`, `home`, `end` and `pageup`, each plain, with `shift-`, with `alt-` and with `cmd-`.

The 10,000 events are a fixed list of 100 events repeated 100 times: 70 plain chars that type (cycling through `a` to `z`, `0` to `9` and space), 10 bound single keystrokes, 5 chords (10 events), and 10 bound named keys. Build the keymap, the dispatcher, the state and the event list once, outside the timed code. Every chord in the list completes, so each iteration ends with nothing pending and starts from the same state. Pass each resolution through `black_box`.

Budget: the default 5% from `budgets.toml`. Add no per-benchmark override. The base commit has no `keymap` benchmark, so `bench-compare` reports it as head-only in this task's PR and gates it from the next PR on. No absolute limit is gated, but a resolution should take well under a microsecond. Index the bindings (by mode, and by first keystroke or as a trie) rather than scanning all 300 per key, and say in the PR what the benchmark measured.

As in T-101 to T-105, the implementation PR adds the benchmark file. This spec PR doesn't.

## Out of scope

- The VS Code bindings: T-107. The tests' sample keymaps hold only a few bindings each.
- Vim's real modes, motions, operators and what an operator does with its motion: T-108. Counts (`3dw`), which T-108 adds to `KeyState` and `CommandArgs`. Text objects, registers, `.` repeat and `:w` `:q` `:e`: T-109.
- Cursor movement commands. `Editor` has none yet, so the registry has none. T-107 and T-108 add the movement commands their bindings need.
- An operator that ends in a different mode, such as Vim's `c` ending in insert mode: T-108, which may let a command ask for a mode switch.
- Chord timeouts, like Vim's `timeoutlen`.
- Loading keymaps and the keybindings file from TOML, reloading them on save, and the settings toggle between VS Code and Vim (ST-1): later tasks. This task gives them `Dispatcher::set_keymap`.
- `when`-clause expressions (`&&`, `!`, comparisons). A binding has at most one context name.
- Converting GPUI key events to `KeyEvent`, and IME composition (ED-8): `helm-ui`, Phase 2. Composed text runs `Invocation::insert` directly.
- Physical key codes and layout-independent bindings.
- The command palette UI and showing each command's binding (ST-3): later. `CommandRegistry::commands` and `Keymap::bindings` are what it will read.
- Commands outside the editor, such as save or open: `helm-ui`.
