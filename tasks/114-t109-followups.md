# T-114: Follow up Khakha's answers to PR #42's open questions

After: T-109. This task waits until that one is merged. It is, as PR #42 (`7638344`).

## Goal

PR #42 (T-109) merged with five open questions for Khakha and three nonblocking notes from its third council round. This task carries out Khakha's answers: it caps the bracket scan at 1 MiB, grows repeated visual bracket objects to the next pair out, counts backslashes before a quote as Vim does, keeps the 1 GiB put limit, and brings a stale section of `tasks/109-vim-objects.md` up to date. It also fixes two comments and adds the missing CRLF test from round 3. Like T-111, the task file and the work land in one PR.

PRD: ED-5, NF-1

## Crates and public interface

Crates: `helm-editor`, in `src/**` and new files in `tests/`. Also `tasks/114-t109-followups.md`, and `tasks/109-vim-objects.md` for answer 5 only. Nothing in `helm-text` or `helm-bench`.

No public type, trait or function changes. The work changes crate-private code in `src/motion/vim/object.rs`, `src/editor/vim.rs` (`Editor::vim_select_object`) and `src/motion.rs` (a `Backward::floored` constructor), the `vim` module docs in `src/vim.rs`, and two comments in `src/editor.rs` and `src/editor/vim.rs`.

## Khakha's answers

Quoted as the orchestrator relayed them:

