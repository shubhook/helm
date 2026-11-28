//! The ids of the Vim commands (T-108), which
//! [`CommandRegistry::with_vim_commands`] registers next to the editor and
//! cursor commands. The [`vim`](crate::vim) keymap binds these, and its
//! module docs hold the rules each one follows.
//!
//! Every command runs at every selection of the editor and takes the
//! [`CommandArgs::count`] it is given, where its rule says it uses one.
//! The motion commands are also the motions the two operators take as
//! [`CommandArgs::motion`]. An operator given a motion id that isn't one of
//! the sixteen motions here, or none, does nothing and returns `Ok`.
//!
//! [`CommandRegistry::with_vim_commands`]: crate::CommandRegistry::with_vim_commands
//! [`CommandArgs::count`]: crate::CommandArgs::count
//! [`CommandArgs::motion`]: crate::CommandArgs::motion

use std::path::PathBuf;
use std::sync::Arc;

use crate::editor::{Entry, Operator, Target};
use crate::motion::vim::VimMotion;
use crate::motion::vim::object::VimObject;
use crate::session::Write;
use crate::vim::ExError;
use crate::{
    Command, CommandContext, CommandError, CommandFn, CommandId, CommandRegistry, Invocation,
    Register, Request,
};

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

// T-109's 52 ids. They join `ALL`, as Khakha's answer to question 2 of
// `tasks/109-vim-objects.md` grants.

/// `y`: the yank operator, waiting in `yank_pending` for a motion or a
/// text object (T-109).
pub const YANK: &str = "vim.yank";
/// `Y`: yank `n` lines, as `yy` (T-109).
pub const YANK_LINE: &str = "vim.yank_line";
/// `p`: put the register after the cursor, or below its line (T-109).
pub const PUT_AFTER: &str = "vim.put_after";
/// `P`: put the register before the cursor, or above its line (T-109).
pub const PUT_BEFORE: &str = "vim.put_before";
/// `y` in visual mode: yank each selection (T-109).
pub const YANK_SELECTION: &str = "vim.yank_selection";
/// `y` in visual-line mode: yank each selection's lines (T-109).
pub const YANK_LINES: &str = "vim.yank_lines";
/// `p` in visual mode: replace each selection with the register, then
/// write the replaced text as a delete does (T-109).
pub const PUT_SELECTION: &str = "vim.put_selection";
/// `P` in visual mode: replace each selection with the register, writing
/// no register (T-109).
pub const PUT_SELECTION_KEEP: &str = "vim.put_selection_keep";
/// `p` in visual-line mode: replace each selection's lines (T-109).
pub const PUT_LINES: &str = "vim.put_lines";
/// `P` in visual-line mode: replace each selection's lines, writing no
/// register (T-109).
pub const PUT_LINES_KEEP: &str = "vim.put_lines_keep";
/// `.`: run the last change again (T-109).
pub const REPEAT: &str = "vim.repeat";
/// Enter in the command line: run the line, `:w`, `:q`, `:e` and the rest
/// (T-109).
pub const EX: &str = "vim.ex";

