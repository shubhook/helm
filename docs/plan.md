# Helm implementation plan

Final plan, approved 13 October 2026. "You" in this file means Khakha, Helm's maintainer, who approves specs and reviews every PR. Agents write the code.

Related files:

- [prd.md](prd.md) says what v1 must do, requirement by requirement. Every task cites the PRD IDs it implements.
- [research.md](research.md) is the background research the architecture follows.

Helm gets built in eight phases, from a headless text core up to an agent panel. v1 is done when you use it instead of VS Code for a week.

## Decisions

All eleven decisions are made. An agent never changes one; propose a change in the PR description instead.

| Decision | Choice | What it means |
| --- | --- | --- |
| First OS | macOS first, Linux second | v1 ships on macOS. Tests for the headless crates run on macOS and Linux CI from Phase 0, so the Linux port after v1 only has to cover the window, fonts and input. |
| UI toolkit | GPUI | The `gpui` crate is Apache-2.0 and published on crates.io, at version 0.2.2 on 13 October 2026 ([Cargo.toml](https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml)). Helm depends on it without taking on Zed's GPL. Pin the version. |
| License | MIT OR Apache-2.0 | The usual Rust dual license, so users may pick either. Agents copy no code from Zed's editor (GPL) or Helix (MPL-2.0), and `cargo-deny` blocks copyleft dependencies. |
| Repo visibility | Public from the start | Nothing in v1 needs to stay private. |
| Agent benchmarking | Not in v1 | A future add-on of your own design. The plan has no benchmark runs, run logs or held-out tasks. |
| Screen reader support | Not in v1 | Listed in the non-goals. |
| Editing model | Both | VS Code-style keys and Vim keys both ship in v1, switchable in settings. The keymap supports modes from Phase 1. |
| Text buffer | ropey | Helix ships it. It sits behind Helm's own buffer trait, so a later swap touches one crate. |
| Perf hardware | No dedicated machine in v1 | The speed budgets stay in CI, because a fast editor is the point of Helm. They run on hosted runners, and each PR is compared with its base commit on the same runner, since shared runners are too noisy for fixed millisecond limits or for comparing against earlier runs. |
| v1 scope | As written in prd.md | Phase 7 and the non-goals below define v1. |
| Agent integration | ACP only | Claude Code, Codex and Gemini CLI already speak it. |

## Goals and non-goals

v1 succeeds when two things are true.

1. You do a week of real TypeScript, JavaScript, Go and Rust work in Helm without opening VS Code or Cursor.
2. Helm meets the performance budgets in prd.md (NF-1 to NF-7), and CI fails any PR that breaks one. The budgets are targets, not measurements, so the first real numbers come from Phase 2.

Out of scope for v1:

- A plugin or extension system. The four languages are built in. If plugins come later, they use WASM, as Zed and Lapce do.
- A debugger (DAP), remote development, real-time collaboration, screen reader support and agent benchmarking.
- Linux builds. Linux is the first port after v1.
- A settings UI. Settings and keybindings live in TOML files.
- Languages beyond TS, JS, Go and Rust, other than tree-sitter highlighting for JSON, TOML, YAML and Markdown.
- Model calls made by Helm itself. Agents run in their own processes over ACP.

## Architecture

Helm is one Cargo workspace of small crates, and only `helm-ui` knows a window exists. That rule lets agents build and test most of the editor without a GPU, and it keeps GPUI swappable.

Each layer depends only on the layers below it:

```
helm            the binary: startup, settings file, wiring the crates together
  │
helm-ui         GPUI views: editor, tabs and splits, file tree, finder,
  │             terminal and agent panels
  │
- - - - - - - - no window below this line: everything below is tested headless - - - -
  │
helm-editor     cursors, multi-cursor, selections, commands, keymap (VS Code and Vim modes)
  │
services, run on worker threads against buffer snapshots:
  helm-syntax    tree-sitter: highlight, fold, indent, outline
  helm-lsp       LSP client: lazy start, one server per root, idle shutdown
  helm-project   files, watching, search, git, large files
  helm-terminal  PTY and emulator, capped scrollback
  helm-agent     ACP client, edit transactions
  │
helm-text       rope behind a Helm trait (ropey), transactions, undo, snapshots,
                UTF-16 offsets
```

