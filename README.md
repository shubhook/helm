# Helm

Helm is a native Rust code editor for macOS. It aims to open in under 200 ms, edit TypeScript, JavaScript, Go and Rust with full language-server support, and host coding agents over the Agent Client Protocol. It has no Electron and no Node runtime.

What v1 must do is in [docs/prd.md](docs/prd.md). How it gets built is in [docs/plan.md](docs/plan.md).

## Status

Phase 1 (headless core) is in progress. There is no window yet, so there is nothing to run as an app.

- T-101 Buffer trait and offset conversions — done
- T-102 Transactions with undo and redo — done
- T-103 Snapshots and concurrent readers — in progress
- T-104 Line endings and BOM preservation — done
- T-105 Selections and multiple cursors — in progress
- T-106 Command registry and keymap engine — not started
- T-107 VS Code bindings — not started
- T-108 Vim modes, motions, operators and counts — not started
- T-109 Vim text objects, registers, repeat and commands — not started

## Crates

- `helm` — binary startup, settings file and crate wiring; currently a placeholder binary.
- `helm-ui` — GPUI editor views, tabs and splits, file tree, finder, terminal and agent panels. **Empty.**
- `helm-editor` — cursors, selections, editing commands and VS Code/Vim keymaps.
- `helm-syntax` — tree-sitter highlighting, folding, indentation and outline. **Empty.**
- `helm-lsp` — language-server client with lazy start, one server per root and idle shutdown. **Empty.**
- `helm-project` — files, watching, search, git and large files. **Empty.**
- `helm-terminal` — PTY and terminal emulator with capped scrollback. **Empty.**
- `helm-agent` — ACP client and edit transactions. **Empty.**
- `helm-text` — rope-backed text buffer, transactions, undo, snapshots and UTF-16 offsets.
- `helm-bench` — performance benchmarks; not shipped.

## Building and testing

Rustup reads the pinned toolchain and installs `rustfmt` and `clippy` from [rust-toolchain.toml](rust-toolchain.toml). CI runs:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check --all-features --locked
cargo bench -p helm-bench
```

The PR bench job runs locked base and head benchmarks with `--warm-up-time 3 --measurement-time 10`, then compares them against the budgets. See [AGENTS.md](AGENTS.md) for the full local workflow and command notes.

## How work happens

Each task starts with a spec PR containing stubs and ignored acceptance tests; after it merges, an implementation PR fills in the stubs and removes the test ignores. The protected-paths check requires the maintainer-only `approved-protected` label for protected changes, including edits to existing acceptance tests and benchmark budgets. The bench job compares each PR against its base commit on the same runner. See the [task template](tasks/TEMPLATE.md) and [plan: How agents do the work](docs/plan.md#how-agents-do-the-work).

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
