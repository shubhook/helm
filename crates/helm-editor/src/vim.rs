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
//! - A selection is never empty in a visual mode, except on the empty last
//!   line, where there is no char to cover: then both are at its offset.
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
//! the end of the line's text, which covers the same bytes.
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
//!   Then skip white space, stopping at the start of an empty line. Stop
//!   at the end of the text if nothing follows.
//! - `b`: move back one char. Skip white space backward, stopping at the
//!   start of an empty line. Then move back to the first char of the run
//!   the cursor is in. At offset 0 it stays.
//! - `e`: move forward one char. If that char is in the same run as the one
//!   before, go to the run's last char. Otherwise skip white space and go
//!   to the last char of the run after it. If there is no next run, `e`
//!   stays where it was.
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
//! A charwise operator with an empty range does nothing, and so does one
//! whose `e` or `E` doesn't move.
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
//!   `cw` and `cW` act as `ce` and `cE`, except that a cursor already on
//!   the last char of its word stays there, so `cw` changes just that
//!   char. On white space, `cw` is like `dw` and changes up to the next
//!   word.
//! - **An exclusive range that ends at the start of a later line.** If the
//!   range begins at or before its line's first non-blank, it becomes
//!   linewise, over the lines from its first up to the one before its end.
//!   Otherwise its end moves back to the end of the previous line's text.
//!   So `dw` on an empty line deletes the line, and `db` at the start of a
//!   line, when `b` lands on the first word of the line before, deletes
//!   that whole line.
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
//! With several cursors, the ranges are joined where they overlap and
//! applied as one transaction, as
//! [`Editor::delete_backward`](crate::Editor::delete_backward) joins them.
//! A cursor whose own range is empty but which sits inside another
//! cursor's range ends where that range began.
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
//! as the group's first edit, with no indentation. `c` and its one-key
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
//! began. After `ctrl-r` it is the start of the selections from after the
//! change: after redoing `ihello<Esc>` the cursor is just past `hello`,
//! where Vim would put it on the `h`. A `Blocked` error comes back as
//! [`CommandError::Editor`](crate::CommandError::Editor), with nothing
//! changed by the step that failed.
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

use crate::Keymap;

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
    todo!()
}