A tenth crate, `helm-bench`, holds the perf benchmarks and is not shipped.

Language servers and agents run as child processes. Workers send results back tagged with the buffer version they read, and the UI drops stale ones. Most tasks should touch one crate, which keeps each agent's context and each review small.

## Roadmap

The phases build the editor from the bottom of the architecture up, so each one can be tested before anything depends on it. There are no dates. A phase starts only when the previous phase's gate passes.

| Phase | Scope | Gate to pass |
| --- | --- | --- |
| 0. Foundations | Workspace, CI on macOS and Linux, license check, `AGENTS.md`, task template, perf benchmark harness | CI runs tests, clippy and benches, and fails a deliberately bad PR |
| 1. Headless core | Rope, transactions, undo, multi-cursor, commands, keymap with VS Code and Vim modes. No window yet. | Property tests pass; rope microbenchmark baseline recorded |
| 2. Window and rendering | GPUI window, text shaping, scrolling, input and IME; open, edit and save one file | Keypress, frame-time, startup and memory budgets met on a 10k-line file |
| 3. Syntax | tree-sitter for TS, TSX, JS, Go, Rust and config files: highlight, indent, fold, outline | Re-parsing a 10 MB file never stalls typing |
| 4. Project | Open folder, file tree, fuzzy finder, project search, tabs, splits, file watching, large files | A 100 MB file opens in under 1 s; outside writes keep unsaved edits |
| 5. Language servers | TS 7, gopls, rust-analyzer: diagnostics, completion, hover, go to definition, rename, format, code actions | You edit a real repo in each language; server memory is tracked |
| 6. Terminal and git | Integrated terminal, git gutter, status and diff view | You build, test and commit without leaving Helm |
| 7. Agents | ACP client, agent panel, multi-file diff review, apply as one undoable transaction | v1: a week of daily use instead of VS Code |

Phase 1 stays headless on purpose: the rope and transaction code is where subtle bugs live, and it is easiest to test before rendering exists. Agent support comes last because it needs diffs, a terminal and diagnostics from the earlier phases. After v1, the likely next items are the Linux port, WASM plugins and a debugger.

## How agents do the work

Every change starts as a task spec you approve and ends as a PR that passes CI and your review. Your review time is the scarcest resource in this project, so CI checks everything a machine can check and you review what it can't.

**Task specs.** Each task is a file in the repo, `tasks/NNN-short-name.md`, with these parts:

- Goal, in two or three sentences, and the PRD requirement IDs it implements.
- The crate or crates it may touch, and the public interface it adds or changes.
- Acceptance tests, committed before any agent starts. The tests are the grader.
- The performance budget it must hold, if any.
- What is out of scope.

**The loop for one task.**

1. An agent drafts the spec and its tests. You approve or edit them, and they merge on their own.
2. An agent branches from that commit in a clean checkout.
3. The agent opens a PR. CI runs `cargo fmt --check`, `clippy` with warnings as errors, the tests, the perf benchmarks against the PR's base commit, and `cargo-deny` for licenses.
4. CI also fails the PR if it changes the spec's test files or a budget threshold. Without this check, an agent can pass by weakening the grader.
5. You review interfaces, `unsafe`, threading and anything the spec calls out. Then merge.

**Shared instructions.** One `AGENTS.md` at the repo root holds the conventions, the crate map and the commands to run tests and benchmarks. `CLAUDE.md` points to it. Every agent reads the same file, so switching agents between tasks needs no extra setup.

