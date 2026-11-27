//! T-108 acceptance tests: Vim's normal and insert modes, typed key by key
//! through `vim::keymap()` and `CommandRegistry::with_vim_commands()` into
//! an `Editor` (ED-5). Motions, counts, the `d` and `c` operators, the
//! one-key edits, entering and leaving insert mode, and undo.
//!
//! Texts are written with markers, as in T-107's tests: `|` is a head and
//! `^` the anchor of the selection whose head is next to it. In normal mode
//! every selection is a cursor, and `|` sits right before the char the
//! block cursor is on. The first selection is the primary.
//!
//! Keys are Vim notation: each char is one key, and `<esc>`, `<cr>`,
//! `<bs>`, `<del>` and `<c-r>` are Escape, Enter, Backspace, Delete and
//! Ctrl+R.

use helm_editor::{
    CommandRegistry, Dispatcher, Editor, Key, KeyEvent, KeyState, Keystroke, Modifiers, Resolution,
    Selection, Selections, vim,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// The text with its markers removed, and the selections they mark, in
/// the order they appear.
fn parse(marked: &str) -> (String, Vec<Selection>) {
    let mut text = String::new();
    let mut markers: Vec<(char, usize)> = Vec::new();
    for c in marked.chars() {
        match c {
            '|' | '^' => markers.push((c, text.len())),
            _ => text.push(c),
        }
    }
    let mut paired = vec![false; markers.len()];
    let mut selections: Vec<(usize, Selection)> = Vec::new();
    for j in 0..markers.len() {
        if markers[j].0 != '^' {
            continue;
        }
        let head = if j + 1 < markers.len() && markers[j + 1].0 == '|' && !paired[j + 1] {
            j + 1
        } else if j > 0 && markers[j - 1].0 == '|' && !paired[j - 1] {
            j - 1
        } else {
            panic!(
                "`^` at {} has no `|` next to it in {marked:?}",
                markers[j].1
            );
        };
        paired[j] = true;
        paired[head] = true;
        selections.push((j.min(head), Selection::new(markers[j].1, markers[head].1)));
    }
    for (j, &(c, at)) in markers.iter().enumerate() {
        if c == '|' && !paired[j] {
            selections.push((j, Selection::cursor(at)));
        }
    }
    selections.sort_by_key(|&(j, _)| j);
    assert!(!selections.is_empty(), "no selection in {marked:?}");
    (text, selections.into_iter().map(|(_, s)| s).collect())
}

/// `text` with the markers for `selections`.
fn show(text: &str, selections: &Selections) -> String {
    let mut markers: Vec<(usize, usize, char)> = Vec::new();
    for (i, s) in selections.as_slice().iter().enumerate() {
        if s.is_empty() {
            markers.push((s.head, i, '|'));
        } else if s.is_reversed() {
            markers.push((s.head, i, '|'));
            markers.push((s.anchor, i, '^'));
        } else {
            markers.push((s.anchor, i, '^'));
            markers.push((s.head, i, '|'));
        }
    }
    markers.sort();
    let mut out = String::new();
    let mut at = 0;
    for (offset, _, c) in markers {
        out.push_str(&text[at..offset]);
        out.push(c);
        at = offset;
    }
    out.push_str(&text[at..]);
    out
}

/// The key events for keys in Vim notation.
fn events(keys: &str) -> Vec<KeyEvent> {
    let mut out = Vec::new();
    let mut chars = keys.chars();
    while let Some(c) = chars.next() {
        let key = if c == '<' {
            let name: String = chars.by_ref().take_while(|&c| c != '>').collect();
            match name.as_str() {
                "esc" => Keystroke::new(Key::Escape, Modifiers::NONE),
                "cr" => Keystroke::new(Key::Enter, Modifiers::NONE),
                "bs" => Keystroke::new(Key::Backspace, Modifiers::NONE),
                "del" => Keystroke::new(Key::Delete, Modifiers::NONE),
                "c-r" => Keystroke::new(Key::Char('r'), Modifiers::CTRL),
                other => panic!("unknown key <{other}>"),
            }
        } else {
            Keystroke::new(Key::Char(c), Modifiers::NONE)
        };
        out.push(KeyEvent::new(key));
    }
    out
}

/// One editor typing through Vim's keymap.
struct Vim {
    dispatcher: Dispatcher,
    state: KeyState,
    set: BufferSet,
    editor: Editor,
}

impl Vim {
    fn new(marked: &str) -> Self {
        let (text, selections) = parse(marked);
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(text.as_str()));
        let format = FileFormat {
            bom: false,
            line_endings: LineEndings::count(&text),
        };
        let mut editor = Editor::new(id, format);
        editor.set_selections(Selections::new(selections, 0).expect("non-empty"));
        let dispatcher = Dispatcher::new(CommandRegistry::with_vim_commands(), vim::keymap());
        let state = dispatcher.key_state();
        Self {
            dispatcher,
            state,
            set,
            editor,
        }
    }

    fn keys(&mut self, keys: &str) -> Vec<Resolution> {
        events(keys)
            .iter()
            .map(|event| {
                self.dispatcher
                    .dispatch(&mut self.state, event, &[], &mut self.editor, &mut self.set)
                    .expect("the command runs")
            })
            .collect()
    }

    fn text(&self) -> String {
        self.set
            .get(self.editor.buffer())
            .expect("in the set")
            .to_string()
    }

    fn show(&self) -> String {
        show(&self.text(), self.editor.selections())
    }

    fn mode(&self) -> &str {
        self.state.mode().as_str()
    }
}

