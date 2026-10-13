#!/usr/bin/env python3
"""Fail a PR that breaks a rule in .github/protected-paths.txt.

Reads the PR's changed files as JSON lines on stdin, one object per file,
in the shape the GitHub API returns from
GET /repos/{owner}/{repo}/pulls/{number}/files: `filename`, `status` and,
for renames, `previous_filename`.

Usage:
    gh api --paginate repos/OWNER/REPO/pulls/N/files --jq '.[]' \\
        | protected_paths.py RULES_FILE [--approved]

Exits 0 when no rule is broken or when --approved is given, 1 when a rule
is broken without approval, 2 on bad input.

This script never runs code from the PR. It only reads file names.
"""

import json
import re
import sys

LABEL = "approved-protected"
KINDS = ("any", "edit")

# The list-files endpoint returns at most this many files. A PR at the cap
# may have more, so the check cannot see all of them and fails closed.
API_FILE_LIMIT = 3000


def glob_to_regex(glob):
    """Turn a path glob into a regex that matches the whole path."""
    out = []
    i = 0
    while i < len(glob):
        if glob.startswith("**/", i):
            out.append("(?:.*/)?")
            i += 3
        elif glob.startswith("**", i):
            out.append(".*")
            i += 2
        elif glob[i] == "*":
            out.append("[^/]*")
            i += 1
        elif glob[i] == "?":
            out.append("[^/]")
            i += 1
        else:
            out.append(re.escape(glob[i]))
            i += 1
    return re.compile("".join(out) + r"\Z")


def parse_rules(text):
    """Return a list of (kind, glob, regex) from the rules file text."""
    rules = []
    for lineno, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2 or parts[0] not in KINDS:
            raise ValueError(f"rules line {lineno}: expected '<any|edit> <glob>', got {raw!r}")
        kind, glob = parts
        rules.append((kind, glob, glob_to_regex(glob)))
    return rules


def changes(entry):
    """Split one API file entry into (path, action) pairs.

    action is "added", "modified" or "removed". A rename is the old path
    removed and the new path added. Unknown statuses count as modified, so
    a new status the API adds later is checked rather than skipped.
    """
    status = entry.get("status")
    path = entry["filename"]
    if status == "added" or status == "copied":
        return [(path, "added")]
    if status == "removed":
        return [(path, "removed")]
    if status == "renamed":
        return [(entry["previous_filename"], "removed"), (path, "added")]
    return [(path, "modified")]


def violations(rules, entries):
    """Return (path, action, rule) for every change that breaks a rule."""
    found = []
    for entry in entries:
        for path, action in changes(entry):
            for kind, glob, regex in rules:
                if kind == "edit" and action == "added":
                    continue
                if regex.match(path):
                    found.append((path, action, f"{kind} {glob}"))
                    break
    return found


def check(rules, entries, approved):
    """Return (exit_code, report_lines)."""
    if len(entries) >= API_FILE_LIMIT:
        return 1, [
            f"The PR changes {len(entries)} or more files, the most the GitHub API lists.",
            "The check cannot see every file, so it fails. Split the PR.",
        ]
    found = violations(rules, entries)
    if not found:
        return 0, [f"Pass: none of the {len(entries)} changed files touch a protected path."]
    lines = [f"{path} ({action}) hits rule `{rule}`" for path, action, rule in found]
    if approved:
        return 0, [f"Pass: the PR has the `{LABEL}` label, so these protected changes are allowed:"] + lines
    return 1, [
        f"Fail: these changes need the `{LABEL}` label, which only Khakha adds:"
    ] + lines


def main(argv):
    args = [a for a in argv if a != "--approved"]
    approved = "--approved" in argv
    if len(args) != 1:
        print(__doc__, file=sys.stderr)
        return 2
    try:
        with open(args[0], encoding="utf-8") as f:
            rules = parse_rules(f.read())
        entries = [json.loads(line) for line in sys.stdin if line.strip()]
        code, lines = check(rules, entries, approved)
    except (OSError, ValueError, KeyError) as e:
        print(f"protected-paths: bad input: {e!r}", file=sys.stderr)
        return 2
    for line in lines:
        print(line)
    return code


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
