# Helm v1 PRD

13 October 2026. Helm v1 is a native Rust code editor for macOS that opens in under 200 ms, edits TypeScript, JavaScript, Go and Rust with full language-server support, and hosts coding agents over ACP. It is done when its one user works in it for a week instead of VS Code.

How it gets built is in [plan.md](plan.md); this file says what it must do. The research behind both is in [research.md](research.md).

## Problem and user

VS Code and its forks, Cursor included, run their UI on Electron and their extensions on Node. They are slow to start, use hundreds of megabytes before a project opens, and were not designed around agents editing many files at once.

The v1 user is Khakha: a developer on macOS who writes TS, JS, Go and Rust, works alongside coding agents every day, and wants both VS Code-style keys and Vim keys. Every requirement below serves that person. Anything only other users would need waits for a later version.

**How to read this file.** Each requirement has an ID such as `ED-3`. Task specs in the repo cite the IDs they implement, and the Acceptance column is what that task's tests must prove. "Must" items block release; "Should" items ship if they are ready and are cut if they are not. Only Khakha changes this file.

## Functional requirements

### Text editing (ED)

| ID | Priority | Requirement | Acceptance |
| --- | --- | --- | --- |
| ED-1 | Must | Insert, delete, cut, copy and paste with the macOS clipboard | Round trip of a 1 MB paste preserves bytes exactly |
| ED-2 | Must | Undo and redo by transaction; an agent's multi-file edit is one transaction | One undo reverts every file the transaction touched |
| ED-3 | Must | Multiple cursors and selections: add next match, add above and below, select all matches | Property tests: N cursors typing produce the same text as N single edits |
| ED-4 | Must | VS Code-style keymap as a mode | The default VS Code bindings for the features in this PRD work as documented |
| ED-5 | Must | Vim keymap as a mode: normal, insert, visual and visual-line; motions, operators, counts, text objects, registers, `.` repeat, `:w` `:q` `:e` | A Vim test suite of keystroke sequences and expected buffers passes |
| ED-6 | Must | Switch between ED-4 and ED-5 in settings without restarting | The toggle applies to open editors |
| ED-7 | Must | Line endings (LF and CRLF) and UTF-8 with or without BOM are preserved on save | An open-save round trip leaves file bytes unchanged |
| ED-8 | Must | macOS text input: IME composition, dead keys, emoji picker | Manual checklist for Japanese IME, dead-key accents and emoji |
| ED-9 | Should | Structural selection: expand and shrink by syntax node | Works in all four languages |

### Files and project (FP)

| ID | Priority | Requirement | Acceptance |
| --- | --- | --- | --- |
| FP-1 | Must | Open a folder from the CLI (`helm .`) or a dialog | The window opens on that folder |
| FP-2 | Must | File tree that respects `.gitignore`, with create, rename, delete and reveal | Ignored files are hidden by default |
| FP-3 | Must | Fuzzy file finder | Results for a 100k-file repo appear while typing |
| FP-4 | Must | Project search with regex, case and whole-word options, results streamed | Search never blocks typing |
| FP-5 | Must | Tabs and split panes, horizontal and vertical | Each pane keeps its own tab list |
| FP-6 | Must | Files changed on disk reload; a buffer with unsaved edits shows a conflict prompt instead | Test: an outside write to a clean buffer reloads it, to a dirty one prompts |
| FP-7 | Must | Large files open read-optimized: syntax off above a size limit, lazy loading | See NF-5 |
| FP-8 | Must | Session restore: open folder, tabs and cursor positions after restart | Quit and relaunch restores state |
| FP-9 | Should | Hot exit: unsaved buffers survive a crash or quit | Kill the process; edits are back on relaunch |

### Syntax (SY)

| ID | Priority | Requirement | Acceptance |
| --- | --- | --- | --- |
| SY-1 | Must | tree-sitter highlighting for TS, TSX, JS, JSX, Go and Rust | Matches the grammars' own highlight queries |
| SY-2 | Must | Highlighting for JSON, TOML, YAML and Markdown | Same |
| SY-3 | Must | Auto-indent, bracket matching and auto-close | Per-language tests |
| SY-4 | Must | Code folding and a document outline | The outline lists top-level symbols |
| SY-5 | Must | Comment toggling | Line and block comments per language |

### Language servers (LS)

| ID | Priority | Requirement | Acceptance |
| --- | --- | --- | --- |
| LS-1 | Must | Spawn TypeScript 7 for TS and JS, gopls for Go, rust-analyzer for Rust; lazy start, one per workspace root, stop after idle | No server starts until a file of its language opens |
| LS-2 | Must | Diagnostics inline and in a problems list | Errors appear without saving |
| LS-3 | Must | Completion with snippets and auto-imports | Works in all four languages |
| LS-4 | Must | Hover, signature help, go to definition, find references, go to symbol | Same |
| LS-5 | Must | Rename and code actions, applied as one transaction | One undo reverts a rename across files |
| LS-6 | Must | Format on save, per language, on by default | Uses each server's formatter |
| LS-7 | Must | Server settings exposed in the settings file, including rust-analyzer's cache priming, proc macros and `checkOnSave` | Changing them restarts the server |
| LS-8 | Should | Inlay hints | Toggle in settings |

### Terminal and git (TG)