**In T3 Code.** You orchestrate the agents with [T3 Code](https://github.com/pingdotgg/t3code), which drives Claude Code, Codex and other agent CLIs, gives every thread its own branch, and opens the PR from a button. The loop maps onto it like this:

1. One task is one T3 Code thread. Start it with the task file's path and nothing else, so the spec in the repo is the whole brief.
2. The thread's branch becomes the task's PR. Use a draft PR until CI is green.
3. Pick the agent per thread. Models can be switched mid-thread; note in the PR when that happened.
4. Merge in task order. A task whose "After" task is not merged yet waits, even if its thread finished.
5. Run threads in parallel only when their tasks touch different crates. Two threads in one crate will conflict.

T3 Code's own README calls it "very very early" and says to expect bugs. Specs, tests and CI all live in the repo, so if T3 Code gets in the way, the same loop runs from Claude Code or Codex directly.

## First tasks

These are the tasks for Phases 0 and 1, in order. Later phases get their task lists when Phase 1's gate passes, because what agents learn in Phase 1 will change how the later tasks are cut.

| ID | Task | Crate | PRD | After |
| --- | --- | --- | --- | --- |
| T-001 | Cargo workspace with the ten crates as empty libraries, the `helm` binary and a pinned toolchain | all | NF-11 | none |
| T-002 | CI on macOS and Linux: fmt, clippy with warnings as errors, tests, and `cargo-deny` with a license allowlist | repo | NF-12 | T-001 |
| T-003 | Protected-paths check: CI fails a PR that edits a spec's tests or a budget threshold unless you add an approval label | repo | none | T-002 |
| T-004 | `AGENTS.md`, a `CLAUDE.md` that points to it, `tasks/TEMPLATE.md` and a PR template that asks for PRD IDs | repo | none | T-001 |
| T-005 | Benchmark harness with `criterion` and a CI job that compares each PR with its base commit | helm-bench | NF-7 | T-002 |
| T-101 | Buffer trait over ropey, with byte, char, line and UTF-16 offset conversions | helm-text | NF-7 | T-005 |
| T-102 | Transactions with undo and redo, including one transaction across several buffers | helm-text | ED-2 | T-101 |
| T-103 | Cheap snapshots tagged with a buffer version, and a stress test of concurrent readers | helm-text | NF-9 | T-101 |
| T-104 | Line ending and BOM detection, preserved on save | helm-text | ED-7 | T-101 |
| T-105 | Selections and multiple cursors, with property tests | helm-editor | ED-3 | T-102 |
| T-106 | Command registry and a keymap engine with modes | helm-editor | ED-4, ED-6 | T-105 |
| T-107 | VS Code bindings for every editing command so far | helm-editor | ED-4 | T-106 |
| T-108 | Vim normal, insert and visual modes, with motions, operators and counts | helm-editor | ED-5 | T-106 |
| T-109 | Vim text objects, registers, `.` repeat and the `:w` `:q` `:e` commands | helm-editor | ED-5 | T-108 |

T-001 to T-004 have no product behavior to test, so their acceptance is that CI runs and fails a deliberately broken PR. From T-005 on, every task starts with its tests.

## Risks

- **Scope is the main risk.** A daily-driver editor needs a file tree, search, terminal, git, multi-cursor, IME and more. The phase gates stop the plan from growing before v1 works.
- **Review time limits the pace.** Agents can open PRs faster than one person reads them. Small tasks confined to one crate keep each review short. This is an inference, not a measurement.
- **Threading bugs pass tests.** Snapshot and version-tagging code can be wrong in ways unit tests miss. Every spec that touches the buffer or workers asks for property tests and a stress test, and you read that code yourself.
- **GPUI moves with Zed.** Pin a version and upgrade it as its own task. Keep GPUI types inside `helm-ui` so an upgrade, or a switch to Floem, touches one crate.
- **Language servers set the memory floor.** rust-analyzer alone has user reports of 12 to 16 GB on large workspaces. Helm's budgets measure Helm's own process; server memory is tracked separately and is not a CI gate.
- **Agents may reproduce copyleft code.** Zed (GPL) and Helix (MPL-2.0) are in their training data. The no-copy rule and `cargo-deny` cover dependencies, but not pasted snippets. Agents name any outside source they followed in the PR description.
