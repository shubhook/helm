#!/usr/bin/env bash
# Rewrites main.toml from the criterion results that CI's bench-record job
# saved for one main commit (the artifact criterion-<sha>). T-006.
#
# Usage: crates/helm-bench/baselines/refresh.sh <full commit sha>
#
# Needs gh, signed in with read access to the repo, in a version whose
# `gh api` has --allow-escape-sequences: the job log holds terminal colour
# codes, and gh refuses to print them without it. Also needs jq 1.7 or
# later, which prints each number exactly as estimates.json holds it, so the
# same artifact always gives the same file. GitHub deletes the artifact and
# the job log 90 days after the run, so this only works within that window.
#
# Set REPO to read another repository (default shubhook/helm).
#
# main.toml is replaced only once the whole file is written and holds at
# least one benchmark; on any error it is left as it was.
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

die() {
  echo "error: $*" >&2
  exit 1
}

if [[ $(jq --version) =~ ^jq-1\.[0-6]([^0-9]|$) ]]; then
  die "needs jq 1.7 or later, found $(jq --version)"
fi
if [[ $(gh api --help 2>&1) != *--allow-escape-sequences* ]]; then
  die "needs a gh whose 'gh api' supports --allow-escape-sequences; update gh"
fi

artifact=$(gh api "repos/$repo/actions/artifacts?name=$name&per_page=100" \
  --jq '[.artifacts[] | select(.expired | not)] | sort_by(.created_at) | last // empty')
[ -n "$artifact" ] || die "no unexpired artifact named $name in $repo"
artifact_id=$(jq -r '.id' <<< "$artifact")
artifact_digest=$(jq -r '.digest // empty' <<< "$artifact")
run_id=$(jq -r '.workflow_run.id' <<< "$artifact")
created=$(jq -r '.created_at' <<< "$artifact")
if [ "$(jq -r '.workflow_run.head_sha' <<< "$artifact")" != "$sha" ]; then
  die "artifact $artifact_id was not made by a run of $sha"
fi
[ -n "$artifact_digest" ] || die "artifact $artifact_id has no digest"

job=$(gh api "repos/$repo/actions/runs/$run_id/jobs?per_page=100" \
  --jq '.jobs[] | select(.name == "bench-record")')
[ -n "$job" ] || die "run $run_id has no bench-record job"
job_id=$(jq -r '.id' <<< "$job")
job_url=$(jq -r '.html_url' <<< "$job")
runner_name=$(jq -r '.runner_name' <<< "$job")
runner_label=$(jq -r '.labels | join(",")' <<< "$job")

tmp=$(mktemp -d)
# Next to main.toml, so the final mv is a rename on one filesystem.
staging=$(mktemp "$out.XXXXXX")
trap 'rm -rf "$tmp" "$staging"' EXIT

# The runner image and the criterion flags are only in the job log.
gh api --allow-escape-sequences "repos/$repo/actions/jobs/$job_id/logs" > "$tmp/job.log"
log_value() {
  # The value after "$2" on the first line holding it. With a group title in
  # $1, only lines inside that log group count. Each log line starts with a
  # timestamp, then the text.
  awk -v group="$1" -v key="$2" '
    BEGIN { on = (group == ""); title = " ##[group]" group }
    { sub(/\r$/, "") }
    !on {
      n = length($0) - length(title)
      if (n >= 0 && substr($0, n + 1) == title) on = 1
      next
    }
    group != "" && index($0, "##[endgroup]") { exit }
    (i = index($0, key)) {
      v = substr($0, i + length(key)); sub(/^ +/, "", v)
      print v; exit
    }' "$tmp/job.log"
}
image_name=$(log_value 'Runner Image' 'Image:')
image_version=$(log_value 'Runner Image' 'Version:')
# BENCH_FLAGS is set for the whole job, so every step prints the same value.
bench_flags=$(log_value '' 'BENCH_FLAGS:')
[ -n "$image_name" ] || die "no runner image name in the job log of job $job_id"
[ -n "$image_version" ] || die "no runner image version in the job log of job $job_id"
[ -n "$bench_flags" ] || die "no BENCH_FLAGS in the job log of job $job_id"

gh run download "$run_id" -R "$repo" -n "$name" -D "$tmp/criterion"

{
  cat <<'EOF'
# Benchmark times on main, recorded by CI's bench-record job. T-006.
#
# Written by refresh.sh from the artifact named below. Don't edit by hand:
# rerun refresh.sh with a newer main commit instead. No check reads this
# file, and bench-compare never compares a PR with it.
#
# Times are nanoseconds per iteration. Each interval is criterion's 95%
# bootstrap confidence interval. bench-compare gates on the median; the mean
# is here too because criterion reports it. artifact_digest is GitHub's
# SHA-256 of the artifact zip, which identifies a copy found after GitHub
# deletes the artifact.

[source]
EOF

  # tojson escapes quotes, backslashes and control characters, and its
  # escapes are valid in TOML basic strings.
  jq -rn \
    --arg commit "$sha" \
    --arg artifact "$name" \
    --argjson artifact_id "$artifact_id" \
    --arg artifact_digest "$artifact_digest" \
    --arg artifact_created "$created" \
    --arg run "https://github.com/$repo/actions/runs/$run_id" \
    --arg job "$job_url" \
    --arg runner_label "$runner_label" \
    --arg runner_name "$runner_name" \
    --arg runner_image "$image_name $image_version" \
    --arg bench_flags "$bench_flags" \
    '$ARGS.named | to_entries[] | "\(.key) = \(.value | tojson)"'

  find "$tmp/criterion" -path '*/main/benchmark.json' | LC_ALL=C sort |
    while read -r bench; do
      jq -r --slurpfile e "${bench%benchmark.json}estimates.json" '
        $e[0] as $e
        | def num($k; f): ($e[$k] | f) // error("\(.full_id): no \($k)");
        "\n[benchmarks.\(.full_id | tojson)]",
          (("mean", "median") as $k
           | "\($k) = \(num($k; .point_estimate))",
             "\($k)_lower = \(num($k; .confidence_interval.lower_bound))",
             "\($k)_upper = \(num($k; .confidence_interval.upper_bound))")
      ' "$bench" || exit 1
    done
} > "$staging"

count=$(grep -c '^\[benchmarks\.' "$staging" || true)
[ "$count" -gt 0 ] || die "artifact $name holds no benchmarks saved as 'main'"

chmod 644 "$staging"
mv "$staging" "$out"
echo "wrote $out: $count benchmarks from $name (run $run_id)"
