"""Tests for protected_paths.py, run against the real protected-paths.txt.

Run from the repo root:
    python3 -m unittest discover -s .github/scripts -v
"""

import json
import os
import tempfile
import unittest

import protected_paths as pp

HERE = os.path.dirname(os.path.abspath(__file__))
RULES_PATH = os.path.join(HERE, "..", "protected-paths.txt")


def load_rules():
    with open(RULES_PATH, encoding="utf-8") as f:
        return pp.parse_rules(f.read())


def added(path):
    return {"filename": path, "status": "added"}


def modified(path):
    return {"filename": path, "status": "modified"}


def removed(path):
    return {"filename": path, "status": "removed"}


def renamed(old, new):
    return {"filename": new, "status": "renamed", "previous_filename": old}


class CheckTest(unittest.TestCase):
    def setUp(self):
        self.rules = load_rules()

    def run_check(self, entries, labels=(), expected_count=None):
        return pp.check(entries, self.rules, set(labels), expected_count)

    def assert_passes(self, entries, labels=()):
        passed, message = self.run_check(entries, labels)
        self.assertTrue(passed, message)
        return message

    def assert_fails(self, entries, labels=()):
        passed, message = self.run_check(entries, labels)
        self.assertFalse(passed, message)
        return message

    def test_unprotected_change_passes(self):
        msg = self.assert_passes([modified("README.md"), added("crates/helm/src/x.rs")])
        self.assertIn("none of the 2 changed files", msg)

    def test_protected_change_fails(self):
        msg = self.assert_fails([modified("README.md"), modified("deny.toml")])
        self.assertIn("deny.toml (modified) hit rule 'any deny.toml'", msg)
        self.assertNotIn("README.md", msg)

    def test_protected_change_with_label_passes(self):
        msg = self.assert_passes([modified("deny.toml")], labels=["approved-protected"])
        self.assertIn("approved", msg)
        self.assertIn("deny.toml", msg)

    def test_other_label_does_not_approve(self):
        self.assert_fails([modified("deny.toml")], labels=["enhancement"])

    def test_new_file_under_crate_tests_passes(self):
        self.assert_passes([added("crates/x/tests/new_test.rs")])

    def test_modified_file_under_crate_tests_fails(self):
        msg = self.assert_fails([modified("crates/x/tests/acceptance.rs")])
        self.assertIn("'existing crates/*/tests/**'", msg)

    def test_removed_file_under_crate_tests_fails(self):
        self.assert_fails([removed("crates/x/tests/acceptance.rs")])

    def test_rename_out_of_protected_path_fails(self):
        msg = self.assert_fails([renamed("crates/x/tests/acceptance.rs", "crates/x/src/moved.rs")])
        self.assertIn("crates/x/tests/acceptance.rs (renamed to crates/x/src/moved.rs)", msg)

    def test_rename_into_crate_tests_passes(self):
        self.assert_passes([renamed("crates/x/src/helper.rs", "crates/x/tests/helper.rs")])

    def test_rename_into_any_path_fails(self):
        self.assert_fails([renamed("notes.md", "docs/notes.md")])

    def test_adding_to_any_path_fails(self):
        for path in [
            "tasks/101-buffer-trait.md",
            "docs/new.md",
            ".github/workflows/new.yml",
            "crates/helm-bench/budgets.toml",
        ]:
            with self.subTest(path=path):
                self.assert_fails([added(path)])

    def test_every_any_rule_blocks_modification(self):
        for path in [
            "tasks/TEMPLATE.md",
            "docs/plan.md",
            ".github/protected-paths.txt",
            ".github/scripts/protected_paths.py",
            ".github/workflows/ci.yml",
            "deny.toml",
            "rust-toolchain.toml",
            "crates/helm-bench/budgets.toml",
            "AGENTS.md",
        ]:
            with self.subTest(path=path):
                self.assert_fails([modified(path)])

    def test_lookalike_paths_pass(self):
        for path in [
            "crates/helm/src/tests/unit.rs",
            "crates/helm/benches/budgets.toml",
            "mytasks/x.md",
            "sub/deny.toml",
            "CLAUDE.md",
            "crates/a/b/tests/x.rs",
        ]:
            with self.subTest(path=path):
                self.assert_passes([modified(path)])

    def test_truncated_file_list_fails(self):
        passed, message = self.run_check([modified("README.md")], expected_count=3001)
        self.assertFalse(passed)
        self.assertIn("1 of 3001", message)


class GlobTest(unittest.TestCase):
    def match(self, glob, path):
        return bool(pp.glob_to_regex(glob).match(path))

    def test_double_star_spans_segments(self):
        self.assertTrue(self.match("tasks/**", "tasks/a.md"))
        self.assertTrue(self.match("tasks/**", "tasks/a/b.md"))
        self.assertTrue(self.match("a/**/b", "a/b"))
        self.assertTrue(self.match("a/**/b", "a/x/y/b"))
        self.assertTrue(self.match("**/b", "b"))
        self.assertTrue(self.match("**/b", "x/y/b"))

    def test_single_star_stays_in_segment(self):
        self.assertTrue(self.match("crates/*/tests/**", "crates/x/tests/t.rs"))
        self.assertFalse(self.match("crates/*/tests/**", "crates/x/y/tests/t.rs"))
        self.assertFalse(self.match("*.toml", "a/b.toml"))

    def test_whole_path_anchored(self):
        self.assertFalse(self.match("deny.toml", "deny.toml.bak"))
        self.assertFalse(self.match("deny.toml", "x/deny.toml"))
        self.assertFalse(self.match("a.b", "axb"))


class RulesFileTest(unittest.TestCase):
    def test_bad_line_rejected(self):
        with self.assertRaises(ValueError):
            pp.parse_rules("tasks/**\n")
        with self.assertRaises(ValueError):
            pp.parse_rules("sometimes tasks/**\n")

    def test_comments_and_blanks_ignored(self):
        rules = pp.parse_rules("# c\n\n  any a/**  \n")
        self.assertEqual([str(r) for r in rules], ["any a/**"])


class MainTest(unittest.TestCase):
    def test_exit_codes(self):
        with tempfile.TemporaryDirectory() as d:
            files = os.path.join(d, "files.jsonl")
            with open(files, "w", encoding="utf-8") as f:
                f.write(json.dumps(modified("deny.toml")) + "\n")
            argv = ["--rules", RULES_PATH, "--files", files]
            self.assertEqual(pp.main(argv), 1)
            self.assertEqual(pp.main(argv + ["--label", "approved-protected"]), 0)


if __name__ == "__main__":
    unittest.main()
