#!/usr/bin/env python3
"""Tests for protected_paths.py. Run: python3 .github/scripts/test_protected_paths.py

The tests use the real .github/protected-paths.txt, so a rule change that
breaks an expected outcome shows up here.
"""

import io
import json
import os
import sys
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

import protected_paths as pp  # noqa: E402

RULES_FILE = os.path.join(HERE, "..", "protected-paths.txt")
with open(RULES_FILE, encoding="utf-8") as f:
    RULES = pp.parse_rules(f.read())


def added(path):
    return {"filename": path, "status": "added"}


def modified(path):
    return {"filename": path, "status": "modified"}


def removed(path):
    return {"filename": path, "status": "removed"}


def renamed(old, new):
    return {"filename": new, "status": "renamed", "previous_filename": old}


def run(*entries, approved=False):
    return pp.check(RULES, list(entries), approved)[0]


class Required(unittest.TestCase):
    """The six cases the T-003 brief names."""

    def test_unprotected_change_passes(self):
        self.assertEqual(run(modified("README.md")), 0)

    def test_protected_change_fails(self):
        self.assertEqual(run(modified("deny.toml")), 1)

    def test_protected_change_with_label_passes(self):
        self.assertEqual(run(modified("deny.toml"), approved=True), 0)

    def test_new_file_under_crate_tests_passes(self):
        self.assertEqual(run(added("crates/x/tests/new.rs")), 0)

    def test_modified_file_under_crate_tests_fails(self):
        self.assertEqual(run(modified("crates/x/tests/old.rs")), 1)

    def test_rename_out_of_protected_path_fails(self):
        self.assertEqual(run(renamed("tasks/101-buffer.md", "notes/101-buffer.md")), 1)
        self.assertEqual(run(renamed("crates/x/tests/a.rs", "crates/x/src/a.rs")), 1)


class Rules(unittest.TestCase):
    def test_every_any_path_fails_on_add_modify_delete(self):
        for path in [
            "tasks/101-buffer.md",
            "docs/prd.md",
            ".github/workflows/ci.yml",
            ".github/protected-paths.txt",
            "deny.toml",
            "rust-toolchain.toml",
            "AGENTS.md",
            "crates/helm-bench/budgets.toml",
        ]:
            for entry in (added(path), modified(path), removed(path)):
                with self.subTest(entry=entry):
                    self.assertEqual(run(entry), 1)

    def test_deleted_crate_test_fails(self):
        self.assertEqual(run(removed("crates/helm-text/tests/offsets.rs")), 1)

    def test_nested_crate_test_is_protected(self):
        self.assertEqual(run(modified("crates/x/tests/data/case.txt")), 1)

    def test_rename_into_crate_tests_counts_as_add(self):
        self.assertEqual(run(renamed("crates/x/src/a.rs", "crates/x/tests/a.rs")), 0)

    def test_rename_within_crate_tests_fails(self):
        self.assertEqual(run(renamed("crates/x/tests/a.rs", "crates/x/tests/b.rs")), 1)

    def test_rename_into_any_path_fails(self):
        self.assertEqual(run(renamed("notes.md", "docs/notes.md")), 1)

    def test_star_stays_in_one_segment(self):
        self.assertEqual(run(modified("crates/x/y/tests/a.rs")), 0)

    def test_lookalike_paths_pass(self):
        for path in ["src/deny.toml", "deny.toml.bak", "my-tasks/a.md", "docs.md", "crates/x/src/tests.rs"]:
            with self.subTest(path=path):
                self.assertEqual(run(modified(path)), 0)

    def test_unknown_status_counts_as_modified(self):
        self.assertEqual(run({"filename": "deny.toml", "status": "changed"}), 1)
        self.assertEqual(run({"filename": "crates/x/tests/a.rs", "status": "something-new"}), 1)

    def test_copy_counts_as_add(self):
        entry = {"filename": "crates/x/tests/b.rs", "status": "copied", "previous_filename": "crates/x/tests/a.rs"}
        self.assertEqual(run(entry), 0)

    def test_api_file_limit_fails_closed(self):
        entries = [modified(f"src/{i}.rs") for i in range(pp.API_FILE_LIMIT)]
        self.assertEqual(pp.check(RULES, entries, True)[0], 1)

    def test_report_names_file_and_rule(self):
        code, lines = pp.check(RULES, [modified("deny.toml"), modified("README.md")], False)
        self.assertEqual(code, 1)
        self.assertIn("deny.toml (modified) hits rule `any deny.toml`", lines)
        self.assertFalse(any("README.md" in line for line in lines))

    def test_bad_rule_line_is_rejected(self):
        with self.assertRaises(ValueError):
            pp.parse_rules("protect deny.toml\n")


class Cli(unittest.TestCase):
    def main(self, entries, *flags):
        stdin = io.StringIO("".join(json.dumps(e) + "\n" for e in entries))
        with mock.patch("sys.stdin", stdin), mock.patch("sys.stdout", io.StringIO()):
            return pp.main([RULES_FILE, *flags])

    def test_exit_codes(self):
        self.assertEqual(self.main([modified("README.md")]), 0)
        self.assertEqual(self.main([modified("deny.toml")]), 1)
        self.assertEqual(self.main([modified("deny.toml")], "--approved"), 0)

    def test_malformed_input_exits_2(self):
        with mock.patch("sys.stderr", io.StringIO()):
            self.assertEqual(self.main([{"status": "modified"}]), 2)
            self.assertEqual(self.main([{"filename": "a", "status": "renamed"}]), 2)


if __name__ == "__main__":
    unittest.main()
