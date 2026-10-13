# Helm: making a fast, light Rust editor

Research notes, 13 October 2026. This is background for [plan.md](plan.md) and [prd.md](prd.md). Where this file and those two disagree, they win: decisions made after this research (GPUI, ropey, no agent benchmarking in v1, macOS first) are recorded in the plan.

## Summary

Helm can be much lighter than VS Code if it has no web runtime, never blocks the UI thread, and ships with no general extension host. The language servers, not the editor, will dominate memory, so those need the most care.

- **Render natively on the GPU.** Zed's GPUI targets 120 fps, which leaves 8.33 ms per frame. Reuse GPUI or Floem rather than writing a UI toolkit.
- **Store text in a rope with cheap snapshots** (ropey, or Zed's SumTree). Background work then reads a snapshot while the user keeps typing.
- **Keep the UI thread to input, layout and paint.** Parsing, LSP traffic, search, git and file IO run on worker threads against snapshots.
- **Use tree-sitter for highlighting, folding and structural selection,** and LSP for everything semantic.
- **Pick the native servers:** TypeScript 7 (Go port, LSP-based) for TS and JS, gopls, and rust-analyzer. rust-analyzer is the biggest memory risk, with user reports of 12 to 16 GB on large workspaces.
- **Skip a plugin system in v1.** Build the four languages in. If plugins come later, use WASM, as Zed and Lapce do.
- **Speak the Agent Client Protocol (ACP)** for agent integration. Claude Code, Codex CLI and Gemini CLI already speak it, so Helm gets them without bespoke integrations.
- **Set numeric budgets on day one** and fail CI when a change breaks one. Agents optimize for what is measured.

## Why VS Code is slow and heavy

VS Code runs its UI in Electron (Chromium plus Node) and its extensions in separate Node.js extension host processes. Microsoft's docs say the host exists to stop extensions from hurting startup or UI speed. It does not make them cheap, because every extension still runs on a JS runtime ([VS Code docs](https://code.visualstudio.com/api/advanced-topics/extension-host)).

The measured gap is old but large. Joe Allen's comparison on Ubuntu 14.04 found VS Code at 339,512 KiB RSS for a tiny C file, against 5,336 KiB for vim and 45,800 KiB for Sublime. Opening a 5.8 MB XML file took VS Code 22 s and Sublime 1 s ([editor-perf](https://github.com/jhallen/joes-sandbox/tree/master/editor-perf)). Today's VS Code will differ, and no recent independent measurement was found. Zed's own comparison page says VS Code "can use 3-4GB+ RAM" but gives no method ([Zed](https://zed.dev/compare/vscode)).

Inference, not a measurement: in daily use, much of the weight people feel comes from extensions and language servers. A native editor that runs the same language servers will still use gigabytes on a big Rust project.

## What Zed, Helix and Lapce do

All three are Rust, use a rope, use tree-sitter for syntax and talk to language servers over LSP. They differ mostly in how they draw and how they extend.

| Editor | UI and rendering | Text storage | Extensions | Source |
| --- | --- | --- | --- | --- |
| Zed | GPUI, its own GPU UI framework. One shader per primitive (rects, shadows, glyphs), glyph atlas, one instanced draw call per text run. Targets 120 fps. | SumTree-backed rope, 128-char chunks, `Arc` nodes so a snapshot is a refcount bump | WASM extensions | [GPUI post](https://zed.dev/blog/videogame), [rope post](https://zed.dev/blog/zed-decoded-rope-sumtree) |
| Helix | Terminal UI. A compositor draws component layers onto a surface each cycle. | ropey; functional core modeled on CodeMirror 6; edits are invertible transactions | None in mainline per its architecture doc | [architecture.md](https://github.com/helix-editor/helix/blob/master/docs/architecture.md) |
| Lapce | Floem UI toolkit, rendered with wgpu | Rope from Xi-editor's design | WASI plugins (Rust, C, AssemblyScript) | [Lapce repo](https://github.com/lapce/lapce) |

Zed's own claims are under 1 s startup, under 10 ms typing latency and about 600 MB RAM. Zed publishes no method for those numbers, so treat them as marketing until measured ([Zed](https://zed.dev/compare/vscode)).

The lesson from Zed's rope post is that concurrency drove the design. Tree-sitter re-parses a snapshot on a background thread, and a snapshot costs almost nothing because rope nodes are shared. Max Brunsfeld said that benefit "matters more than any of that other stuff."

## Recommended architecture

One process for the editor, one child process per language server and per agent, and a UI thread that only handles input, layout and paint.

```
Helm process
├── UI thread: input, layout, paint (never blocks)
├── Worker threads: tree-sitter parsing, LSP traffic, search, git, file IO
│     read rope snapshots, return results tagged with the buffer version
├── child process per language server (TS 7, gopls, rust-analyzer), JSON-RPC over stdio
└── child process per agent (Claude Code, Codex, ...), ACP over stdio
```

Workers read rope snapshots and send results back to the UI thread tagged with the buffer version, so stale results are dropped and typing never waits.

**Rendering.** Draw with the GPU and cache shaped glyphs in an atlas, as GPUI does. Building a UI toolkit is a project as large as the editor, so reuse one. GPUI is proven on exactly this workload. Floem is the other Rust option built for an editor. Immediate-mode toolkits like egui are easier to start with, but no measurements of them on large buffers were found.

**Text buffer.** A rope with O(log n) offset, line and UTF-16 conversions. LSP positions default to UTF-16, so the buffer should convert cheaply, as Zed's `TextSummary` does. ropey is a standalone crate that Helix ships.

**Threads.** Every background job takes a snapshot and returns results tagged with the buffer version they came from. Stale results are dropped. This is what keeps typing latency flat while a 10 MB file re-parses.

**Syntax.** Tree-sitter parses incrementally, tolerates syntax errors, and its runtime is dependency-free C11 ([tree-sitter](https://tree-sitter.github.io/tree-sitter/)). Use it for highlighting, folding, indentation, outline and structural selection. Only run highlighting queries on the visible range plus a margin.

**Language servers.** Start a server lazily when the first file of that language opens, share one per workspace root, and shut it down after it sits idle. Debounce `didChange` and cancel stale requests.

**Files and search.** Watch files with the OS notifier (the `notify` crate), respect `.gitignore`, and search with ripgrep's libraries (`grep-searcher`, `ignore`) on a thread pool.

**Large files.** Load lazily, turn off tree-sitter above a size limit, and never compute the longest line on the UI thread. Joe Allen's tests show that most editors fail or hang on a 3 GB file, so decide the limit up front.

## Language support for TS, JS, Go and Rust

Helm should spawn the official language servers and own nothing semantic itself. The servers are where the memory goes.

| Language | Server | What is known about its cost | Source |
| --- | --- | --- | --- |
| TypeScript, JavaScript | TypeScript 7 (native Go port), built on LSP, released July 2026 | Opening a file with an error in the VS Code repo went from about 17.5 s to under 1.3 s. Build memory on the VS Code repo fell from 5.2 GB to 4.2 GB. | [TS 7 announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/) |
| Go | gopls | v0.12 cut memory by about 75% on average across 28 repos | [Go blog](https://go.dev/blog/gopls-scalability) |
| Rust | rust-analyzer | One user reported 12 GB idle and up to 16 GB while indexing the Zed repo. A maintainer said memory use has grown and will be looked at after the trait solver migration. | [discussion #20871](https://github.com/rust-lang/rust-analyzer/discussions/20871) |

TS 7 matters most for the "VS Code is slow" complaint. The old TS server ran on Node, and a guess is that it was a large part of what made TS projects feel heavy in any editor. With TS 7, Helm does not need a Node runtime for TS and JS intelligence at all. JS-only projects use the same server.

For rust-analyzer, expose its settings for cache priming, proc-macro expansion and `checkOnSave`. Those trade features for memory and CPU. How much each one saves has not been measured.

## Agentic coding without the bulk

Implement the Agent Client Protocol and let agents run as child processes. ACP is JSON-RPC over stdio for local agents, like LSP is for language servers ([ACP](https://agentclientprotocol.com/overview/introduction)). Zed created it, and its page lists Claude Agent, Codex CLI and Gemini CLI as supported agents, plus editors including JetBrains IDEs, Neovim and Emacs ([zed.dev/acp](https://zed.dev/acp)).

This keeps the editor light because the agent's model calls, tools and memory live in the agent's own process. Helm only renders its messages, shows diffs and answers file and terminal requests.

What the editor needs to do well for agents:

- Apply multi-file edits as one undoable transaction, with a reviewable diff per file.
- Reload buffers fast when an agent writes files on disk, without losing unsaved user edits.
- Run agent terminals off the UI thread and cap their scrollback.
- Give agents the same diagnostics the user sees, so they don't re-run a full build to find errors.

## Performance budgets and how to measure them

The numbers below are targets, not measurements. They are set near what the fastest editors above report, so Helm has something concrete to fail against. The PRD's NF requirements adopt them.

| Metric | Target | How to measure |
| --- | --- | --- |
| Keypress to pixel, average | under 5 ms of editor processing | [Typometer](https://pavelfatin.com/typing-with-pleasure/), same method as Pavel Fatin's tests |
| Frame time while scrolling a 10k-line file, p99 | under 8.33 ms (120 Hz) | frame timings from an in-app profiler such as `puffin` or Tracy |
| Cold start to first frame, empty window | under 200 ms | `hyperfine` with a flag that exits after the first frame |
| Editor RSS, one file open, no servers | under 100 MB | `/proc/<pid>/status` on Linux, `footprint` on macOS |
| Open a 100 MB file and jump to the end | under 1 s, UI never blocks | scripted run, same shape as Joe Allen's XML test |
| Rope edit and offset lookups | no regression over 5% | `criterion` microbenchmarks |

For scale, Pavel Fatin measured GVim at 1.4 ms average, Sublime Text at 12.6 ms and Atom at 60.4 ms, on a Windows laptop on battery. Keyboard and monitor alone add about 26 ms, so the editor's share is the part Helm controls.

CI runners on shared machines are noisy, so compare against a rolling baseline, not a fixed number.

## Building it with agents

Split the codebase into crates with narrow interfaces early (buffer, syntax, lsp client, render, agent client). Agents do better when a task touches one crate with its own tests. That is an inference from how agents use context, not a measured result.

Other people's editors are in agents' training data. An agent may reproduce Zed or Helix code, so check licenses. Zed's editor is GPL; Helix is MPL-2.0.

## Risks

- **Scope.** A daily-driver editor needs a lot beyond editing: settings, keybindings, git, terminal, file tree, multi-cursor and IME input. Zed has had a funded team on this for years. Pick a narrow v1.
- **Text input and IME.** Non-Latin input and dead keys are platform-specific and easy for agents to get subtly wrong. Write tests for them early.
- **Cross-platform rendering.** Font shaping and rasterization differ by OS. GPUI uses each OS's shaping API.
- **Toolkit lock-in.** GPUI evolves with Zed's needs. Pin a version and update it on Helm's schedule.
- **Language servers set the memory floor.** No editor choice fixes rust-analyzer at 12 GB. Budgets in CI measure Helm's process alone, with servers tracked separately.
- **Measured numbers are thin.** The editor comparisons found are old (Joe Allen's run on Ubuntu 14.04, Pavel Fatin's from around 2015, judging by the OS versions he used) or vendor claims with no method. Helm's own benchmarks will be the first trustworthy data.

## Sources

- [Leveraging Rust and the GPU to render user interfaces at 120 FPS](https://zed.dev/blog/videogame), Zed
- [Rope & SumTree](https://zed.dev/blog/zed-decoded-rope-sumtree), Zed
- [Zed vs VS Code](https://zed.dev/compare/vscode), Zed (vendor claims)
- [Helix architecture](https://github.com/helix-editor/helix/blob/master/docs/architecture.md)
- [Lapce](https://github.com/lapce/lapce)
- [VS Code extension host](https://code.visualstudio.com/api/advanced-topics/extension-host)
- [Editor performance comparison](https://github.com/jhallen/joes-sandbox/tree/master/editor-perf), Joe Allen
- [Typing with pleasure](https://pavelfatin.com/typing-with-pleasure/), Pavel Fatin
- [Tree-sitter](https://tree-sitter.github.io/tree-sitter/)
- [Announcing TypeScript 7.0](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/) and [A 10x faster TypeScript](https://devblogs.microsoft.com/typescript/typescript-native-port/), Microsoft
- [Scaling gopls](https://go.dev/blog/gopls-scalability), Go blog
- [rust-analyzer memory on the Zed project](https://github.com/rust-lang/rust-analyzer/discussions/20871)
- [Agent Client Protocol](https://agentclientprotocol.com/overview/introduction) and [zed.dev/acp](https://zed.dev/acp)
