//! Vim's keymap (ED-5, T-108): normal, insert, visual and visual-line
//! modes, motions, the `d` and `c` operators, and counts, for every
//! command in [`vim_commands`](crate::vim_commands).
//!
//! The commands run on an [`Editor`](crate::Editor) and work at every
//! selection, so Vim's keys work with several cursors too. The rules below
//! are what the commands in `vim_commands` do. Where a rule differs from
//! Vim on purpose, it says so.
//!
//! # Modes
//!
//! | Mode | Typing | What it is |
//! | --- | --- | --- |
//! | [`NORMAL`] | [`Count`](crate::Typing::Count) | The initial mode. Keys are commands, digits make a count. |
//! | [`INSERT`] | [`Insert`](crate::Typing::Insert) | Keys type. Escape goes back to normal mode. |
//! | [`VISUAL`] | `Count` | `v`: a selection of characters. |
//! | [`VISUAL_LINE`] | `Count` | `V`: a selection of whole lines. |
//! | [`DELETE_PENDING`] | `Count` | `d` is waiting for its motion. |
//! | [`CHANGE_PENDING`] | `Count` | `c` is waiting for its motion. |
//!
//! The mode lives in the editor's [`KeyState`](crate::KeyState), not in
//! the [`Editor`](crate::Editor). The commands don't know the mode: each
//! mode binds its own commands. `d` and `c` each have their own pending
//! mode so that the motion bindings in [`CHANGE_PENDING`] can switch to
//! insert mode when they complete, with `then_mode`, and no command has to
//! ask for a mode switch.
//!
//! # Where the cursor is
//!
//! Vim's normal-mode cursor is a block on a char. Here a normal-mode
//! cursor is an empty [`Selection`](crate::Selection) at the offset of the
//! char it is on, so `|` in `a|bc` is on `b`. A line break is LF, CRLF or a
//! lone CR, as `helm-text` defines it, and CRLF is always one char.
//!
//! A *normal-mode position* is a char boundary that isn't inside a CRLF and
//! isn't at the end of the text of a line that has text. So the cursor is
//! never right before a line break unless the line is empty, and never at
//! the end of the text unless the last line is empty. An empty line's
//! position is its start, where its line break is. In `"ab\n"` the empty
//! last line's position is offset 3, the end of the text. (Vim reads a
//! final line break as the end of the last line, not as a new empty line.
//! Helm keeps the text's own lines, as T-107 does.)
//!
//! *Settling* an offset makes it a normal-mode position: an offset between
//! a CR and its LF moves before the CR, and then an offset at the end of a
//! line's text, on a line that has text, moves back onto the line's last
//! char.
//!
//! Every command that runs in normal mode first clamps the selections, as
//! every `Editor` command does, then turns each selection into a cursor at
//! its settled head. After it runs, every cursor is at a normal-mode
//! position, except where a command enters insert mode. So a cursor left
//! at the end of a line by another keymap settles onto the line's last
//! char before the first Vim motion moves it.
//!
//! # Visual selections
//!
//! A visual selection is stored as the bytes it covers, so the selection
//! `helm-ui` draws and the text an edit removes are the same thing. Vim's
//! visual cursor and its anchor are each on a char at one end:
//!
//! - Pointing forward (anchor before head), the anchor char starts at the
//!   anchor, and the visual cursor is the char that ends at the head.
//! - Pointing back (head before anchor), the visual cursor starts at the
//!   head, and the anchor char ends at the anchor.
//! - The visual cursor and the anchor are always on a char, so every
//!   selection in a visual mode covers at least one char and decodes to
//!   one pair of ends. The one exception is `v` or `V` on the empty last
//!   line, below.
//!
//! So `v` on `b` in `abc` gives the selection `1..2`, written `a^b|c`, and
//! `l` then gives `1..3`, `a^bc|`. On an empty line the selection covers its
//! line break, both bytes of a CRLF.
//!
//! A visual motion moves the visual cursor the way the same motion moves a
//! normal-mode cursor, settled, and the selection is rebuilt from the
//! anchor char to the new cursor char. When the cursor passes the anchor,
//! the selection flips direction and still covers both chars. Visual-line
//! mode keeps the same selection of chars, and its `d` and `c` act on every
//! line from the anchor char's line to the visual cursor's line. `helm-ui`
//! draws whole lines in that mode. `v` and `V` switch between the two
//! modes without changing the selection. `$` in a visual mode stops on the
//! line's last char, where Vim's also takes the line break.
//!
//! **The empty last line.** The empty line after a final line break, at
//! the end of the text, has no char to cover. If it could be a visual end,
//! `0..4` in `"a\nb\n"` would mean both "visual cursor on `b`" and "visual
//! cursor on the empty last line", and with an empty line before it two
//! ends would cover the same bytes. So in a visual mode it is never an
//! end:
//!
//! - A visual motion whose settled target is the empty last line lands on
//!   the line before it instead: on that line's last char, or on its line
//!   break if it is empty. So `v2j` and `vG` on `"|a\nb\n"` select `a\nb`.
//! - `v` or `V` with the cursor on the empty last line gives an empty
//!   selection at the end of the text, the only empty visual selection in a
//!   text that isn't empty. Its visual cursor and anchor are both at the end
//!   of the text. The first visual motion that moves the cursor also moves
//!   the anchor onto the line before, by the same rule. A motion that can't
//!   move (`j` there) leaves it empty, and Escape leaves a cursor there.
//!
//! Vim has no such line, since it reads a final line break as the end of
//! the last line, so no Vim behaviour is lost.
//!
//! # Motions
//!
//! Each motion is one command id. Run on its own in normal mode, it moves
//! every cursor. Given to an operator as
//! [`CommandArgs::motion`](crate::CommandArgs::motion), it says what the
//! operator acts on. In the visual modes the `vim.select_` command of the
//! same name moves the visual cursor instead.
//!
//! `n` below is the count, 1 when none was typed. Motions that repeat stop
//! early as soon as a step doesn't move, so a huge count costs no more than
//! the text it crosses.
//!
//! | Key | Id | Where it goes | Count | Kind |
//! | --- | --- | --- | --- | --- |
//! | `h` | `vim.left` | One char back, not before the line's start | repeats | exclusive |
//! | `l` | `vim.right` | One char forward, not past the end of the line's text | repeats | exclusive |
//! | `j` | `vim.down` | The line below, at the goal column | `n` lines, up to the last | linewise |
//! | `k` | `vim.up` | The line above, at the goal column | `n` lines, up to the first | linewise |
//! | `w` | `vim.next_word` | The start of the next word | repeats | exclusive |
//! | `b` | `vim.prev_word` | The start of this word, or the one before | repeats | exclusive |
//! | `e` | `vim.word_end` | The end of this word, or the next | repeats | inclusive |
//! | `W` `B` `E` | `vim.next_big_word`, `vim.prev_big_word`, `vim.big_word_end` | As `w` `b` `e`, by WORDs | repeats | as `w` `b` `e` |
//! | `0` | `vim.line_start` | The line's start | none | exclusive |
//! | `^` | `vim.first_non_blank` | The line's first non-blank | none | exclusive |
//! | `$` | `vim.line_end` | The end of the line's text | `n - 1` lines down first | exclusive |
//! | `g g` | `vim.first_line` | The first non-blank of line `n`, or of the first line | line number | linewise |
//! | `G` | `vim.last_line` | The first non-blank of line `n`, or of the last line | line number | linewise |
//! | `d` `c` | `vim.line` | The first non-blank of the line `n - 1` below | lines | linewise |
//!
//! Line numbers count from 1, and a number past the last line means the
//! last line. A count of 0 can't be typed, since `0` alone is a motion. `vim.line`
//! is bound only in the two pending modes, as the second `d` of `dd` and
//! the second `c` of `cc`.
//!
//! In normal mode every motion's target is settled. So `l` on a line's last
//! char stays, `$` lands on the last char, and `j` to a shorter line lands
//! on its last char. Under an operator the target isn't settled: `dl` on
//! the last char deletes it, and `d$` deletes to the end of the line's
//! text. Vim's `$` is inclusive of the last char. Here it is exclusive of
//! the end of the line's text, which covers the same bytes. A counted `$`
//! that ends on an empty line is the one case where that differs; see
//! "Operators".
//!
//! `j` and `k` on the last or first line don't move, and under an operator
//! they then do nothing. `j` and `k` keep a goal column, in chars, exactly
//! as T-107's [`Motion::Down`](crate::Motion::Down) and
//! [`Motion::Up`](crate::Motion::Up) do, in normal and in visual modes: the
//! first `j` or `k` sets it to the cursor's column, later ones aim for it,
//! and any other command drops it. Vim's sticky end of line after `$` is
//! left out.
//!
//! ## Words
//!
//! A *word* is a run of chars of one class. The classes are T-107's:
//! word chars (`char::is_alphanumeric`, or `_`), blanks (whitespace that
//! isn't a line break) and punctuation (every other char, `😀` included).
//! A *WORD* is a run of chars that aren't blanks or line breaks. Line
//! breaks and blanks are both *white space* for the word motions. An empty
//! line counts as a word for `w` and `b`, but not for `e`. Vim gives CJK
//! ideographs a class of their own; here `中文` is a word like `ab`.
//!
//! - `w`: if the char under the cursor isn't white space, skip its run.
//!   Then skip white space, stopping at the start of an empty line other
//!   than the one it started on. Stop at the end of the text if nothing
//!   follows. So from an empty line `w` goes to the next empty line or the
//!   next line's first non-blank, whichever comes first.
//! - `b`: move back one char. Skip white space backward, stopping at the
//!   start of an empty line. Then move back to the first char of the run
//!   the cursor is in. At offset 0 it stays.
//! - `e`: move forward one char. If that char is in the same run as the one
//!   before, go to the run's last char. Otherwise skip white space and go
//!   to the last char of the run after it. If there is no next run, `e`
//!   stays where it was. A run is word or punctuation chars, never white
//!   space, so `e` on `a` in `a.(b` goes to `(`: `.(` is one run of
//!   punctuation.
//!
//! `^` on a line of only blanks goes to the end of its text, which settles
//! onto its last blank, as in Vim.
//!
//! # Operators
//!
//! `d` deletes and `c` changes: it deletes, then enters insert mode where
//! the deleted text was. Each takes a motion, and both run from every
//! cursor. The range a motion gives from cursor `p` to target `t`:
//!
//! - **Exclusive:** the bytes from the lower of `p` and `t` up to, not
//!   including, the higher.
//! - **Inclusive:** that range plus the char at the higher end. Only `e`
//!   and `E` are inclusive.
//! - **Linewise:** every line from `p`'s line to `t`'s.
//!
//! A charwise operator with an empty range does nothing. An `e` or `E`
//! that finds no next run, under an operator, ends on the char under the
//! cursor, so `de` on the last char of the text deletes it, as in Vim.
//!
//! Vim's special cases, from its documentation:
//!
//! - **`w` under an operator doesn't cross a line break.** The last of
//!   the `n` steps stops at the end of the line's text if it would move to
//!   a later line, so `dw` on a line's last word leaves the line break. If
//!   that step starts at the end of a line's text, as it does on an empty
//!   line, it stops at the start of the next line instead. The same goes
//!   for `W`.
//! - **`cw` is `ce`.** When the char under the cursor isn't white space,
//!   `cw` and `cW` act as `ce` and `cE`, except that the first of the `n`
//!   steps doesn't move a cursor already on the last char of its word. So
//!   `cw` there changes just that char, and `c2w` on the last `o` of `foo
//!   bar` changes `o bar`. Later steps are plain `e` steps. On white
//!   space, `cw` is like `dw` and changes up to the next word.
//! - **An exclusive range that ends at the start of a later line.** If the
//!   range begins at or before its line's first non-blank, it becomes
//!   linewise, over the lines from its first up to the one before its end.
//!   Otherwise its end moves back to the end of the previous line's text.
//!   So `dw` on an empty line deletes the line, and `db` at the start of a
//!   line, when `b` lands on the first word of the line before, deletes
//!   that whole line. This rule is for exclusive motions only, never `$`.
//! - **A counted `$` that ends on an empty line.** Vim's `$` is inclusive,
//!   and an empty line has no char to include. If the range begins at or
//!   before its line's first non-blank, it becomes linewise over every line
//!   from its first to the empty one. Otherwise it runs to the empty
//!   line's offset, so its line break stays. So `2D` from `b` in
//!   `"abc\n\nd"` deletes `bc\n`, and from an indent it deletes both lines.
//!   This holds for `D`, `C`, `d$` and `c$`, measured in Vim 9.1.
//!
//! **Linewise delete** of lines `a` to `b` deletes from the start of `a` to
//! the start of the line after `b`. When `b` is the last line, it deletes
//! from the end of line `a - 1`'s text to the end of the text instead, so
//! no empty line is left behind. When `a` is the first line too, it
//! deletes everything. Each cursor then goes to the first non-blank of the
//! line now at its range's start, settled.
//!
//! **Linewise change** of lines `a` to `b` deletes from the start of `a` to
//! the end of `b`'s text, keeping `b`'s line break, so one empty line is
//! left to type into. No indentation is kept: Vim's `autoindent` is left
//! out, as indentation settings are.
//!
//! After a **charwise delete** each cursor is at its range's start,
//! settled. After a **change** each cursor is at its range's start, in
//! insert mode.
//!
//! With several cursors, linewise spans from different cursors that
//! overlap or touch (the lines `a..=b` and `b + 1..=c`) join into one span
//! first, before the rules above turn lines into bytes. So `dd` at cursors
//! on two neighbouring lines does exactly what `2dd` at the first does,
//! the last line included. Then the byte ranges are joined where they
//! overlap and applied as one transaction, as
//! [`Editor::delete_backward`](crate::Editor::delete_backward) joins them.
//! A cursor whose own range is empty but which sits inside another
//! cursor's range ends where that range began.
//!
//! **A `c` whose motion finds nothing still enters insert mode.** `ch` at a
//! line's start, `cj` on the last line and `ck` on the first delete
//! nothing, open an undo group and type where the cursor is, so the typing
//! undoes in one step. Vim does this for `ch` and `c0`, but beeps and stays
//! in normal mode for `cj` and `ck`. Helm can't match that without a
//! command asking for a mode switch: `change_pending`'s bindings switch to
//! insert mode before the command runs. That is an open question for
//! Khakha.
//!
//! # One-key edits
//!
//! | Key | Id | Same as |
//! | --- | --- | --- |
//! | `x` | `vim.delete_char` | `dl` |
//! | `X` | `vim.delete_char_before` | `dh` |
//! | `D` | `vim.delete_to_line_end` | `d$` |
//! | `C` | `vim.change_to_line_end` | `c$` |
//! | `s` | `vim.substitute` | `cl` |
//! | `S` | `vim.substitute_line` | `cc` |
//!
//! Each takes the count its motion would: `3x` deletes up to three chars,
//! not past the end of the line's text, and `2D` deletes to the end of the
//! next line.
//!
//! # Insert mode
//!
//! Each of these opens an undo group (see [`Editor`](crate::Editor),
//! "Undo groups") and enters insert mode. They ignore a count: `3ix` with
//! its repeat is T-109's.
//!
//! | Key | Id | Where typing starts |
//! | --- | --- | --- |
//! | `i` | `vim.insert` | Before the char under the cursor |
//! | `a` | `vim.append` | After it, or at the line's start if the line is empty |
//! | `I` | `vim.insert_line_start` | Before the line's first non-blank |
//! | `A` | `vim.append_line_end` | At the end of the line's text |
//! | `o` | `vim.open_below` | On a new empty line below |
//! | `O` | `vim.open_above` | On a new empty line above |
//!
//! `o` and `O` insert [`FileFormat::newline`](helm_text::FileFormat::newline)
//! of the editor's format, at the end of the line's text or at its start,
//! as the group's first edit, with no indentation. Their undo record keeps
//! the cursors from before the key, so `u` puts each cursor back where it
//! was, as Vim does. (`A` moves the cursor before its group starts, so `u`
//! after `A` goes to the line's last char, also as Vim does.) `c` and its one-key
//! forms open the group before they delete, so the deletion and the typing
//! undo together. A visual `c` does the same.
//!
//! In insert mode keys type, and Enter, Backspace and Delete run
//! `editor.newline`, `editor.delete_backward` and `editor.delete_forward`.
//! Every edit folds into the open group. Escape runs `vim.normal`: it
//! closes the group, moves each cursor back one char unless it is at its
//! line's start, settles it, and returns to normal mode. So `ihello<Esc>`
//! then `u` removes all of `hello` in one step.
//!
//! Using `BufferSet::amend` for this now, rather than in a later task, is a
//! T-108 choice: Vim's `u` undoing one character at a time would be wrong
//! enough to fail ED-5's suite. When a group ends is entirely Vim's rule
//! (enter insert, leave insert), so it needs no timers. VS Code's grouping
//! is still a later task.
//!
//! # Undo and redo
//!
//! `u` (`vim.undo`) and `ctrl-r` (`vim.redo`) run
//! [`Editor::undo`](crate::Editor::undo) or
//! [`Editor::redo`](crate::Editor::redo) `n` times, stopping early when
//! there is nothing left, then turn each selection the editor put back into
//! a cursor at its start, settled. After `u` that is where the change
//! began: each selection's settled head for a normal-mode operator, a
//! one-key edit, `o` or `O`, and the selection's start for a visual edit.
//! After `ctrl-r` it is the start of the selections from after the
//! change: after redoing `ihello<Esc>` the cursor is just past `hello`,
//! where Vim puts it on the `h` (measured in Vim 9.1). A `Blocked` error
//! comes back as [`CommandError::Editor`](crate::CommandError::Editor),
//! with nothing changed by the step that failed. If that was the first
//! step, nothing changes at all: the selections and goal columns stay.
//!
//! # Measured against Vim
//!
//! Every row of the normal-mode tables in `tests/vim_normal.rs` and the
//! visual tables in `tests/vim_visual.rs` that ends in normal mode was run
//! through Vim 9.1 (`vim -u NONE -N`, `binary`), except rows whose text has
//! a CR, which Vim reads with a different line model. The rows that differ
//! are the choices this spec makes on purpose, or held for Khakha:
//!
//! - Columns count chars, so `j` from `文` in `中文x` lands on the second
//!   char of the line below, where Vim counts display cells (T-107's rule).
//! - `中文` is one word, where Vim gives CJK ideographs a class of their own.
//! - The empty last line after a final line break is a line here, and not
//!   in Vim.
//! - `ctrl-r` leaves the cursor after the redone text.
//! - `cj` and `ck` that find nothing enter insert mode.
//!
//! T-111 ran `d` and `c` with 58 motion and count forms from random
//! cursors in 20,000 short texts of ASCII lines through the same Vim, and
//! found five more kinds of difference, held for Khakha. Each follows the
//! rules above, and the oracle in `tests/vim_oracle.rs` pins each for `d`:
//!
//! - Vim's `:help d` exception: a charwise `d` across lines with only
//!   blanks before its start and after its end becomes linewise. `d2w`
//!   from `a` in `"  ab\n  "` leaves `""` in Vim and `"  "` here.
//! - A count of 2 or more for `$`, `dd` or `cc` on the last line is an
//!   error in Vim, which changes nothing and stays in normal mode. Here it
//!   acts on the last line: `2D` from `c` in `"a\nb\nc"` leaves `"a\nb\n"`,
//!   and `2$` moves to the line's last char.
//! - An `e` or `E` with no next run goes to the text's last char in Vim,
//!   and stays here. `de` from `.` in `"cd.  "` leaves `"cd"` in Vim and
//!   `"cd  "` here.
//! - A counted `b` or `B` under an operator that runs out on an empty
//!   first line cancels the operator in Vim. `d2b` from `x` in `"\nx"`
//!   leaves the text as it was in Vim and `"x"` here.
//! - `^` on a line of only blanks is on the last blank in Vim, so `d^`
//!   from the line's start keeps one blank. Here it deletes them all.
//!
//! Visual-mode `d` and `c` weren't part of that run.
//!
//! # Visual-mode commands
//!
//! | Id | Keys | What it does |
//! | --- | --- | --- |
//! | `vim.visual` | `v`, `V` in normal mode | Each cursor, settled, becomes a selection of its char |
//! | `vim.exit_visual` | Escape, and `v` in visual or `V` in visual-line mode | Each selection becomes a cursor on its visual cursor char, settled |
//! | `vim.swap_ends` | `o` | The visual cursor and the anchor swap chars, Vim's `o` |
//! | `vim.delete_selection` | `d`, `x` | Deletes each selection, then as a charwise delete |
//! | `vim.change_selection` | `c`, `s` | Opens a group, deletes each selection, then insert mode |
//! | `vim.delete_lines` | `d`, `x` in visual-line mode | A linewise delete of the selection's lines |
//! | `vim.change_lines` | `c`, `s` in visual-line mode | A linewise change of the selection's lines |
//!
//! Undoing a visual edit leaves a cursor where the selection began, in
//! normal mode, as `u` always does.