/// `iw` under an operator: inside the word, or the run of white space, under the cursor (T-109).
pub const INNER_WORD: &str = "vim.inner_word";
/// `aw` under an operator: all of the word, or the run of white space, under the cursor (T-109).
pub const A_WORD: &str = "vim.a_word";
/// `iw` in a visual mode: select inside the word, or the run of white space, under the cursor (T-109).
pub const SELECT_INNER_WORD: &str = "vim.select_inner_word";
/// `aw` in a visual mode: select all of the word, or the run of white space, under the cursor (T-109).
pub const SELECT_A_WORD: &str = "vim.select_a_word";
/// `iW` under an operator: inside the WORD, or the run of white space, under the cursor (T-109).
pub const INNER_BIG_WORD: &str = "vim.inner_big_word";
/// `aW` under an operator: all of the WORD, or the run of white space, under the cursor (T-109).
pub const A_BIG_WORD: &str = "vim.a_big_word";
/// `iW` in a visual mode: select inside the WORD, or the run of white space, under the cursor (T-109).
pub const SELECT_INNER_BIG_WORD: &str = "vim.select_inner_big_word";
/// `aW` in a visual mode: select all of the WORD, or the run of white space, under the cursor (T-109).
pub const SELECT_A_BIG_WORD: &str = "vim.select_a_big_word";
/// `i(`, `i)`, `ib` under an operator: inside the `(` `)` pair around the cursor (T-109).
pub const INNER_PAREN: &str = "vim.inner_paren";
/// `a(`, `a)`, `ab` under an operator: all of the `(` `)` pair around the cursor (T-109).
pub const A_PAREN: &str = "vim.a_paren";
/// `i(`, `i)`, `ib` in a visual mode: select inside the `(` `)` pair around the cursor (T-109).
pub const SELECT_INNER_PAREN: &str = "vim.select_inner_paren";
/// `a(`, `a)`, `ab` in a visual mode: select all of the `(` `)` pair around the cursor (T-109).
pub const SELECT_A_PAREN: &str = "vim.select_a_paren";
/// `i[`, `i]` under an operator: inside the `[` `]` pair around the cursor (T-109).
pub const INNER_BRACKET: &str = "vim.inner_bracket";
/// `a[`, `a]` under an operator: all of the `[` `]` pair around the cursor (T-109).
pub const A_BRACKET: &str = "vim.a_bracket";
/// `i[`, `i]` in a visual mode: select inside the `[` `]` pair around the cursor (T-109).
pub const SELECT_INNER_BRACKET: &str = "vim.select_inner_bracket";
/// `a[`, `a]` in a visual mode: select all of the `[` `]` pair around the cursor (T-109).
pub const SELECT_A_BRACKET: &str = "vim.select_a_bracket";
/// `i{`, `i}`, `iB` under an operator: inside the `{` `}` pair around the cursor (T-109).
pub const INNER_BRACE: &str = "vim.inner_brace";
/// `a{`, `a}`, `aB` under an operator: all of the `{` `}` pair around the cursor (T-109).
pub const A_BRACE: &str = "vim.a_brace";
/// `i{`, `i}`, `iB` in a visual mode: select inside the `{` `}` pair around the cursor (T-109).
pub const SELECT_INNER_BRACE: &str = "vim.select_inner_brace";
/// `a{`, `a}`, `aB` in a visual mode: select all of the `{` `}` pair around the cursor (T-109).
pub const SELECT_A_BRACE: &str = "vim.select_a_brace";
/// `i<`, `i>` under an operator: inside the `<` `>` pair around the cursor (T-109).
pub const INNER_ANGLE: &str = "vim.inner_angle";
/// `a<`, `a>` under an operator: all of the `<` `>` pair around the cursor (T-109).
pub const A_ANGLE: &str = "vim.a_angle";
/// `i<`, `i>` in a visual mode: select inside the `<` `>` pair around the cursor (T-109).
pub const SELECT_INNER_ANGLE: &str = "vim.select_inner_angle";
/// `a<`, `a>` in a visual mode: select all of the `<` `>` pair around the cursor (T-109).
pub const SELECT_A_ANGLE: &str = "vim.select_a_angle";
/// `i"` under an operator: inside the `"` quotes around or after the cursor on its line (T-109).
pub const INNER_DOUBLE_QUOTE: &str = "vim.inner_double_quote";
/// `a"` under an operator: all of the `"` quotes around or after the cursor on its line (T-109).
pub const A_DOUBLE_QUOTE: &str = "vim.a_double_quote";
/// `i"` in a visual mode: select inside the `"` quotes around or after the cursor on its line (T-109).
pub const SELECT_INNER_DOUBLE_QUOTE: &str = "vim.select_inner_double_quote";
/// `a"` in a visual mode: select all of the `"` quotes around or after the cursor on its line (T-109).
pub const SELECT_A_DOUBLE_QUOTE: &str = "vim.select_a_double_quote";
/// `i'` under an operator: inside the `'` quotes around or after the cursor on its line (T-109).
pub const INNER_SINGLE_QUOTE: &str = "vim.inner_single_quote";
/// `a'` under an operator: all of the `'` quotes around or after the cursor on its line (T-109).
pub const A_SINGLE_QUOTE: &str = "vim.a_single_quote";
/// `i'` in a visual mode: select inside the `'` quotes around or after the cursor on its line (T-109).
pub const SELECT_INNER_SINGLE_QUOTE: &str = "vim.select_inner_single_quote";
/// `a'` in a visual mode: select all of the `'` quotes around or after the cursor on its line (T-109).
pub const SELECT_A_SINGLE_QUOTE: &str = "vim.select_a_single_quote";
/// `` i` `` under an operator: inside the `` ` `` quotes around or after the cursor on its line (T-109).
pub const INNER_BACKTICK: &str = "vim.inner_backtick";
/// `` a` `` under an operator: all of the `` ` `` quotes around or after the cursor on its line (T-109).
pub const A_BACKTICK: &str = "vim.a_backtick";
/// `` i` `` in a visual mode: select inside the `` ` `` quotes around or after the cursor on its line (T-109).
pub const SELECT_INNER_BACKTICK: &str = "vim.select_inner_backtick";
/// `` a` `` in a visual mode: select all of the `` ` `` quotes around or after the cursor on its line (T-109).
pub const SELECT_A_BACKTICK: &str = "vim.select_a_backtick";
/// `ip` under an operator: inside the paragraph, or the run of blank lines, under the cursor (T-109).
pub const INNER_PARAGRAPH: &str = "vim.inner_paragraph";
/// `ap` under an operator: all of the paragraph, or the run of blank lines, under the cursor (T-109).
pub const A_PARAGRAPH: &str = "vim.a_paragraph";
/// `ip` in a visual mode: select inside the paragraph, or the run of blank lines, under the cursor (T-109).
pub const SELECT_INNER_PARAGRAPH: &str = "vim.select_inner_paragraph";
/// `ap` in a visual mode: select all of the paragraph, or the run of blank lines, under the cursor (T-109).
pub const SELECT_A_PARAGRAPH: &str = "vim.select_a_paragraph";