/// `marked` after typing `keys`, with markers.
fn after(marked: &str, keys: &str) -> String {
    let mut v = Vim::new(marked);
    v.keys(keys);
    v.show()
}

/// Checks each row: start, keys, expected.
fn table(rows: &[(&str, &str, &str)]) {
    for &(start, keys, expected) in rows {
        assert_eq!(after(start, keys), expected, "{start:?} then {keys:?}");
    }
}

#[test]
fn h_and_l_move_within_the_line() {
    table(&[
        ("ab|c", "h", "a|bc"),
        ("|abc", "h", "|abc"),
        ("a\n|bc", "h", "a\n|bc"),
        ("a|bc", "l", "ab|c"),
        ("ab|c", "l", "ab|c"),
        ("ab|c\nd", "l", "ab|c\nd"),
        ("a|b\r\nc", "l", "a|b\r\nc"),
        ("a\n|\nb", "l", "a\n|\nb"),
        ("a\n|\nb", "h", "a\n|\nb"),
        ("|é中😀x", "l", "é|中😀x"),
        ("é中|😀x", "l", "é中😀|x"),
        ("é中😀|x", "h", "é中|😀x"),
    ]);
}

#[test]
fn counts_repeat_h_and_l_up_to_the_line_ends() {
    table(&[
        ("|abcdef", "3l", "abc|def"),
        ("|abc", "9l", "ab|c"),
        ("abcd|ef", "3h", "a|bcdef"),
        ("ab|c", "9h", "|abc"),
        ("|abcdefghijkl", "10l", "abcdefghij|kl"),
        ("|é中😀x\ny", "3l", "é中😀|x\ny"),
    ]);
}

#[test]
fn j_and_k_keep_the_column_on_a_char() {
    table(&[
        ("a|bc\nde\nfgh", "j", "abc\nd|e\nfgh"),
        ("abc\nd|e", "k", "a|bc\nde"),
        ("ab|c\nd", "j", "abc\n|d"),
        ("ab|c\n\nabc", "j", "abc\n|\nabc"),
        ("a|bc", "k", "a|bc"),
        ("a|bc", "j", "a|bc"),
        ("a|b\r\ncd", "j", "ab\r\nc|d"),
        ("中|文x\nabc", "j", "中文x\na|bc"),
        ("a|bc\r\nd\re", "jj", "abc\r\nd\r|e"),
    ]);
}

#[test]
fn j_and_k_keep_a_goal_column_through_short_lines() {
    table(&[
        ("ab|cd\nx\nefgh", "jj", "abcd\nx\nef|gh"),
        ("abcd\n\nef|gh", "kk", "ab|cd\n\nefgh"),
        ("abc|d\nx\nefgh", "jk", "abc|d\nx\nefgh"),
        // Another motion drops the goal.
        ("abc|d\nxy\nefgh", "jhj", "abcd\nxy\n|efgh"),
    ]);
}