use crate::{Binding, Keymap, Mode, Typing, editor_commands as editor, vim_commands as ids};

/// The keymap's name.
pub const NAME: &str = "vim";

/// Normal mode, the initial mode.
pub const NORMAL: &str = "normal";
/// Insert mode, where keys type.
pub const INSERT: &str = "insert";
/// Visual mode, `v`.
pub const VISUAL: &str = "visual";
/// Visual-line mode, `V`.
pub const VISUAL_LINE: &str = "visual_line";
/// The mode `d` waits for its motion in.
pub const DELETE_PENDING: &str = "delete_pending";
/// The mode `c` waits for its motion in.
pub const CHANGE_PENDING: &str = "change_pending";

/// Vim's keymap: the six modes in the module docs, in that order, and the
/// 115 bindings below. Every binding is scoped to one mode and has no
/// context. Keys are in [`KeySequence`](crate::KeySequence) notation, where
/// Shift is folded into chars, so `G` is Shift+G and `$` is whatever key
/// types `$`.
///
/// The fifteen *motion keys* are `h` `l` `j` `k` `w` `b` `e` `W` `B` `E`
/// `0` `^` `$`, the chord `g g`, and `G`, with the ids in the module docs'
/// motion table.
///
/// **Normal mode** (33 bindings):
///
/// | Keys | Action | Then mode |
/// | --- | --- | --- |
/// | each motion key | run its motion | |
/// | `d` | operator `vim.delete` | `delete_pending` |
/// | `c` | operator `vim.change` | `change_pending` |
/// | `x`, `X`, `D` | `vim.delete_char`, `vim.delete_char_before`, `vim.delete_to_line_end` | |
/// | `u`, `ctrl-r` | `vim.undo`, `vim.redo` | |
/// | `C`, `s`, `S` | `vim.change_to_line_end`, `vim.substitute`, `vim.substitute_line` | `insert` |
/// | `i`, `a`, `I`, `A`, `o`, `O` | `vim.insert`, `vim.append`, `vim.insert_line_start`, `vim.append_line_end`, `vim.open_below`, `vim.open_above` | `insert` |
/// | `v` | `vim.visual` | `visual` |
/// | `V` | `vim.visual` | `visual_line` |
///
/// **Delete-pending mode** (16): each motion key runs its motion, and `d`
/// runs `vim.line`. With `d` pending, these give `Run` of `vim.delete`
/// with the motion, back in normal mode.
///
/// **Change-pending mode** (16): each motion key runs its motion and `c`
/// runs `vim.line`, each with then-mode `insert`. With `c` pending, they
/// give `Run` of `vim.change` with the motion, and the mode becomes
/// insert.
///
/// **Visual mode** (23) and **visual-line mode** (23):
///
/// | Keys | Visual | Visual-line | Then mode |
/// | --- | --- | --- | --- |
/// | each motion key | its `vim.select_` command | the same | |
/// | `escape` | `vim.exit_visual` | `vim.exit_visual` | `normal` |
/// | `v` | `vim.exit_visual` | no action | `normal`; `visual` |
/// | `V` | no action | `vim.exit_visual` | `visual_line`; `normal` |
/// | `o` | `vim.swap_ends` | `vim.swap_ends` | |
/// | `d`, `x` | `vim.delete_selection` | `vim.delete_lines` | `normal` |
/// | `c`, `s` | `vim.change_selection` | `vim.change_lines` | `insert` |
///
/// **Insert mode** (4): `escape` runs `vim.normal` then mode `normal`, and
/// `enter`, `backspace` and `delete` run `editor.newline`,
/// `editor.delete_backward` and `editor.delete_forward`.
///
/// Every command it binds is in
/// [`CommandRegistry::with_vim_commands`](crate::CommandRegistry::with_vim_commands).
/// Escape in normal mode and in the pending modes is unbound, so it
/// cancels a pending count or operator, as
/// [`Dispatcher::resolve`](crate::Dispatcher::resolve)'s rule 3 says.
pub fn keymap() -> Keymap {
    /// The motion keys, with the command each runs in normal and the
    /// pending modes, and in the visual modes.
    const MOTIONS: [(&str, &str, &str); 15] = [
        ("h", ids::LEFT, ids::SELECT_LEFT),
        ("l", ids::RIGHT, ids::SELECT_RIGHT),
        ("j", ids::DOWN, ids::SELECT_DOWN),
        ("k", ids::UP, ids::SELECT_UP),
        ("w", ids::NEXT_WORD, ids::SELECT_NEXT_WORD),
        ("b", ids::PREV_WORD, ids::SELECT_PREV_WORD),
        ("e", ids::WORD_END, ids::SELECT_WORD_END),
        ("W", ids::NEXT_BIG_WORD, ids::SELECT_NEXT_BIG_WORD),
        ("B", ids::PREV_BIG_WORD, ids::SELECT_PREV_BIG_WORD),
        ("E", ids::BIG_WORD_END, ids::SELECT_BIG_WORD_END),
        ("0", ids::LINE_START, ids::SELECT_LINE_START),
        ("^", ids::FIRST_NON_BLANK, ids::SELECT_FIRST_NON_BLANK),
        ("$", ids::LINE_END, ids::SELECT_LINE_END),
        ("g g", ids::FIRST_LINE, ids::SELECT_FIRST_LINE),
        ("G", ids::LAST_LINE, ids::SELECT_LAST_LINE),
    ];
    let run = |keys: &str, command: &str| Binding::run(keys, command).expect("the keys parse");
    let switch = |keys: &str, mode: &str| Binding::switch_mode(keys, mode).expect("the keys parse");

    let mut bindings = Vec::with_capacity(115);
    for (keys, motion, _) in MOTIONS {
        bindings.push(run(keys, motion).in_mode(NORMAL));
    }
    bindings.push(
        Binding::operator("d", ids::DELETE)
            .expect("the keys parse")
            .in_mode(NORMAL)
            .then_mode(DELETE_PENDING),
    );
    bindings.push(
        Binding::operator("c", ids::CHANGE)
            .expect("the keys parse")
            .in_mode(NORMAL)
            .then_mode(CHANGE_PENDING),
    );
    for (keys, command) in [
        ("x", ids::DELETE_CHAR),
        ("X", ids::DELETE_CHAR_BEFORE),
        ("D", ids::DELETE_TO_LINE_END),
        ("u", ids::UNDO),
        ("ctrl-r", ids::REDO),
    ] {
        bindings.push(run(keys, command).in_mode(NORMAL));
    }
    for (keys, command) in [
        ("C", ids::CHANGE_TO_LINE_END),
        ("s", ids::SUBSTITUTE),
        ("S", ids::SUBSTITUTE_LINE),
        ("i", ids::INSERT),
        ("a", ids::APPEND),
        ("I", ids::INSERT_LINE_START),
        ("A", ids::APPEND_LINE_END),
        ("o", ids::OPEN_BELOW),
        ("O", ids::OPEN_ABOVE),
    ] {
        bindings.push(run(keys, command).in_mode(NORMAL).then_mode(INSERT));
    }
    bindings.push(run("v", ids::VISUAL).in_mode(NORMAL).then_mode(VISUAL));
    bindings.push(run("V", ids::VISUAL).in_mode(NORMAL).then_mode(VISUAL_LINE));

    for (keys, motion, _) in MOTIONS {
        bindings.push(run(keys, motion).in_mode(DELETE_PENDING));
    }
    bindings.push(run("d", ids::LINE).in_mode(DELETE_PENDING));
    for (keys, motion, _) in MOTIONS {
        bindings.push(run(keys, motion).in_mode(CHANGE_PENDING).then_mode(INSERT));
    }
    bindings.push(
        run("c", ids::LINE)
            .in_mode(CHANGE_PENDING)
            .then_mode(INSERT),
    );

    for (mode, own, other_key, other, delete, change) in [
        (
            VISUAL,
            "v",
            "V",
            VISUAL_LINE,
            ids::DELETE_SELECTION,
            ids::CHANGE_SELECTION,
        ),
        (
            VISUAL_LINE,
            "V",
            "v",
            VISUAL,
            ids::DELETE_LINES,
            ids::CHANGE_LINES,
        ),
    ] {
        for (keys, _, select) in MOTIONS {
            bindings.push(run(keys, select).in_mode(mode));
        }
        bindings.push(
            run("escape", ids::EXIT_VISUAL)
                .in_mode(mode)
                .then_mode(NORMAL),
        );
        bindings.push(run(own, ids::EXIT_VISUAL).in_mode(mode).then_mode(NORMAL));
        bindings.push(switch(other_key, other).in_mode(mode));
        bindings.push(run("o", ids::SWAP_ENDS).in_mode(mode));
        for keys in ["d", "x"] {
            bindings.push(run(keys, delete).in_mode(mode).then_mode(NORMAL));
        }
        for keys in ["c", "s"] {
            bindings.push(run(keys, change).in_mode(mode).then_mode(INSERT));
        }
    }

    bindings.push(run("escape", ids::NORMAL).in_mode(INSERT).then_mode(NORMAL));
    bindings.push(run("enter", editor::NEWLINE).in_mode(INSERT));
    bindings.push(run("backspace", editor::DELETE_BACKWARD).in_mode(INSERT));
    bindings.push(run("delete", editor::DELETE_FORWARD).in_mode(INSERT));

    let modes = vec![
        Mode::new(NORMAL, Typing::Count),
        Mode::new(INSERT, Typing::Insert),
        Mode::new(VISUAL, Typing::Count),
        Mode::new(VISUAL_LINE, Typing::Count),
        Mode::new(DELETE_PENDING, Typing::Count),
        Mode::new(CHANGE_PENDING, Typing::Count),
    ];
    let mut keymap = Keymap::new(NAME, modes).expect("six modes, no duplicates");
    for binding in bindings {
        keymap
            .bind(binding)
            .expect("bindings name only the six modes");
    }
    keymap
}
