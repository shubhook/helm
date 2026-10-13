# AGENTS.md

Read this before every task. It applies to every agent: Claude Code, Codex or any other.

## What Helm is

Helm is a native Rust code editor for macOS that opens in under 200 ms, edits TypeScript, JavaScript, Go and Rust with full language-server support, and hosts coding agents over ACP. What v1 must do is in [docs/prd.md](docs/prd.md), and how it gets built is in [docs/plan.md](docs/plan.md).

## Crate map

| Crate | Owns |
| --- | --- |
| `helm` | The binary: startup, the settings file, wiring the crates together |
| `helm-ui` | GPUI views: editor, tabs and splits, file tree, finder, terminal and agent panels |
| `helm-editor` | Cursors, multi-cursor, selections, commands, keymap (VS Code and Vim modes) |
| `helm-syntax` | tree-sitter: highlight, fold, indent, outline |
| `helm-lsp` | LSP client: lazy start, one server per root, idle shutdown |
| `helm-project` | Files, watching, search, git, large files |
| `helm-terminal` | PTY and emulator, capped scrollback |
| `helm-agent` | ACP client, edit transactions |
| `helm-text` | Rope behind a Helm trait (ropey), transactions, undo, snapshots, UTF-16 offsets |
| `helm-bench` | Perf benchmarks. Not shipped. |

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

Only `helm-ui` may use GPUI or know that a window exists. Nothing below `helm-ui` depends on it, and no GPUI type appears in another crate's public interface.

## How a task works

1. Start from the task file in `tasks/` (format in [tasks/TEMPLATE.md](tasks/TEMPLATE.md)). It is the whole brief.
2. Touch only the crates the task file names.
3. Don't change the task's acceptance tests or any perf budget threshold. If one looks wrong, say so in the PR description.
4. Open a draft PR using [the PR template](.github/pull_request_template.md). Mark it ready only when CI is green.

## Rules

- Never change a decision in the Decisions table of `docs/plan.md`, or anything under `docs/`. Propose the change in the PR description instead.
- Never copy code from Zed's editor crates (GPL) or from Helix (MPL-2.0). The `gpui` crate itself is Apache-2.0 and is fine to depend on.
- Name every outside source you followed (docs, articles, other repos) in the PR description.
- Add no new dependency without saying why in the PR description.
- Every `unsafe` block gets a `// SAFETY:` comment explaining why it is sound.
- Never add or remove the `approved-protected` label. Only Khakha does. CI fails a PR that changes a protected path (listed in `.github/protected-paths.txt`) unless the PR has that label.
- The UI thread never blocks (NF-9). Parsing, LSP traffic, search, git, file IO and agent traffic run on workers against snapshots.

## Commands

Run all of these before marking a PR ready. CI runs the same ones.

```sh
cargo build --workspace
cargo test --workspace
cargo fmt --all --check        # cargo fmt has no --workspace flag; --all is its equivalent
cargo clippy --workspace --all-targets -- -D warnings
# benchmarks: T-005 adds the command here
```

## Commit messages

One short subject line in the style of the existing history (`build: add cargo workspace with ten empty crates`). An optional body says what changed and why. The last line names the task and PRD IDs:

```
build: add cargo workspace with ten empty crates

Refs: T-001, NF-11, NF-12
```

Use `Refs: T-004` alone when the task has no PRD IDs.