> Answers to PR #42's open questions, from Khakha:
> 1. Question 6 (no-pair bracket scan): cap the scan at 1 MiB in each direction from the cursor, and treat a capped scan as no pair. List it in vim.rs's 'Measured against Vim' as a difference.
> 2. Repeated visual bracket objects (va(a(, vi(i(): grow to the next pair out, as Vim does.
> 3. Escaped quotes: a quote is escaped only when an odd number of backslashes come right before it, as Vim does.
> 4. The 1 GiB put limit: keep it, refused silently, for now. Feedback for a refused put is a later helm-ui task.
> 5. Update the stale 'How this compiles without the two pending grants' section of tasks/109-vim-objects.md to describe what merged.
>
> If any of 1 to 3 would change an assertion in an existing test, stop and tell me which one.

None of 1 to 3 changes an existing test's assertion. Every existing test passes unchanged.

## The work

1. **The bracket scan's cap.** A bracket object reads at most 1 MiB (`1 << 20` bytes) each way from where its scan starts: the cursor, or the first char of a longer visual selection. A pair counts only if both brackets lie from 1 MiB before that point up to, not including, 1 MiB after it. A scan that reaches either end without its bracket finds no pair. A count and a visual growth step share the same window. `vim.rs` lists the cap under "Measured against Vim".
2. **Visual bracket objects from a longer selection.** Measured in Vim 9.1 (`vim -u NONE -N -i NONE -n`, `binary`, `noeol`), Vim's rule for a selection longer than one char is:
   - The scan starts before the selection's first char, so a bracket there isn't the pair's. The pair is the `n`th out from there.
   - `a` selects that pair.
   - `i` selects its inside. While that inside lies within the selection and is more than one char, it moves out one pair.
   - The new selection runs forward, from the pair's first char to its last, whichever way the old one ran.

   So `va(a(` and `vi(i(` grow to the next pair out. Helm now does this for bracket objects. A one-char selection still starts a fresh object, and word, quote and paragraph objects don't change.
3. **Escaped quotes.** A backslash escapes the char after it, including another backslash, so a quote is escaped only after an odd number of backslashes.
4. **The 1 GiB put limit.** Unchanged, as answered.
5. **`tasks/109-vim-objects.md`.** The section "How this compiles without the two pending grants" becomes "How the two grants landed" and describes what merged. It is its own commit.
6. **Round 3's notes.**
   - `EditorId`'s docs said the editor owns the only strong reference. They now say it owns the only lasting one, and that `CommandRegistry::run_in` holds a clone while a command runs.
   - The comment on normal linewise `p` below a last line with no line break said the bytes move in place. It now says `split_off` copies the line break, 1 or 2 bytes, into a small string, and the rest stays in the text's own allocation.
   - The CRLF test for that `p` is added.

### Measured against Vim 9.1

Text is shown as a Rust string, `|` before the char the cursor is on. The main column was run on `7638344`.

Repeated and longer-selection bracket objects, each ending in `d`:

| Text | Keys | Vim 9.1 and Helm after T-114 | Helm on main |
| --- | --- | --- | --- |
| `"f(a(b\|c)d)e"` | `va(a(d` | `"f\|e"` | `"f(a\|d)e"` |
| `"f(a(b\|c)d)e"` | `vi(i(d` | `"f(\|)e"` | `"f(a(\|)d)e"` |
| `"f(a(b\|c)d)e"` | `vi(a(d` | `"f(a\|d)e"` | `"f(a\|d)e"` |
| `"f(a(b\|c)d)e"` | `va(i(d` | `"f(\|)e"` | `"f(a\|d)e"` |
| `"((((\|bc))))"` | `vi(2i(d` | `"(((\|)))"` | `"(((\|)))"` |
| `"((((\|bc))))"` | `vi(i(i(d` | `"((\|))"` | `"((((\|))))"` |
| `"f(a(\|b)c(d)e)"` | `v4la(d` | `"f(a\|c(d)e)"` | `"f(a(\|e)"` |
| `"f(a(\|b)c(d)e)"` | `v4li(d` | `"f(a(\|)c(d)e)"` | `"f(a(\|)e)"` |
| `"f(a(b\|)c)"` | `vli(d` | `"f(a(\|)c)"` | `"f(\|)"` |

Beyond the table rows, two runs of 3,000 random cases each compared Helm with Vim: `v`, up to three of `h`, `l` and `o`, one to three of `i(`, `a(`, `2i(` and `2a(`, then `d`, on one line of `(`, `)`, `a`, `b` and spaces. Where ten pairs enclose the cursor, none of the 3,000 differ. On random lines, 286 differ, and each has a step where no pair encloses the selection's start. Vim then looks forward for a pair (below), and Helm keeps the selection. A Python model of Helm's rule gives Helm's result in all 6,000 cases. The scripts stayed in the scratch directory and are not in the PR.

Escaped quotes:

| Text | Keys | Vim 9.1 and Helm after T-114 | Helm on main |
| --- | --- | --- | --- |
| `"\"\|a\\\\\"b\""` | `di"` | `"\"\|\"b\""` | `"\"\|\""` |
| `"\"a\\\\\|\"b\""` | `di"` | `"\"\|\"b\""` | `"\"\|\""` |
| `"x\\\\\"\|a\"b\""` | `di"` | `"x\\\\\"\|\"b\""` | `"x\\\\\"a\"\|\""` |
| `"\"a\\\\\" \|x \"c\""` | `di"` | `"\"a\\\\\"\|\"c\""` | `"\"\|\"c\""` |
| `"\"\|a\\\\\\\"b\""` | `di"` | `"\"\|\""` | `"\"\|\""` |

### Differences found, kept

Each is listed under "Measured against Vim" in `vim.rs`:

- The 1 MiB cap (answer 1).
- With no pair around the cursor, Vim 9.1 looks for the next pair after it, even on a later line: `di(` from `b` in `"a b (c) d"` leaves `"a b () d"`. Helm finds nothing, in normal and visual modes, before and after this task. In a visual mode with no pair out from the selection's start, Vim can then select a pair inside the selection: `va(a(a(` from `c` in `"f(a(bc)d)e"` leaves `(bc)` selected. Helm keeps `(a(bc)d)` selected, by the rule that an object that finds nothing leaves the selection alone.
- With no unescaped quote before the cursor, Vim takes the line's first quote as the opening one even if it is escaped: `di"` from `a` in `"x\\\"a\"b\""` leaves `"x\\\"\"b\""`. Helm leaves `"x\\\"a\"\""`.
- A grown inner block of lines in a visual mode stops on the closing line's last char, where Vim takes its line break too, as for every visual end since T-108.

## Acceptance tests

| File | What it proves |
| --- | --- |
| `crates/helm-editor/tests/vim_t114.rs` (13, new) | Answers 2 and 3 with the Vim rows above, the cap with a pair just inside and just outside 1 MiB each way, a count and a visual growth stopped by the cap, the forward direction of a grown selection, the kept differences, and the CRLF row for `p`. Ten of the 13 fail on main at `7638344`. The three that pass there are the CRLF `p` test, the capped visual growth (main keeps that selection too) and the escaped quote that never opens a pair (main counts a single backslash too). |

Every existing test passes unchanged.

## Performance budget

The cap bounds `vim_objects/di_paren_no_pair_10mb`. Its single cursor sits about 5 MiB into the text, so the backward scan reads 1 MiB, not about 5 MiB, and the benchmark should get faster. The PR reports its CI criterion means from the bench job's artifact against base. Every other benchmark stays within the default 5% gate in `budgets.toml`. No budget changes.

The doc comment of `crates/helm-bench/benches/vim_objects.rs` still says this benchmark scans to the text's start. That file is protected, so this task leaves it and the PR description proposes the new wording.

## Out of scope

- Vim's forward search for a pair when none encloses the cursor. That is a question for Khakha.
- Feedback for a refused put, a later `helm-ui` task (answer 4).
- Visual ends on a line break, kept since T-108.
