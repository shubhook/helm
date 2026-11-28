Task: T-NNN, [tasks/NNN-short-name.md](https://github.com/shubhook/helm/blob/main/tasks/NNN-short-name.md)

PRD: <!-- IDs, or "none" -->

## What changed

<!-- What this PR does and why. Call out new dependencies (with the reason), `unsafe`, threading, and anything the task file asks the reviewer to check. -->

## Commands run

<!-- Each command from AGENTS.md and its result (exit code, test counts). -->

```
$ cargo build --workspace
$ cargo test --workspace
$ cargo fmt --all --check
$ cargo clippy --workspace --all-targets -- -D warnings
$ cargo bench -p helm-bench
```

## Grants

<!-- Each written grant from Khakha this PR relies on, quoted word for word, and the commit that carries it. Write "none" if none. -->

## Outside sources

<!-- Docs, articles or repos you followed, with links. Write "none" if none. -->

## Council review

<!-- Each round: the head it reviewed, each delegate's overall score, performance score and verdict, and what you did with each finding (fixed, kept with a reason, or held for Khakha). If there was no council, write "none" and why. -->

## Agent and model

<!-- The agent and model used. Did either change mid-thread? If yes, say when. -->
