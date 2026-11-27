# T-006: Keep main's benchmark numbers past the artifact's 90 days

After: T-005. This task waits until that one is merged. It is.

## Goal

Phase 1's gate in `docs/plan.md` asks for a rope microbenchmark baseline to be recorded. Today the only record is CI's `bench-record` job, which saves main's criterion results as the artifact `criterion-<sha>`, and GitHub deletes that after 90 days. This task commits a copy of one main commit's numbers to the repo, with a script that rebuilds the copy from a `bench-record` artifact, so the numbers outlive the artifact and anyone can check them against it while it exists.

PRD: NF-7

## Crates and public interface

Crates: `helm-bench`, new files only, in a new directory `crates/helm-bench/baselines/`. Nothing in `src/`, `benches/`, `Cargo.toml` or `budgets.toml` changes, and no workflow changes.

There is no Rust interface. The task adds two files:

| File | What it is |
| --- | --- |
| `crates/helm-bench/baselines/main.toml` | One main commit's criterion numbers, written by `refresh.sh`. |
| `crates/helm-bench/baselines/refresh.sh` | `refresh.sh <full commit sha>` downloads `criterion-<sha>`, reads the run and job it came from, and rewrites `main.toml`. |

### What `main.toml` holds

A `[source]` table says where the numbers came from:

| Key | Value |
| --- | --- |
| `commit` | The main commit `bench-record` measured. |
| `artifact`, `artifact_id`, `artifact_created` | The artifact's name, id and creation time. |
| `run`, `job` | Links to the CI run and its `bench-record` job. |
| `runner_label`, `runner_name`, `runner_image` | The `runs-on` label, the hosted runner's name, and the runner image and version from the job log. |
| `bench_flags` | The criterion flags the job ran with. |

Then one `[benchmarks."<criterion full id>"]` table per benchmark in the artifact, sorted by id, with `mean`, `mean_lower`, `mean_upper`, `median`, `median_lower` and `median_upper` in nanoseconds per iteration. Each pair of bounds is criterion's 95% bootstrap confidence interval. Every value is copied from the `main` baseline's `estimates.json`, number for number.

The rope baseline the Phase 1 gate asks for is the three `text/` benchmarks from `benches/text.rs`: `text/insert_middle_10mb`, `text/byte_to_line_10mb` and `text/utf16_to_byte_10mb`. The file holds every benchmark in the artifact, because filtering would only add a step to the script.

The median is there because `bench-compare` gates on it. The mean is there because criterion reports it and the brief asked for it. The `time:` line in the job log is neither: criterion prints its slope estimate there, so it won't match either column exactly.

### The committed numbers

From `criterion-5f0c4259bc1d7bdc383bcd46c61d1343bc6f0d96` (artifact id 15093909343), made by [run 45772477109](https://github.com/shubhook/helm/actions/runs/45772477109) on main commit `5f0c4259bc1d7bdc383bcd46c61d1343bc6f0d96`, on an `ubuntu-latest` runner with image `ubuntu-26.04` version `20261122.9.1`. The artifact was created on 2026-11-26 and expires around 2027-02-24. No number comes from a local run.

### Refreshing

Run `crates/helm-bench/baselines/refresh.sh <sha>` for a main commit whose `criterion-<sha>` artifact hasn't expired, and commit the new `main.toml`. The script needs `gh`, signed in with read access, and `jq` 1.7 or later, which prints numbers exactly as `estimates.json` holds them. Older numbers stay in git history, so one file is enough. Refresh when a gate or report needs current numbers; nothing refreshes it automatically.

### Choices I rejected

- **Store the numbers in a release asset from `bench-record` (option b).** It needs `contents: write` on a job that runs `cargo bench` on main's code, a change to `.github/workflows/ci.yml`, and a release or tag per commit that nobody reviews. Release assets can also be deleted, and the numbers would sit outside the repo where a gate report can't link to them by commit. A committed file is reviewed in a PR and lasts as long as the repo.
- **Raise `retention-days`.** GitHub caps artifact retention, at 90 days for public repositories, so the numbers would still expire.
- **Commit the whole criterion directory.** About 48 KB per refresh, mostly raw samples nobody reads. The six statistics per benchmark are what a report quotes, and the artifact id says where the rest was.
- **Have CI commit the file to main, or push to a `gh-pages` branch** (for example with `benchmark-action/github-action-benchmark`). Both need a write token in CI and a new third-party action, and they land numbers on main without review.
- **Compare PRs with the committed numbers.** The perf-hardware decision in `docs/plan.md` says shared runners are too noisy for that, and `bench-compare` keeps comparing each PR with its base commit on one runner. `main.toml` is a record for reports, not a threshold.

## Acceptance tests

No new test and no CI step. A test that reads `main.toml` could fail a PR, and this file must never change a PR's result. Nothing checks it in CI; the checks below are for the reviewer while the artifact exists.

How a reviewer checks `main.toml` against the artifact:

1. Rebuild it and diff. On this PR's branch, run `crates/helm-bench/baselines/refresh.sh 5f0c4259bc1d7bdc383bcd46c61d1343bc6f0d96`, then `git diff --exit-code crates/helm-bench/baselines/main.toml`. No diff means the file is exactly what the artifact gives.
2. Or spot-check by hand. Download the artifact with `gh run download 45772477109 -R shubhook/helm -n criterion-5f0c4259bc1d7bdc383bcd46c61d1343bc6f0d96 -D /tmp/criterion`, then compare one benchmark's numbers, for example `jq '.mean, .median' /tmp/criterion/text/insert_middle_10mb/main/estimates.json` against the `text/insert_middle_10mb` table.
3. Check `[source]` against the run page: the commit, the `bench-record` job link, and the runner image in the job log's "Runner Image" group.

The PR must not change the result of `bench`, `bench-plan`, `bench-shard` or any other check on any PR, since no workflow or checked file reads the new directory.

## Performance budget

None. This task records numbers and gates nothing. `budgets.toml` and every benchmark stay as they are.

## Out of scope

- Comparing later numbers with `main.toml` in CI, or any automatic refresh. A later task, if a gate report needs it, and only in a way that keeps PR results unchanged.
- Recording the runner's CPU model. The job log doesn't print it, and printing it needs a workflow change. Numbers from two refreshes may come from different hardware, which is one more reason not to compare them as a gate.
- Plots or a history of every main commit.