#[test]
fn counts_move_j_and_k_by_lines_up_to_the_ends() {
    table(&[
        ("|a\nb\nc\nd", "2j", "a\nb\n|c\nd"),
        ("|a\nb", "9j", "a\n|b"),
        ("a\nb\n|c", "9k", "|a\nb\nc"),
        ("a\nb\nc\n|d", "2k", "a\n|b\nc\nd"),
    ]);
}

#[test]
fn w_goes_to_the_next_word_start() {
    table(&[
        ("|foo bar", "w", "foo |bar"),
        ("|foo.bar", "w", "foo|.bar"),
        ("foo|.bar", "w", "foo.|bar"),
        ("|foo_bar baz", "w", "foo_bar |baz"),
        ("|foo  \n  bar", "w", "foo  \n  |bar"),
        ("|foo\n\nbar", "w", "foo\n|\nbar"),
        ("foo\n|\nbar", "w", "foo\n\n|bar"),
        ("foo\n|\n\nbar", "w", "foo\n\n|\nbar"),
        ("|foo\r\nbar", "w", "foo\r\n|bar"),
        ("foo |bar", "w", "foo ba|r"),
        ("|é中 😀x", "w", "é中 |😀x"),
        ("é中 |😀x", "w", "é中 😀|x"),
        ("|a b c d", "3w", "a b c |d"),
        ("|a b", "9w", "a |b"),
    ]);
}

#[test]
fn b_goes_to_the_previous_word_start() {
    table(&[
        ("foo |bar", "b", "|foo bar"),
        ("foo ba|r", "b", "foo |bar"),
        ("foo.|bar", "b", "foo|.bar"),
        ("foo|.bar", "b", "|foo.bar"),
        ("foo\n  |bar", "b", "|foo\n  bar"),
        ("foo\n\n|bar", "b", "foo\n|\nbar"),
        ("foo\r\n|bar", "b", "|foo\r\nbar"),
        ("|foo", "b", "|foo"),
        ("a b c |d", "3b", "|a b c d"),
        ("a b c |d", "2b", "a |b c d"),
        ("a |b", "9b", "|a b"),
    ]);
}

#[test]
fn e_goes_to_the_end_of_the_word() {
    table(&[
        ("|foo bar", "e", "fo|o bar"),
        ("fo|o bar", "e", "foo ba|r"),
        ("|foo.bar", "e", "fo|o.bar"),
        ("fo|o\n\n  bar", "e", "foo\n\n  ba|r"),
        ("foo ba|r", "e", "foo ba|r"),
        ("|a b c", "2e", "a b |c"),
        ("|ab cd ef", "2e", "ab c|d ef"),
        ("|中文 x", "e", "中|文 x"),
    ]);
}

#[test]
fn big_words_split_only_at_blanks() {
    table(&[
        ("|foo.bar baz", "W", "foo.bar |baz"),
        ("foo.bar |baz", "B", "|foo.bar baz"),
        ("|foo.bar baz", "E", "foo.ba|r baz"),
        ("|a.b\n\nc", "W", "a.b\n|\nc"),
        ("|a.b c.d e", "2W", "a.b c.d |e"),
    ]);
}

#[test]
fn zero_caret_and_dollar() {
    table(&[
        ("  a|bc", "0", "|  abc"),
        ("  a|bc", "^", "  |abc"),
        ("|  abc", "^", "  |abc"),
        ("  |abc", "$", "  ab|c"),
        ("|abc\r\nd", "$", "ab|c\r\nd"),
        ("a\n|\nb", "$", "a\n|\nb"),
        ("|ab\ncd\nef", "2$", "ab\nc|d\nef"),
        ("|ab\ncd", "9$", "ab\nc|d"),
        ("|   \nx", "^", "  | \nx"),
        ("x\n\t|y", "0", "x\n|\ty"),
    ]);
}