/// Every id above, sorted.
pub const ALL: [&str; 107] = [
    A_ANGLE,
    A_BACKTICK,
    A_BIG_WORD,
    A_BRACE,
    A_BRACKET,
    A_DOUBLE_QUOTE,
    A_PARAGRAPH,
    A_PAREN,
    A_SINGLE_QUOTE,
    A_WORD,
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
    EX,
    EXIT_VISUAL,
    FIRST_LINE,
    FIRST_NON_BLANK,
    INNER_ANGLE,
    INNER_BACKTICK,
    INNER_BIG_WORD,
    INNER_BRACE,
    INNER_BRACKET,
    INNER_DOUBLE_QUOTE,
    INNER_PARAGRAPH,
    INNER_PAREN,
    INNER_SINGLE_QUOTE,
    INNER_WORD,
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
    PUT_AFTER,
    PUT_BEFORE,
    PUT_LINES,
    PUT_LINES_KEEP,
    PUT_SELECTION,
    PUT_SELECTION_KEEP,
    REDO,
    REPEAT,
    RIGHT,
    SELECT_A_ANGLE,
    SELECT_A_BACKTICK,
    SELECT_A_BIG_WORD,
    SELECT_A_BRACE,
    SELECT_A_BRACKET,
    SELECT_A_DOUBLE_QUOTE,
    SELECT_A_PARAGRAPH,
    SELECT_A_PAREN,
    SELECT_A_SINGLE_QUOTE,
    SELECT_A_WORD,
    SELECT_BIG_WORD_END,
    SELECT_DOWN,
    SELECT_FIRST_LINE,
    SELECT_FIRST_NON_BLANK,
    SELECT_INNER_ANGLE,
    SELECT_INNER_BACKTICK,
    SELECT_INNER_BIG_WORD,
    SELECT_INNER_BRACE,
    SELECT_INNER_BRACKET,
    SELECT_INNER_DOUBLE_QUOTE,
    SELECT_INNER_PARAGRAPH,
    SELECT_INNER_PAREN,
    SELECT_INNER_SINGLE_QUOTE,
    SELECT_INNER_WORD,
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
    YANK,
    YANK_LINE,
    YANK_LINES,
    YANK_SELECTION,
];

