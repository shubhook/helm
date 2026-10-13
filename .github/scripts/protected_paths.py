#!/usr/bin/env python3
"""Fail a PR that touches protected paths unless it has the approval label.

Usage:
    protected_paths.py --rules RULES --files FILES [--label NAME ...]
                       [--expected-count N]

RULES is .github/protected-paths.txt from the base branch. FILES holds one
JSON object per line, as returned by the GitHub "list pull request files"
API: {"filename": ..., "status": ..., "previous_filename": ...}. Each
--label is a label currently on the PR. --expected-count is the PR's
`changed_files`; if fewer files were listed, the check fails rather than
pass on a truncated list.

Exits 0 when the PR passes and 1 when it fails. Prints the reason either way.
Uses only the standard library and never reads the PR's code.
"""

import argparse
import json
import re
import sys
from dataclasses import dataclass

APPROVAL_LABEL = "approved-protected"
KINDS = ("any", "existing")


@dataclass(frozen=True)
class Rule:
    kind: str
    glob: str
    regex: re.Pattern

    def __str__(self):
        return f"{self.kind} {self.glob}"


@dataclass(frozen=True)
class Violation:
    path: str
    change: str
    rule: Rule


def glob_to_regex(glob):
    """Translate a path glob to an anchored regex.

    `**` matches any number of whole segments (including none), `*` matches
    within one segment and `?` matches one character other than `/`.
    """
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
    rules = []
    for lineno, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2 or parts[0] not in KINDS:
            raise ValueError(
                f"rules line {lineno}: expected '<any|existing> <glob>', got {raw!r}"
            )
        kind, glob = parts
        rules.append(Rule(kind, glob, glob_to_regex(glob)))
    return rules


def first_match(path, rules, kinds):
    for rule in rules:
        if rule.kind in kinds and rule.regex.match(path):
            return rule
    return None


def touched_paths(entry):
    """Yield (path, change, rule kinds that apply) for one changed file.

    A new path only trips `any` rules. A path that existed before the PR
    (modified, removed, or the old side of a rename) trips both kinds.
    """
    status = entry["status"]
    path = entry["filename"]
    if status == "added":
        yield path, "added", ("any",)
    elif status == "removed":
        yield path, "removed", KINDS
    elif status == "renamed":
        old = entry.get("previous_filename") or path
        yield old, f"renamed to {path}", KINDS
        yield path, f"renamed from {old}", ("any",)
    elif status == "copied":
        old = entry.get("previous_filename") or "?"
        yield path, f"copied from {old}", ("any",)
    else:
        # modified, changed, unchanged, or a status GitHub adds later.
        # Treat it as a change to an existing file.
        yield path, status, KINDS


def find_violations(entries, rules):
    violations = []
    for entry in entries:
        for path, change, kinds in touched_paths(entry):
            rule = first_match(path, rules, kinds)
            if rule:
                violations.append(Violation(path, change, rule))
    return violations


def check(entries, rules, labels, expected_count=None):
    """Return (passed, message)."""
    lines = []
    if expected_count is not None and len(entries) < expected_count:
        return False, (
            f"FAIL: the API listed {len(entries)} of {expected_count} changed "
            "files (it stops at 3000), so the check cannot see the whole PR. "
            "Split the PR."
        )

    violations = find_violations(entries, rules)
    approved = APPROVAL_LABEL in labels

    if not violations:
        return True, (
            f"PASS: none of the {len(entries)} changed files touches a "
            "protected path."
        )

    for v in violations:
        lines.append(f"  {v.path} ({v.change}) hit rule '{v.rule}'")
    listing = "\n".join(lines)

    if approved:
        return True, (
            f"PASS: {len(violations)} protected change(s), approved by the "
            f"'{APPROVAL_LABEL}' label:\n{listing}"
        )
    return False, (
        f"FAIL: {len(violations)} protected change(s) need the "
        f"'{APPROVAL_LABEL}' label, which only Khakha adds:\n{listing}"
    )


def read_entries(path):
    with open(path, encoding="utf-8") as f:
        return [json.loads(line) for line in f if line.strip()]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--rules", required=True)
    parser.add_argument("--files", required=True)
    parser.add_argument("--label", action="append", default=[])
    parser.add_argument("--expected-count", type=int)
    args = parser.parse_args(argv)

    with open(args.rules, encoding="utf-8") as f:
        rules = parse_rules(f.read())
    entries = read_entries(args.files)

    passed, message = check(entries, rules, set(args.label), args.expected_count)
    print(message)
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