#[test]
fn gg_and_g_go_to_a_line_and_its_first_non_blank() {
    table(&[
        ("a\n  b\n|c", "gg", "|a\n  b\nc"),
        ("|a\n  b\nc", "G", "a\n  b\n|c"),
        ("|a\n  b\nc", "2G", "a\n  |b\nc"),
        ("a\n  b\n|c", "2gg", "a\n  |b\nc"),
        ("|a\nb", "9G", "a\n|b"),
        ("|a\nb", "9gg", "a\n|b"),
        ("|a\nb\n", "G", "a\nb\n|"),
        ("a\n|b", "0G", "a\n|b"),
    ]);
}

#[test]
fn a_cursor_left_past_the_line_end_settles_first() {
    // An offset at the end of a line's text (left by another keymap) is
    // moved onto the last char before the motion runs.
    table(&[
        ("ab|\ncd", "h", "|ab\ncd"),
        ("ab|", "x", "|a"),
        ("a\r|\nb", "l", "|a\r\nb"),
    ]);
}

#[test]
fn dw_deletes_to_the_next_word_but_not_past_the_line_end() {
    table(&[
        ("|foo bar", "dw", "|bar"),
        ("foo |bar", "dw", "foo| "),
        ("foo |bar\nbaz", "dw", "foo| \nbaz"),
        ("|foo  \nbar", "dw", "|\nbar"),
        ("|foo\n\nbar", "dw", "|\n\nbar"),
        ("a\n|\nb", "dw", "a\n|b"),
        ("|a b c d", "d3w", "|d"),
        ("|a b c d", "3dw", "|d"),
        ("|a\nb c", "d2w", "|c"),
        ("|a b c d e f g", "2d3w", "|g"),
        ("|a.b c", "dW", "|c"),
        ("|foo", "dw", "|"),
    ]);
}

#[test]
fn db_and_de() {
    table(&[
        ("foo |bar", "db", "|bar"),
        ("foo.|bar", "db", "foo|bar"),
        // An exclusive motion ending at a line start, begun at or before
        // the first non-blank, takes whole lines.
        ("foo\n|bar", "db", "|bar"),
        // Begun after the first non-blank, it stops before the line break.
        ("x foo\n|bar", "db", "x| \nbar"),
        ("|foo bar", "de", "| bar"),
        ("fo|o bar", "de", "f|o"),
        ("|ab cd ef", "d2e", "| ef"),
        ("foo ba|r", "de", "foo b|a"),
        ("foo ba|r", "dE", "foo b|a"),
        ("foo ba|r", "ceX<esc>", "foo ba|X"),
        ("a |b", "de", "a| "),
        ("|a.b c", "dE", "| c"),
        ("a.b |c", "dB", "|c"),
    ]);
}

#[test]
fn d_with_line_motions_within_a_line() {
    table(&[
        ("a|bc\nd", "d$", "|a\nd"),
        ("a|bc\nd", "D", "|a\nd"),
        ("a|bc\nd\ne", "2D", "|a\ne"),
        ("a|b\r\ncd", "D", "|a\r\ncd"),
        ("ab|c", "d0", "|c"),
        ("  ab|c", "d^", "  |c"),
        ("|  abc", "d^", "|abc"),
        ("ab|c", "dh", "a|c"),
        ("ab|c", "dl", "a|b"),
        ("|abc", "dh", "|abc"),
        ("a\n|\nb", "d$", "a\n|\nb"),
    ]);
}

#[test]
fn x_and_capital_x_delete_chars_within_the_line() {
    table(&[
        ("ab|c", "x", "a|b"),
        ("|abc", "2x", "|c"),
        ("a|bc\nd", "9x", "|a\nd"),
        ("a\n|\nb", "x", "a\n|\nb"),
        ("|😀中é", "x", "|中é"),
        ("ab|c", "X", "a|c"),
        ("|abc", "X", "|abc"),
        ("abc|d", "9X", "|d"),
        ("a\r\n|b", "X", "a\r\n|b"),
    ]);
}

#[test]
fn dd_deletes_whole_lines() {
    table(&[
        ("a\n|b\nc", "dd", "a\n|c"),
        ("a\nb\n|c", "dd", "a\n|b"),
        ("|a", "dd", "|"),
        ("  a\n|b\n  c", "dd", "  a\n  |c"),
        ("|a\nb\nc", "2dd", "|c"),
        ("|a\nb\nc", "d2d", "|c"),
        ("|a\nb\nc", "9dd", "|"),
        ("a\r\n|b\r\nc", "dd", "a\r\n|c"),
        ("a\r\n|b", "dd", "|a"),
        ("a\n|b\n", "dd", "a\n|"),
    ]);
}

