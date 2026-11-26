//! The ids of the Vim commands (T-108), which
//! [`CommandRegistry::with_vim_commands`] registers next to the editor and
//! cursor commands. The [`vim`](crate::vim) keymap binds these, and its
//! module docs hold the rules each one follows.
//!
//! Every command runs at every selection of the editor and takes the
//! [`CommandContext::count`] it is given, where its rule says it uses one.
//! The motion commands are also the motions the two operators take as
//! [`CommandArgs::motion`]. An operator given a motion id that isn't one of
//! the sixteen motions here, or none, does nothing and returns `Ok`.
//!
//! [`CommandRegistry::with_vim_commands`]: crate::CommandRegistry::with_vim_commands
//! [`CommandContext::count`]: crate::CommandContext::count
//! [`CommandArgs::motion`]: crate::CommandArgs::motion

#![expect(
    dead_code,
    reason = "T-108 interface stubs; the implementation deletes this"
)]

use crate::CommandRegistry;

/// `h`: one char back within the line.
pub const LEFT: &str = "vim.left";
/// `l`: one char forward within the line.
pub const RIGHT: &str = "vim.right";
/// `j`: the line below, at the goal column.
pub const DOWN: &str = "vim.down";
/// `k`: the line above, at the goal column.
pub const UP: &str = "vim.up";
/// `w`: the next word's start.
pub const NEXT_WORD: &str = "vim.next_word";
/// `b`: this or the previous word's start.
pub const PREV_WORD: &str = "vim.prev_word";
/// `e`: this or the next word's end.
pub const WORD_END: &str = "vim.word_end";
/// `W`: the next WORD's start.
pub const NEXT_BIG_WORD: &str = "vim.next_big_word";
/// `B`: this or the previous WORD's start.
pub const PREV_BIG_WORD: &str = "vim.prev_big_word";
/// `E`: this or the next WORD's end.
pub const BIG_WORD_END: &str = "vim.big_word_end";
/// `0`: the line's start.
pub const LINE_START: &str = "vim.line_start";
/// `^`: the line's first non-blank.
pub const FIRST_NON_BLANK: &str = "vim.first_non_blank";
/// `$`: the end of the line's text, settled onto its last char in normal
/// mode.
pub const LINE_END: &str = "vim.line_end";
/// `gg`: the first line, or line `n`, at its first non-blank.
pub const FIRST_LINE: &str = "vim.first_line";
/// `G`: the last line, or line `n`, at its first non-blank.
pub const LAST_LINE: &str = "vim.last_line";
/// The second `d` of `dd` and `c` of `cc`: `n` lines from the cursor's,
/// linewise. Run on its own it moves to the first non-blank of the line
/// `n - 1` below, as Vim's `_` does.
pub const LINE: &str = "vim.line";

/// `h` in a visual mode: moves the visual cursor as [`LEFT`] moves a
/// cursor.
pub const SELECT_LEFT: &str = "vim.select_left";
/// `l` in a visual mode.
pub const SELECT_RIGHT: &str = "vim.select_right";
/// `j` in a visual mode.
pub const SELECT_DOWN: &str = "vim.select_down";
/// `k` in a visual mode.
pub const SELECT_UP: &str = "vim.select_up";
/// `w` in a visual mode.
pub const SELECT_NEXT_WORD: &str = "vim.select_next_word";
/// `b` in a visual mode.
pub const SELECT_PREV_WORD: &str = "vim.select_prev_word";
/// `e` in a visual mode.
pub const SELECT_WORD_END: &str = "vim.select_word_end";
/// `W` in a visual mode.
pub const SELECT_NEXT_BIG_WORD: &str = "vim.select_next_big_word";
/// `B` in a visual mode.
pub const SELECT_PREV_BIG_WORD: &str = "vim.select_prev_big_word";
/// `E` in a visual mode.
pub const SELECT_BIG_WORD_END: &str = "vim.select_big_word_end";
/// `0` in a visual mode.
pub const SELECT_LINE_START: &str = "vim.select_line_start";
/// `^` in a visual mode.
pub const SELECT_FIRST_NON_BLANK: &str = "vim.select_first_non_blank";
/// `$` in a visual mode.
pub const SELECT_LINE_END: &str = "vim.select_line_end";
/// `gg` in a visual mode.
pub const SELECT_FIRST_LINE: &str = "vim.select_first_line";
/// `G` in a visual mode.
pub const SELECT_LAST_LINE: &str = "vim.select_last_line";