| ID | Priority | Requirement | Acceptance |
| --- | --- | --- | --- |
| TG-1 | Must | Integrated terminal with tabs, using the user's login shell | `vim`, `htop` and `git log` render correctly |
| TG-2 | Must | Terminal scrollback capped and rendered off the UI thread | 1 GB of output does not stall typing in the editor |
| TG-3 | Must | Git gutter: added, changed and deleted lines | Updates on save and on outside changes |
| TG-4 | Must | Git status in the file tree and a diff view against HEAD | The diff matches `git diff` |
| TG-5 | Should | Stage, unstage and commit from the editor | The commit matches the CLI result |

### Settings (ST)

| ID | Priority | Requirement | Acceptance |
| --- | --- | --- | --- |
| ST-1 | Must | One TOML settings file and one keybindings file, reloaded on save | Invalid TOML shows an error and keeps the last good settings |
| ST-2 | Must | Light and dark themes that follow macOS appearance | Switches live |
| ST-3 | Must | Command palette listing every command with its key binding | Every command in this PRD is in it |

## Non-functional requirements

The numbers are targets from the research, not measurements of anything yet. Phase 2 produces the first real numbers, and a target that proves wrong is changed here by Khakha, never by an agent. Every NF item with a number is checked in CI against a rolling baseline on hosted runners, and Helm's own process is measured apart from its language servers.

| ID | Requirement | Target | How CI measures it |
| --- | --- | --- | --- |
| NF-1 | Keypress to pixel, editor's share | under 5 ms average | Instrumented input-to-present timing in a scripted typing run |
| NF-2 | Frame time scrolling a 10k-line file | p99 under 8.33 ms (120 Hz) | Frame timings from the in-app profiler |
| NF-3 | Cold start to first frame, empty window | under 200 ms | `hyperfine` with an exit-after-first-frame flag |
| NF-4 | Memory, one file open, no servers | under 100 MB | `footprint` on macOS, `/proc` on Linux CI |
| NF-5 | Open a 100 MB file and jump to the end | under 1 s, UI never blocks | Scripted run |
| NF-6 | Typing while a 10 MB file re-parses | NF-1 still holds | Scripted run during re-parse |
| NF-7 | Rope edits and offset lookups | no regression over 5% | `criterion` microbenchmarks |

Other requirements:

- **NF-8 No data loss.** Save writes to a temp file and renames it. No crash path discards a user's unsaved edit without FP-9's backup.
- **NF-9 The UI thread never waits.** Parsing, LSP traffic, search, git, file IO and agent traffic run on workers against snapshots. A debug build asserts on blocking calls from the UI thread.
- **NF-10 No Node runtime.** Helm ships without Node, and none of its four language servers needs one.
- **NF-11 Platforms.** v1 runs on macOS 14 or later on Apple silicon. Headless crates also pass their tests on Linux. Intel Macs are untested.
- **NF-12 License hygiene.** MIT OR Apache-2.0. `cargo-deny` rejects copyleft dependencies, and no code is copied from GPL or MPL editors.
- **NF-13 No telemetry.** Helm sends nothing over the network except what language servers and agents do on their own.

## Agent integration (AG)

Helm talks to agents only through the [Agent Client Protocol](https://agentclientprotocol.com/overview/introduction). Each agent runs as its own child process, so its model calls and memory never live in Helm. This is separate from T3 Code, which is used to build Helm; these requirements are about using agents inside Helm while editing.

| ID | Priority | Requirement | Acceptance |
| --- | --- | --- | --- |
| AG-1 | Must | Start, stop and switch ACP agents configured in settings; Claude Code and Codex at minimum | Both complete a prompt end to end |
| AG-2 | Must | Agent panel that streams messages, tool calls and plans | Streaming never stalls typing (NF-9) |
| AG-3 | Must | Agent edits arrive as a per-file diff the user accepts or rejects, file by file or all at once | Rejecting leaves files untouched |
| AG-4 | Must | Accepted edits apply as one transaction (ED-2) | One undo reverts the whole agent change |
| AG-5 | Must | Agent file reads see unsaved buffer contents, and agent writes to an open dirty buffer go through FP-6's conflict path | No silent overwrite of unsaved edits |
| AG-6 | Must | Permission prompts for agent terminal commands and file writes, with an allow-for-session option | Denied actions do not run |
| AG-7 | Must | Agents read the same diagnostics the user sees (LS-2) | An agent can list current errors without running a build |
| AG-8 | Must | Agent terminals run in Helm's terminal (TG-1, TG-2) and are visible to the user | Output shows live in a terminal tab |
| AG-9 | Should | Mention a file, selection or symbol in a prompt | The agent receives its content |
| AG-10 | Should | Session history kept per project and reopenable | Relaunch shows past sessions |

## Out of scope for v1

- Plugins or extensions of any kind
- Linux and Windows builds (Linux is the first port after v1)
- Debugger, remote development, real-time collaboration
- Screen reader support
- Agent benchmarking (a future add-on with its own design)
- A settings UI, a theme marketplace, or languages beyond those in SY-1 and SY-2
- Model calls made by Helm itself

## Release criteria

v1 ships when all of these hold:

- [ ] Every Must requirement is merged with its acceptance tests passing.
- [ ] NF-1 to NF-7 pass in CI on the release commit.
- [ ] Khakha has used Helm for five working days on real TS, JS, Go and Rust work without opening VS Code or Cursor.
- [ ] Every bug found during those days that loses data or blocks work is fixed.
- [ ] A release build installs on Khakha's Mac from a GitHub release or `cargo install`. Signing and notarization wait until someone else uses Helm.