/// The commands Vim's `.` repeats (T-109).
const CHANGES: [&str; 16] = [
    DELETE,
    CHANGE,
    DELETE_CHAR,
    DELETE_CHAR_BEFORE,
    DELETE_TO_LINE_END,
    CHANGE_TO_LINE_END,
    SUBSTITUTE,
    SUBSTITUTE_LINE,
    INSERT,
    APPEND,
    INSERT_LINE_START,
    APPEND_LINE_END,
    OPEN_BELOW,
    OPEN_ABOVE,
    PUT_AFTER,
    PUT_BEFORE,
];

/// The commands that start typing, whose count repeats what was typed
/// (T-109).
const ENTRIES: [&str; 6] = [
    INSERT,
    APPEND,
    INSERT_LINE_START,
    APPEND_LINE_END,
    OPEN_BELOW,
    OPEN_ABOVE,
];

/// Registers one command for each id in [`ALL`], each with a non-empty
/// title, and marks the changes `.` repeats.
pub(crate) fn register_all(registry: &mut CommandRegistry) {
    let motions: [(CommandFn, CommandFn, &str, &str); 16] = [
        (moves::<0>, selects::<0>, "Vim: left", "Vim: select left"),
        (moves::<1>, selects::<1>, "Vim: right", "Vim: select right"),
        (moves::<2>, selects::<2>, "Vim: down", "Vim: select down"),
        (moves::<3>, selects::<3>, "Vim: up", "Vim: select up"),
        (
            moves::<4>,
            selects::<4>,
            "Vim: next word",
            "Vim: select to next word",
        ),
        (
            moves::<5>,
            selects::<5>,
            "Vim: previous word",
            "Vim: select to previous word",
        ),
        (
            moves::<6>,
            selects::<6>,
            "Vim: end of word",
            "Vim: select to end of word",
        ),
        (
            moves::<7>,
            selects::<7>,
            "Vim: next WORD",
            "Vim: select to next WORD",
        ),
        (
            moves::<8>,
            selects::<8>,
            "Vim: previous WORD",
            "Vim: select to previous WORD",
        ),
        (
            moves::<9>,
            selects::<9>,
            "Vim: end of WORD",
            "Vim: select to end of WORD",
        ),
        (
            moves::<10>,
            selects::<10>,
            "Vim: start of line",
            "Vim: select to start of line",
        ),
        (
            moves::<11>,
            selects::<11>,
            "Vim: first non-blank",
            "Vim: select to first non-blank",
        ),
        (
            moves::<12>,
            selects::<12>,
            "Vim: end of line",
            "Vim: select to end of line",
        ),
        (
            moves::<13>,
            selects::<13>,
            "Vim: first line",
            "Vim: select to first line",
        ),
        (
            moves::<14>,
            selects::<14>,
            "Vim: last line",
            "Vim: select to last line",
        ),
        (moves::<15>, selects::<15>, "Vim: line", ""),
    ];
    let mut commands: Vec<(&str, &'static str, CommandFn)> = Vec::with_capacity(ALL.len());
    for (&(_, id, select), (run, run_select, title, select_title)) in
        VimMotion::ALL.iter().zip(motions)
    {
        commands.push((id, title, run));
        if let Some(select) = select {
            commands.push((select, select_title, run_select));
        }
    }
    let others: [(&str, &'static str, CommandFn); 36] = [
        (DELETE, "Vim: delete", |cx| operate(cx, Operator::Delete)),
        (CHANGE, "Vim: change", |cx| operate(cx, Operator::Change)),
        (YANK, "Vim: yank", yank),
        (YANK_LINE, "Vim: yank line", |cx| {
            let target = Target::Motion(VimMotion::Line);
            let register = cx.editor.vim_yank(cx.set, target, cx.args.count)?;
            write(cx, register, Write::Yank);
            Ok(())
        }),
        (PUT_AFTER, "Vim: put after", |cx| put(cx, true)),
        (PUT_BEFORE, "Vim: put before", |cx| put(cx, false)),
        (YANK_SELECTION, "Vim: yank selection", |cx| {
            yank_selections(cx, false)
        }),
        (YANK_LINES, "Vim: yank selected lines", |cx| {
            yank_selections(cx, true)
        }),
        (PUT_SELECTION, "Vim: put over selection", |cx| {
            put_selections(cx, false, false)
        }),
        (
            PUT_SELECTION_KEEP,
            "Vim: put over selection, keeping the register",
            |cx| put_selections(cx, false, true),
        ),
        (PUT_LINES, "Vim: put over selected lines", |cx| {
            put_selections(cx, true, false)
        }),
        (
            PUT_LINES_KEEP,
            "Vim: put over selected lines, keeping the register",
            |cx| put_selections(cx, true, true),
        ),
        (REPEAT, "Vim: repeat last change", repeat),
        (EX, "Vim: run command line", ex),
        (DELETE_CHAR, "Vim: delete char", |cx| {
            one_key(cx, Operator::Delete, VimMotion::Right)
        }),
        (DELETE_CHAR_BEFORE, "Vim: delete char before", |cx| {
            one_key(cx, Operator::Delete, VimMotion::Left)
        }),
        (DELETE_TO_LINE_END, "Vim: delete to end of line", |cx| {
            one_key(cx, Operator::Delete, VimMotion::LineEnd)
        }),
        (CHANGE_TO_LINE_END, "Vim: change to end of line", |cx| {
            one_key(cx, Operator::Change, VimMotion::LineEnd)
        }),
        (SUBSTITUTE, "Vim: substitute char", |cx| {
            one_key(cx, Operator::Change, VimMotion::Right)
        }),
        (SUBSTITUTE_LINE, "Vim: substitute line", |cx| {
            one_key(cx, Operator::Change, VimMotion::Line)
        }),
        (INSERT, "Vim: insert", |cx| enter(cx, Entry::Insert)),
        (APPEND, "Vim: append", |cx| enter(cx, Entry::Append)),
        (INSERT_LINE_START, "Vim: insert at line start", |cx| {
            enter(cx, Entry::LineStart)
        }),
        (APPEND_LINE_END, "Vim: append at line end", |cx| {
            enter(cx, Entry::LineEnd)
        }),
        (OPEN_BELOW, "Vim: open line below", |cx| {
            cx.editor.vim_open_line(cx.set, true)?;
            Ok(())
        }),
        (OPEN_ABOVE, "Vim: open line above", |cx| {
            cx.editor.vim_open_line(cx.set, false)?;
            Ok(())
        }),
        (NORMAL, "Vim: normal mode", leave_insert),
        (VISUAL, "Vim: visual mode", |cx| {
            cx.editor.vim_visual(cx.set)?;
            Ok(())
        }),
        (EXIT_VISUAL, "Vim: leave visual mode", |cx| {
            cx.editor.vim_exit_visual(cx.set)?;
            Ok(())
        }),
        (SWAP_ENDS, "Vim: go to other end of selection", |cx| {
            cx.editor.vim_swap_ends(cx.set)?;
            Ok(())
        }),
        (DELETE_SELECTION, "Vim: delete selection", |cx| {
            edit_selections(cx, Operator::Delete, false)
        }),
        (CHANGE_SELECTION, "Vim: change selection", |cx| {
            edit_selections(cx, Operator::Change, false)
        }),
        (DELETE_LINES, "Vim: delete selected lines", |cx| {
            edit_selections(cx, Operator::Delete, true)
        }),
        (CHANGE_LINES, "Vim: change selected lines", |cx| {
            edit_selections(cx, Operator::Change, true)
        }),
        (UNDO, "Vim: undo", |cx| {
            cx.editor.vim_history(cx.set, false, cx.args.count)?;
            Ok(())
        }),
        (REDO, "Vim: redo", |cx| {
            cx.editor.vim_history(cx.set, true, cx.args.count)?;
            Ok(())
        }),
    ];
    commands.extend(others);
    let objects: [(CommandFn, CommandFn, &str, &str); 20] = [
        (
            objects::<0>,
            selects_object::<0>,
            "Vim: inner word",
            "Vim: select inner word",
        ),
        (
            objects::<1>,
            selects_object::<1>,
            "Vim: a word",
            "Vim: select a word",
        ),
        (
            objects::<2>,
            selects_object::<2>,
            "Vim: inner WORD",
            "Vim: select inner WORD",
        ),
        (
            objects::<3>,
            selects_object::<3>,
            "Vim: a WORD",
            "Vim: select a WORD",
        ),
        (
            objects::<4>,
            selects_object::<4>,
            "Vim: inner parentheses",
            "Vim: select inner parentheses",
        ),
        (
            objects::<5>,
            selects_object::<5>,
            "Vim: a parentheses block",
            "Vim: select a parentheses block",
        ),
        (
            objects::<6>,
            selects_object::<6>,
            "Vim: inner brackets",
            "Vim: select inner brackets",
        ),
        (
            objects::<7>,
            selects_object::<7>,
            "Vim: a brackets block",
            "Vim: select a brackets block",
        ),
        (
            objects::<8>,
            selects_object::<8>,
            "Vim: inner braces",
            "Vim: select inner braces",
        ),
        (
            objects::<9>,
            selects_object::<9>,
            "Vim: a braces block",
            "Vim: select a braces block",
        ),
        (
            objects::<10>,
            selects_object::<10>,
            "Vim: inner angle brackets",
            "Vim: select inner angle brackets",
        ),
        (
            objects::<11>,
            selects_object::<11>,
            "Vim: an angle brackets block",
            "Vim: select an angle brackets block",
        ),
        (
            objects::<12>,
            selects_object::<12>,
            "Vim: inner double quotes",
            "Vim: select inner double quotes",
        ),
        (
            objects::<13>,
            selects_object::<13>,
            "Vim: a double-quoted string",
            "Vim: select a double-quoted string",
        ),
        (
            objects::<14>,
            selects_object::<14>,
            "Vim: inner single quotes",
            "Vim: select inner single quotes",
        ),
        (
            objects::<15>,
            selects_object::<15>,
            "Vim: a single-quoted string",
            "Vim: select a single-quoted string",
        ),
        (
            objects::<16>,
            selects_object::<16>,
            "Vim: inner backticks",
            "Vim: select inner backticks",
        ),
        (
            objects::<17>,
            selects_object::<17>,
            "Vim: a backtick string",
            "Vim: select a backtick string",
        ),
        (
            objects::<18>,
            selects_object::<18>,
            "Vim: inner paragraph",
            "Vim: select inner paragraph",
        ),
        (
            objects::<19>,
            selects_object::<19>,
            "Vim: a paragraph",
            "Vim: select a paragraph",
        ),
    ];
    for (&(_, id, select), (run, run_select, title, select_title)) in
        VimObject::ALL.iter().zip(objects)
    {
        commands.push((id, title, run));
        commands.push((select, select_title, run_select));
    }
    for (id, title, run) in commands {
        registry
            .register(Command {
                id: CommandId::from(id),
                title,
                run,
            })
            .expect("Vim command ids are valid and distinct");
    }
    for id in CHANGES {
        registry.mark_change(id);
    }
}

/// The motion at index `I` of [`VimMotion::ALL`], in normal mode.
fn moves<const I: usize>(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let (motion, _, _) = VimMotion::ALL[I];
    cx.editor.vim_move(cx.set, motion, cx.args.count)?;
    Ok(())
}

/// The motion at index `I` of [`VimMotion::ALL`], in a visual mode.
fn selects<const I: usize>(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let (motion, _, _) = VimMotion::ALL[I];
    cx.editor.vim_select(cx.set, motion, cx.args.count)?;
    Ok(())
}

/// What an operator's motion id names: one of the motions here, or one of
/// the text objects (T-109).
fn target(id: Option<&CommandId>) -> Option<Target> {
    let id = id?.as_str();
    VimMotion::from_id(id)
        .map(Target::Motion)
        .or_else(|| VimObject::from_id(id).map(Target::Object))
}

/// [`DELETE`] and [`CHANGE`]: `operator` with the invocation's motion or
/// object, or nothing at all if that isn't one of the motions or objects
/// here. Writes what it deleted to the registers (T-109).
fn operate(cx: CommandContext<'_>, operator: Operator) -> Result<(), CommandError> {
    if let Some(target) = target(cx.args.motion.as_ref()) {
        let register = cx
            .editor
            .vim_operate(cx.set, operator, target, cx.args.count)?;
        write(cx, register, Write::Delete);
    }
    Ok(())
}

/// A one-key edit: `operator` with a fixed motion.
fn one_key(
    cx: CommandContext<'_>,
    operator: Operator,
    motion: VimMotion,
) -> Result<(), CommandError> {
    let register =
        cx.editor
            .vim_operate(cx.set, operator, Target::Motion(motion), cx.args.count)?;
    write(cx, register, Write::Delete);
    Ok(())
}

/// Writes what a command took from the text to the register the
/// invocation names, or as `write` does with none (T-109).
fn write(cx: CommandContext<'_>, register: Option<Register>, write: Write) {
    if let Some(register) = register {
        let newline = cx.editor.format().newline();
        cx.session.write(
            cx.args.register,
            register,
            write,
            newline.as_str(),
            cx.requests,
        );
    }
}

/// [`YANK`]: yanks the invocation's motion or object at every cursor.
fn yank(cx: CommandContext<'_>) -> Result<(), CommandError> {
    if let Some(target) = target(cx.args.motion.as_ref()) {
        let register = cx.editor.vim_yank(cx.set, target, cx.args.count)?;
        write(cx, register, Write::Yank);
    }
    Ok(())
}

/// Visual `y`, on the selections or on their lines.
fn yank_selections(cx: CommandContext<'_>, lines: bool) -> Result<(), CommandError> {
    let register = cx.editor.vim_yank_selections(cx.set, lines)?;
    write(cx, register, Write::Yank);
    Ok(())
}

/// The register the invocation names, or the unnamed one. The session
/// shares its copy, so a 1 MB clipboard register isn't copied here.
fn named(cx: &CommandContext<'_>) -> Option<Arc<Register>> {
    cx.session.shared(cx.args.register.unwrap_or('"'))
}

/// [`PUT_AFTER`] and [`PUT_BEFORE`].
fn put(cx: CommandContext<'_>, after: bool) -> Result<(), CommandError> {
    if let Some(register) = named(&cx) {
        cx.editor.vim_put(cx.set, &register, after, cx.args.count)?;
    }
    Ok(())
}

/// Visual `p` and `P`: puts over the selections or their lines, then,
/// unless `keep`, writes what they held as a delete does.
fn put_selections(cx: CommandContext<'_>, lines: bool, keep: bool) -> Result<(), CommandError> {
    let register = named(&cx);
    let replaced =
        cx.editor
            .vim_put_selections(cx.set, register.as_deref(), lines, keep, cx.args.count)?;
    if !keep {
        let newline = cx.editor.format().newline();
        if let Some(replaced) = replaced {
            cx.session
                .write(None, replaced, Write::Delete, newline.as_str(), cx.requests);
        }
    }
    Ok(())
}

/// The object at index `I` of [`VimObject::ALL`] on its own: it does
/// nothing.
fn objects<const I: usize>(_: CommandContext<'_>) -> Result<(), CommandError> {
    Ok(())
}

/// The object at index `I` of [`VimObject::ALL`], in a visual mode.
fn selects_object<const I: usize>(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let (object, _, _) = VimObject::ALL[I];
    cx.editor.vim_select_object(cx.set, object, cx.args.count)?;
    Ok(())
}

/// Escape in insert mode (T-108). After a counted `i`, `a`, `I`, `A`, `o`
/// or `O`, first runs what was typed again, `n - 1` more times, inside the
/// same undo group (T-109).
fn leave_insert(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let typed = cx.session.recorded(cx.editor.id()).and_then(|record| {
        let (first, rest) = record.split_first()?;
        let n = first.args.count.filter(|&n| n > 1)?;
        if !ENTRIES.contains(&first.command.as_str()) {
            return None;
        }
        // The record ends with this Escape, which is already in it.
        let rest = match rest.split_last() {
            Some((last, typed)) if last.command.as_str() == NORMAL => typed,
            _ => rest,
        };
        let open = matches!(first.command.as_str(), OPEN_BELOW | OPEN_ABOVE);
        Some((n, open, rest.to_vec()))
    });
    if let Some((n, open, typed)) = typed {
        for _ in 1..n {
            if open {
                cx.editor.newline(cx.set)?;
            }
            for invocation in &typed {
                cx.registry.run_unrecorded(
                    invocation,
                    cx.editor,
                    cx.set,
                    cx.session,
                    cx.requests,
                )?;
            }
        }
    }
    cx.editor.vim_leave_insert(cx.set)?;
    Ok(())
}

/// [`REPEAT`]: runs the last change again, with the invocation's count in
/// place of its first count, as the `vim` module docs' "`.` repeat" says.
fn repeat(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let Some(change) = cx.session.last_change() else {
        return Ok(());
    };
    let mut change = change.to_vec();
    if let Some(first) = change.first_mut() {
        if cx.args.count.is_some() {
            first.args.count = cx.args.count;
        }
        if let Some(name @ '1'..='8') = first.args.register {
            first.args.register = char::from_digit(name.to_digit(10).unwrap_or(0) + 1, 10);
        }
    }
    for invocation in &change {
        let requests = cx
            .registry
            .run_in(invocation, cx.editor, cx.set, cx.session)?;
        cx.requests.extend(requests);
    }
    // A record that ended before Escape leaves its group open.
    if cx.editor.in_undo_group() {
        let requests =
            cx.registry
                .run_in(&Invocation::new(NORMAL), cx.editor, cx.set, cx.session)?;
        cx.requests.extend(requests);
    }
    Ok(())
}

/// [`EX`]: runs the command line in the invocation's text.
fn ex(cx: CommandContext<'_>) -> Result<(), CommandError> {
    let line = cx.args.text.as_deref().unwrap_or("");
    let requests = parse_ex(line).map_err(|error| CommandError::Ex {
        line: line.to_owned(),
        error,
    })?;
    cx.requests.extend(requests);
    Ok(())
}

/// The requests command line `line` makes, as the `vim` module docs' "The
/// command line" says.
fn parse_ex(line: &str) -> Result<Vec<Request>, ExError> {
    let line = line.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
    if line.is_empty() {
        return Ok(Vec::new());
    }
    if line.starts_with(|c: char| c.is_ascii_digit() || ",;.$%'".contains(c)) {
        return Err(ExError::Range);
    }
    let name_len = line
        .find(|c: char| !c.is_ascii_alphabetic())
        .unwrap_or(line.len());
    let (name, rest) = line.split_at(name_len);
    let (force, rest) = match rest.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, rest),
    };
    let path = rest.trim();
    let path = (!path.is_empty()).then(|| PathBuf::from(path));
    let is = |full: &str, shortest: usize| name.len() >= shortest && full.starts_with(name);
    if name == "wq" {
        Ok(vec![
            Request::Save { path, force },
            Request::Close { force: false },
        ])
    } else if is("write", 1) {
        Ok(vec![Request::Save { path, force }])
    } else if is("quit", 1) {
        if path.is_some() {
            return Err(ExError::TrailingCharacters);
        }
        Ok(vec![Request::Close { force }])
    } else if is("edit", 1) {
        Ok(vec![Request::Open { path, force }])
    } else {
        Err(ExError::NotACommand)
    }
}

/// `i`, `a`, `I` and `A`.
fn enter(cx: CommandContext<'_>, entry: Entry) -> Result<(), CommandError> {
    cx.editor.vim_enter_insert(cx.set, entry)?;
    Ok(())
}

/// The visual `d` and `c`, on the selections or on their lines.
fn edit_selections(
    cx: CommandContext<'_>,
    operator: Operator,
    lines: bool,
) -> Result<(), CommandError> {
    let register = cx.editor.vim_edit_selections(cx.set, operator, lines)?;
    write(cx, register, Write::Delete);
    Ok(())
}