#[test]
fn d_with_j_k_gg_and_g_deletes_lines() {
    table(&[
        ("|a\nb\nc", "dj", "|c"),
        ("a\n|b\nc", "dk", "|c"),
        ("a\n|b", "dj", "a\n|b"),
        ("|a\nb", "dk", "|a\nb"),
        ("|a\nb\nc\nd", "d2j", "|d"),
        ("a\n|b\nc", "dG", "|a"),
        ("a\n|b\nc", "dgg", "|c"),
        ("a\nb\n|c\nd", "d2G", "a\n|d"),
    ]);
}

#[test]
fn a_huge_count_deletes_only_what_is_there() {
    let nines = "9".repeat(40);
    table(&[
        ("|abc\nd", &format!("{nines}x"), "|\nd"),
        ("|a b c", &format!("d{nines}w"), "|"),
        ("a\n|b\nc", &format!("{nines}dd"), "|a"),
        ("|a\nb", &format!("{nines}j"), "a\n|b"),
    ]);
}

#[test]
fn an_operator_waits_in_its_pending_mode() {
    let mut v = Vim::new("|foo bar");
    assert_eq!(v.keys("d"), [Resolution::Pending]);
    assert_eq!(v.mode(), vim::DELETE_PENDING);
    assert_eq!(v.keys("<esc>"), [Resolution::Cancelled]);
    assert_eq!(v.mode(), vim::NORMAL);
    assert_eq!(v.show(), "|foo bar");
    v.keys("c");
    assert_eq!(v.mode(), vim::CHANGE_PENDING);
    v.keys("q");
    assert_eq!(v.mode(), vim::NORMAL);
    v.keys("cw");
    assert_eq!(v.mode(), vim::INSERT);
    assert_eq!(v.show(), "| bar");
}

#[test]
fn cw_changes_to_the_end_of_the_word() {
    table(&[
        ("|foo bar", "cwxy<esc>", "x|y bar"),
        ("fo|o bar", "cwX<esc>", "fo|X bar"),
        ("|foo  bar", "c2wX<esc>", "|X"),
        ("foo|  bar", "cwX<esc>", "foo|Xbar"),
        ("|a.b c", "cWX<esc>", "|X c"),
        ("|foo bar", "ceX<esc>", "|X bar"),
        ("foo |bar", "cbX<esc>", "|Xbar"),
    ]);
}

#[test]
fn cc_s_and_c_change_lines_and_chars() {
    table(&[
        ("a\n  |b c\nd", "ccX<esc>", "a\n|X\nd"),
        ("|a\nb\nc", "cjX<esc>", "|X\nc"),
        ("|a\nb\nc", "2ccX<esc>", "|X\nc"),
        ("a\n|b", "cc<esc>", "a\n|"),
        ("a|bc", "CX<esc>", "a|X"),
        ("a|bc\nd", "2CX<esc>", "a|X"),
        ("a|bc", "sX<esc>", "a|Xc"),
        ("|abc", "3sX<esc>", "|X"),
        ("a|bc\nd", "SX<esc>", "|X\nd"),
        ("ab|c", "cl<esc>", "a|b"),
    ]);
}

#[test]
fn i_a_capital_i_and_capital_a_start_typing() {
    table(&[
        ("a|bc", "iX<esc>", "a|Xbc"),
        ("|abc", "i<esc>", "|abc"),
        ("a\n|b", "i<esc>", "a\n|b"),
        ("a|bc", "aX<esc>", "ab|Xc"),
        ("ab|c", "aX<esc>", "abc|X"),
        ("a\n|\nb", "aX<esc>", "a\n|X\nb"),
        ("  ab|c", "IX<esc>", "  |Xabc"),
        ("a|bc\nd", "AX<esc>", "abc|X\nd"),
        ("a|b\r\nc", "AX<esc>", "ab|X\r\nc"),
        ("a|bc", "a<esc>", "a|bc"),
    ]);
}

