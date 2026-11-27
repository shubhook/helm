#!/usr/bin/env bash
# Rewrites main.toml from the criterion results that CI's bench-record job
# saved for one main commit (the artifact criterion-<sha>). T-006.
#
# Usage: crates/helm-bench/baselines/refresh.sh <full commit sha>
#
# Needs gh (signed in, with read access to the repo) and jq 1.7 or later,
# which prints each number exactly as estimates.json holds it, so the same
# artifact always gives the same file. GitHub deletes the artifact and the
# job log 90 days after the run, so this only works within that window.
#
# Set REPO to read another repository (default shubhook/helm).
#
# No check reads main.toml. bench-compare still compares each PR with its
# base commit on one runner.

set -euo pipefail

if [ $# -ne 1 ] || ! [[ $1 =~ ^[0-9a-f]{40}$ ]]; then
  echo "usage: $0 <full 40-character commit sha>" >&2
  exit 2
fi
sha=$1
repo=${REPO:-shubhook/helm}
name="criterion-$sha"
out="$(cd "$(dirname "$0")" && pwd)/main.toml"

if [[ $(jq --version) =~ ^jq-1\.[0-6]([^0-9]|$) ]]; then
  echo "error: needs jq 1.7 or later, found $(jq --version)" >&2
  exit 1
fi

artifact=$(gh api "repos/$repo/actions/artifacts?name=$name&per_page=100" \
  --jq '[.artifacts[] | select(.expired | not)] | sort_by(.created_at) | last // empty')
if [ -z "$artifact" ]; then
  echo "error: no unexpired artifact named $name in $repo" >&2
  exit 1
fi
artifact_id=$(jq -r '.id' <<< "$artifact")
run_id=$(jq -r '.workflow_run.id' <<< "$artifact")
created=$(jq -r '.created_at' <<< "$artifact")
if [ "$(jq -r '.workflow_run.head_sha' <<< "$artifact")" != "$sha" ]; then
  echo "error: artifact $artifact_id was not made by a run of $sha" >&2
  exit 1
fi

job=$(gh api "repos/$repo/actions/runs/$run_id/jobs?per_page=100" \
  --jq '.jobs[] | select(.name == "bench-record")')
if [ -z "$job" ]; then
  echo "error: run $run_id has no bench-record job" >&2
  exit 1
fi
job_id=$(jq -r '.id' <<< "$job")
job_url=$(jq -r '.html_url' <<< "$job")
runner_name=$(jq -r '.runner_name' <<< "$job")
runner_label=$(jq -r '.labels | join(",")' <<< "$job")

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

# The runner image and the criterion flags are only in the job log.
gh api --allow-escape-sequences "repos/$repo/actions/jobs/$job_id/logs" > "$tmp/job.log"
log_value() {
  # The value on the first line holding "$2" after the first line ending in
  # "$1". Each log line starts with a timestamp, then the text.
  awk -v start="$1" -v key="$2" '
    { sub(/\r$/, "") }
    !on && substr($0, length($0) - length(start) + 1) == start { on = 1; next }
    on && (i = index($0, key)) {
      v = substr($0, i + length(key)); sub(/^ +/, "", v)
      print v; exit
    }' "$tmp/job.log"
}
image="$(log_value '##[group]Runner Image' 'Image:') $(log_value '##[group]Runner Image' 'Version:')"
# BENCH_FLAGS is set for the whole job, so every step prints the same value.
bench_flags=$(log_value '' 'BENCH_FLAGS:')

gh run download "$run_id" -R "$repo" -n "$name" -D "$tmp/criterion"

{
  cat <<EOF
# Benchmark times on main, recorded by CI's bench-record job. T-006.
#
# Written by refresh.sh from the artifact named below. Don't edit by hand:
# rerun refresh.sh with a newer main commit instead. No check reads this
# file, and bench-compare never compares a PR with it.
#
# Times are nanoseconds per iteration. Each interval is criterion's 95%
# bootstrap confidence interval. bench-compare gates on the median; the mean
# is here too because criterion reports it.

[source]
commit = "$sha"
artifact = "$name"
artifact_id = $artifact_id
artifact_created = "$created"
run = "https://github.com/$repo/actions/runs/$run_id"
job = "$job_url"
runner_label = "$runner_label"
runner_name = "$runner_name"
runner_image = "$image"
bench_flags = "$bench_flags"
EOF

  find "$tmp/criterion" -path '*/main/benchmark.json' | LC_ALL=C sort |
    while read -r bench; do
      jq -r --slurpfile e "${bench%benchmark.json}estimates.json" '
        $e[0] as $e
        | "\n[benchmarks.\"\(.full_id)\"]",
          (("mean", "median") as $k
           | "\($k) = \($e[$k].point_estimate)",
             "\($k)_lower = \($e[$k].confidence_interval.lower_bound)",
             "\($k)_upper = \($e[$k].confidence_interval.upper_bound)")
      ' "$bench"
    done
} > "$out"

echo "wrote $out from $name (run $run_id)"