/// `d`: the delete operator.
pub const DELETE: &str = "vim.delete";
/// `c`: the change operator, which deletes and then types.
pub const CHANGE: &str = "vim.change";

/// `x`: `dl`.
pub const DELETE_CHAR: &str = "vim.delete_char";
/// `X`: `dh`.
pub const DELETE_CHAR_BEFORE: &str = "vim.delete_char_before";
/// `D`: `d$`.
pub const DELETE_TO_LINE_END: &str = "vim.delete_to_line_end";
/// `C`: `c$`.
pub const CHANGE_TO_LINE_END: &str = "vim.change_to_line_end";
/// `s`: `cl`.
pub const SUBSTITUTE: &str = "vim.substitute";
/// `S`: `cc`.
pub const SUBSTITUTE_LINE: &str = "vim.substitute_line";

/// `i`: type before the char under the cursor.
pub const INSERT: &str = "vim.insert";
/// `a`: type after the char under the cursor.
pub const APPEND: &str = "vim.append";
/// `I`: type before the line's first non-blank.
pub const INSERT_LINE_START: &str = "vim.insert_line_start";
/// `A`: type at the end of the line's text.
pub const APPEND_LINE_END: &str = "vim.append_line_end";
/// `o`: type on a new line below.
pub const OPEN_BELOW: &str = "vim.open_below";
/// `O`: type on a new line above.
pub const OPEN_ABOVE: &str = "vim.open_above";
/// Escape in insert mode: close the undo group and step back one char.
pub const NORMAL: &str = "vim.normal";

/// `v` and `V` in normal mode: select the char under each cursor.
pub const VISUAL: &str = "vim.visual";
/// Escape in a visual mode: a cursor on each visual cursor char.
pub const EXIT_VISUAL: &str = "vim.exit_visual";
/// `o` in a visual mode: swap the visual cursor and the anchor.
pub const SWAP_ENDS: &str = "vim.swap_ends";
/// `d` and `x` in visual mode: delete each selection.
pub const DELETE_SELECTION: &str = "vim.delete_selection";
/// `c` and `s` in visual mode: change each selection.
pub const CHANGE_SELECTION: &str = "vim.change_selection";
/// `d` and `x` in visual-line mode: delete each selection's lines.
pub const DELETE_LINES: &str = "vim.delete_lines";
/// `c` and `s` in visual-line mode: change each selection's lines.
pub const CHANGE_LINES: &str = "vim.change_lines";

/// `u`: undo `n` times.
pub const UNDO: &str = "vim.undo";
/// `ctrl-r`: redo `n` times.
pub const REDO: &str = "vim.redo";

/// Every id above, sorted.
pub const ALL: [&str; 55] = [
    APPEND,
    APPEND_LINE_END,
    BIG_WORD_END,
    CHANGE,
    CHANGE_LINES,
    CHANGE_SELECTION,
    CHANGE_TO_LINE_END,
    DELETE,
    DELETE_CHAR,
    DELETE_CHAR_BEFORE,
    DELETE_LINES,
    DELETE_SELECTION,
    DELETE_TO_LINE_END,
    DOWN,
    EXIT_VISUAL,
    FIRST_LINE,
    FIRST_NON_BLANK,
    INSERT,
    INSERT_LINE_START,
    LAST_LINE,
    LEFT,
    LINE,
    LINE_END,
    LINE_START,
    NEXT_BIG_WORD,
    NEXT_WORD,
    NORMAL,
    OPEN_ABOVE,
    OPEN_BELOW,
    PREV_BIG_WORD,
    PREV_WORD,
    REDO,
    RIGHT,
    SELECT_BIG_WORD_END,
    SELECT_DOWN,
    SELECT_FIRST_LINE,
    SELECT_FIRST_NON_BLANK,
    SELECT_LAST_LINE,
    SELECT_LEFT,
    SELECT_LINE_END,
    SELECT_LINE_START,
    SELECT_NEXT_BIG_WORD,
    SELECT_NEXT_WORD,
    SELECT_PREV_BIG_WORD,
    SELECT_PREV_WORD,
    SELECT_RIGHT,
    SELECT_UP,
    SELECT_WORD_END,
    SUBSTITUTE,
    SUBSTITUTE_LINE,
    SWAP_ENDS,
    UNDO,
    UP,
    VISUAL,
    WORD_END,
];

/// Registers one command for each id in [`ALL`], each with a non-empty
/// title.
pub(crate) fn register_all(registry: &mut CommandRegistry) {
    let _ = registry;
    todo!()
}