#[test]
fn o_and_capital_o_open_a_line() {
    table(&[
        ("a|bc\nd", "oX<esc>", "abc\n|X\nd"),
        ("a\nb|c", "OX<esc>", "a\n|X\nbc"),
        ("a|b", "o<esc>", "ab\n|"),
        ("|ab", "O<esc>", "|\nab"),
        ("a|b\r\nc", "oX<esc>", "ab\r\n|X\r\nc"),
        ("a|b\r\nc", "OX<esc>", "|X\r\nab\r\nc"),
    ]);
}

#[test]
fn insert_mode_types_and_edits() {
    table(&[
        ("|ab", "iX<cr>Y<bs>Z<esc>", "X\n|Zab"),
        ("a|b", "i<del><del>X<esc>", "a|X"),
        ("|ab", "i123<esc>", "12|3ab"),
        ("|ab", "ihjkl0wd<esc>", "hjkl0w|dab"),
        ("a|b\r\nc", "a<cr><esc>", "ab\r\n|\r\nc"),
    ]);
    let mut v = Vim::new("|ab");
    v.keys("i");
    assert_eq!(v.mode(), vim::INSERT);
    v.keys("x<esc>");
    assert_eq!(v.mode(), vim::NORMAL);
}

#[test]
fn u_undoes_a_whole_insert_or_change() {
    table(&[
        ("a|bc", "iXYZ<esc>u", "a|bc"),
        ("a|bc", "iX<cr>Y<bs><esc>u", "a|bc"),
        ("|foo bar", "cwxy<esc>u", "|foo bar"),
        ("a|b\nc", "oX<esc>u", "a|b\nc"),
        ("a\n|b\nc", "ddu", "a\n|b\nc"),
        ("|abc", "xxu", "|bc"),
        ("|abcd", "xxx3u", "|abcd"),
        ("|ab", "u", "|ab"),
        ("a\n|b c", "SX<esc>u", "a\n|b c"),
    ]);
}

#[test]
fn ctrl_r_redoes() {
    table(&[
        ("a|bc", "iXYZ<esc>u<c-r>", "aXYZ|bc"),
        ("|foo bar", "dwu<c-r>", "|bar"),
        ("|abcd", "xxx3u2<c-r>", "|cd"),
        ("a\n|b\nc", "ddu<c-r>", "a\n|c"),
    ]);
}

#[test]
fn every_cursor_runs_the_command() {
    table(&[
        ("|foo bar |baz qux", "dw", "|bar |qux"),
        ("a|b c|d", "0", "|ab cd"),
        ("|ab\n|cd", "AX<esc>", "ab|X\ncd|X"),
        ("|ab\n|cd", "dd", "|"),
        ("|ab\ncd\n|ef", "dd", "|cd"),
        ("|abc |def", "2x", "|c |f"),
        ("|a b\n|c d", "wD", "a| \nc| "),
    ]);
    let mut v = Vim::new("|foo |bar |baz");
    v.keys("cwX<esc>");
    assert_eq!(v.show(), "|X |X |X");
    v.keys("u");
    assert_eq!(v.show(), "|foo |bar |baz");
}

#[test]
fn counted_cw_skips_ahead_only_on_its_first_step() {
    // Checked against Vim 9.1 with `-u NONE`.
    table(&[
        ("fo|o bar", "c2wX<esc>", "fo|X"),
        ("|a b c", "cwX<esc>", "|X b c"),
        ("|a b c", "c2wX<esc>", "|X c"),
        ("a.|b c.d", "cWX<esc>", "a.|X c.d"),
        ("a.|b c.d e", "c2WX<esc>", "a.|X e"),
    ]);
}

#[test]
fn c_with_a_motion_that_finds_nothing_still_enters_insert() {
    // Vim does this for `ch` and `c0` at the line's start, and beeps for
    // `cj` and `ck` at the last and first lines; Helm types in all four.
    table(&[
        ("|ab", "chX<esc>", "|Xab"),
        ("|ab", "c0X<esc>", "|Xab"),
        ("a|b", "cjX<esc>", "a|Xb"),
        ("|a\nb", "ckX<esc>", "|Xa\nb"),
        ("a\n|b", "cjXY<esc>u", "a\n|b"),
        ("|ab", "chX<esc>u", "|ab"),
    ]);
    let mut v = Vim::new("a|b");
    v.keys("cj");
    assert_eq!(v.mode(), vim::INSERT);
    assert!(v.editor.in_undo_group());
}

