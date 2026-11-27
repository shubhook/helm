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

use crate::editor::{Entry, Operator};
use crate::motion::vim::VimMotion;
use crate::{Command, CommandContext, CommandError, CommandFn, CommandId, CommandRegistry};

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

// T-109. These ids aren't in `ALL` yet: whether `ALL` grows or a second
// list holds them is open question 2 of `tasks/109-vim-objects.md`, since
// T-108's `tests/vim_keymap.rs` pins `ALL` at 55 ids.

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
    let others: [(&str, &'static str, CommandFn); 24] = [
        (DELETE, "Vim: delete", |cx| operate(cx, Operator::Delete)),
        (CHANGE, "Vim: change", |cx| operate(cx, Operator::Change)),
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
        (NORMAL, "Vim: normal mode", |cx| {
            cx.editor.vim_leave_insert(cx.set)?;
            Ok(())
        }),
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
    for (id, title, run) in commands {
        registry
            .register(Command {
                id: CommandId::from(id),
                title,
                run,
            })
            .expect("Vim command ids are valid and distinct");
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

/// [`DELETE`] and [`CHANGE`]: `operator` with the invocation's motion, or
/// nothing at all if that isn't one of the motions here.
fn operate(cx: CommandContext<'_>, operator: Operator) -> Result<(), CommandError> {
    let motion = cx.args.motion.as_ref();
    if let Some(motion) = motion.and_then(|id| VimMotion::from_id(id.as_str())) {
        cx.editor
            .vim_operate(cx.set, operator, motion, cx.args.count)?;
    }
    Ok(())
}

/// A one-key edit: `operator` with a fixed motion.
fn one_key(
    cx: CommandContext<'_>,
    operator: Operator,
    motion: VimMotion,
) -> Result<(), CommandError> {
    cx.editor
        .vim_operate(cx.set, operator, motion, cx.args.count)?;
    Ok(())
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
    cx.editor.vim_edit_selections(cx.set, operator, lines)?;
    Ok(())
}