#[test]
fn a_counted_dollar_that_ends_on_an_empty_line() {
    // Checked against Vim 9.1 with `-u NONE`.
    table(&[
        ("a|bc\n\nd", "2D", "|a\nd"),
        ("a|bc\n\nd", "2d$", "|a\nd"),
        ("|  bc\n\nd", "2D", "|d"),
        ("  |bc\n\nd", "2D", "|d"),
        ("a|bc\n\nd", "2CX<esc>", "a|X\nd"),
        ("  b|c\n\nd", "2c$X<esc>", "  b|X\nd"),
        ("|  bc\n\nd", "2CX<esc>", "|X\nd"),
        ("a|bc\r\n\r\nd", "2D", "|a\r\nd"),
        ("ab|c\nd", "2D", "a|b"),
    ]);
}

#[test]
fn cursors_on_touching_lines_delete_them_like_one_count() {
    table(&[
        ("ab\n|cd\n|ef", "dd", "|ab"),
        ("ab\n|cd\nef", "2dd", "|ab"),
        ("ab\r\n|cd\r\n|ef", "dd", "|ab"),
        ("|ab\n|cd\nef", "dd", "|ef"),
        ("|a\nb\n|c\nd", "dd", "|b\n|d"),
        ("ab\n|cd\n|ef", "Vd", "|ab"),
        ("ab\n|cd\n|ef\ngh", "ccX<esc>", "ab\n|X\ngh"),
    ]);
}

#[test]
fn more_word_and_line_cases() {
    // Checked against Vim 9.1 with `-u NONE`.
    table(&[
        ("a\n|\n  b", "w", "a\n\n  |b"),
        ("|   \nx", "^", "  | \nx"),
        ("|a.b", "e", "a|.b"),
        ("|a.(b", "e", "a.|(b"),
        ("a|.(b c", "e", "a.|(b c"),
        ("|a.b", "de", "|b"),
        ("ab|c", "2d0", "|c"),
        ("ab|cdef", "d20l", "a|b"),
    ]);
}

#[test]
fn undoing_o_and_capital_o_puts_the_cursor_back_where_it_was() {
    // Checked against Vim 9.1 with `-u NONE`. Undoing `A` goes to the
    // line's last char, where the change began.
    table(&[
        ("|abc\nd", "oX<esc>u", "|abc\nd"),
        ("ab|c\nd", "OX<esc>u", "ab|c\nd"),
        ("|abc", "AX<esc>u", "ab|c"),
    ]);
}

#[test]
fn vim_line_on_its_own_and_operators_without_a_vim_motion() {
    use helm_editor::{CommandId, Invocation, vim_commands};
    let mut v = Vim::new("|a\n  b\n  c\nd");
    let registry = CommandRegistry::with_vim_commands();
    let line = Invocation::new(vim_commands::LINE);
    let mut counted = line.clone();
    counted.args.count = Some(3);
    registry.run(&counted, &mut v.editor, &mut v.set).unwrap();
    assert_eq!(v.show(), "a\n  b\n  |c\nd");
    registry.run(&line, &mut v.editor, &mut v.set).unwrap();
    assert_eq!(v.show(), "a\n  b\n  |c\nd");
    for operator in [vim_commands::DELETE, vim_commands::CHANGE] {
        for motion in [None, Some("cursor.left"), Some("vim.select_left")] {
            let mut invocation = Invocation::new(operator);
            invocation.args.motion = motion.map(CommandId::from);
            invocation.args.count = Some(2);
            registry
                .run(&invocation, &mut v.editor, &mut v.set)
                .unwrap();
            assert_eq!(v.show(), "a\n  b\n  |c\nd", "{operator} {motion:?}");
            assert!(!v.editor.in_undo_group(), "{operator} {motion:?}");
        }
    }
}
