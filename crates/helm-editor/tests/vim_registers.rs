//! T-109 acceptance tests: Vim's registers, `y`, `p` and `P`, and `d` and
//! `c` writing registers, typed key by key into an `Editor` with a
//! `Session` (ED-5), and the `+` and `*` registers as the clipboard (ED-1).
//!
//! Markers and key notation are as in `vim_harness`.

mod vim_harness;

use helm_editor::{Request, Selection, Selections, Session};
use vim_harness::{Vim, reg_table, table};

#[test]
fn y_yanks_without_changing_the_text() {
    reg_table(&[
        ("|ab cd", "yw", "|ab cd", '"', "ab ", false),
        ("ab |cd", "yb", "|ab cd", '"', "ab ", false),
        ("a|b cd", "y$", "a|b cd", '"', "b cd", false),
        ("ab|c\nd", "yw", "ab|c\nd", '"', "c", false),
        ("a|b\ncd", "yy", "a|b\ncd", '"', "ab\n", true),
        ("a|b\ncd", "Y", "a|b\ncd", '"', "ab\n", true),
        ("a|b\ncd\ne", "2yy", "a|b\ncd\ne", '"', "ab\ncd\n", true),
        ("a\n|b", "yk", "|a\nb", '"', "a\nb\n", true),
        ("|a\nb", "yj", "|a\nb", '0', "a\nb\n", true),
        ("|ab\r\ncd", "yy", "|ab\r\ncd", '"', "ab\r\n", true),
    ]);
    let mut v = Vim::new("|ab cd");
    v.keys("yw");
    assert_eq!(v.editor.undo(&mut v.set), Ok(None));
}

#[test]
fn p_and_p_put_charwise_text() {
    table(&[
        ("|ab", "ylp", "a|ab"),
        ("|ab", "ylP", "|aab"),
        ("|ab", "yl3p", "aaa|ab"),
        ("|ab", "yl3P", "aa|aab"),
        ("|ab", "xp", "b|a"),
        ("foo |bar", "yiwP", "foo ba|rbar"),
        ("|ab\ncd", "ylj$p", "ab\ncd|a"),
        ("|a\nb", "vjyGp", "a\nb|a\nb"),
    ]);
}

#[test]
fn p_and_p_put_linewise_text() {
    table(&[
        ("|a\nb", "yyp", "a\n|a\nb"),
        ("|a\nb", "yyP", "|a\na\nb"),
        ("a\n|b", "yyp", "a\nb\n|b"),
        ("|ab", "yy2p", "ab\n|ab\nab"),
        ("|  ab\ncd", "yyjp", "  ab\ncd\n  |ab"),
        ("|a\nb", "ddp", "b\n|a"),
        ("|a\r\nb", "yyp", "a\r\n|a\r\nb"),
    ]);
}

#[test]
fn deletes_write_the_numbered_and_small_delete_registers() {
    reg_table(&[
        ("|ab cd", "dw", "|cd", '-', "ab ", false),
        ("|ab cd", "dw", "|cd", '"', "ab ", false),
        ("|a\nb", "dd", "|b", '1', "a\n", true),
        ("|a\nb\nc", "dddd", "|c", '2', "a\n", true),
        ("|a\nb\nc", "dddd", "|c", '1', "b\n", true),
        ("a|b\ncd", "d$", "|a\ncd", '-', "b", false),
        ("a|b\ncd", "dj", "|", '1', "ab\ncd\n", true),
        ("|ab", "x", "|b", '-', "a", false),
        ("|ab cd", "cwX<esc>", "|X cd", '-', "ab", false),
        ("|ab cd", "\"adw", "|cd", 'a', "ab ", false),
        ("|ab cd", "\"adw", "|cd", '"', "ab ", false),
    ]);
    // A named register leaves `1` and `-` alone.
    let mut v = Vim::new("|a\nb\nc");
    v.keys("dd\"bdd");
    assert_eq!(v.reg1('1'), Some(("a\n".to_owned(), true)));
    assert_eq!(v.reg1('b'), Some(("b\n".to_owned(), true)));
    // A delete with a line break in it that isn't linewise goes to `1`.
    let mut v = Vim::new("a|b\ncd");
    v.keys("dw");
    assert_eq!(v.reg('1'), None);
    let mut v = Vim::new("a|b\ncd");
    v.keys("vjd");
    assert_eq!(v.reg1('1'), Some(("b\ncd".to_owned(), false)));
}

#[test]
fn named_registers_append_and_the_black_hole() {
    table(&[
        ("|ab cd", "\"ayiww\"ap", "ab ca|bd"),
        ("|ab cd", "\"ayiww\"Ayiw\"ap", "ab cabc|dd"),
        ("|ab cd", "yiww\"_dwP", "aba|b "),
        ("|ab cd", "yiwwdw\"0P", "aba|b "),
        ("|ab cd", "\"ayiwx\"aP", "a|bb cd"),
        ("|a\nb", "\"ayyj\"Ayy\"ap", "a\nb\n|a\nb"),
    ]);
    let mut v = Vim::new("|ab cd");
    v.keys("\"_dw");
    assert_eq!(v.reg('"'), None);
    assert_eq!(v.reg('_'), None);
}

#[test]
fn c_writes_registers_and_puts_after_it() {
    table(&[
        ("|ab cd", "cwX<esc>wp", "X ca|bd"),
        ("|ab\ncd", "ccX<esc>jp", "X\ncd\n|ab"),
    ]);
}

#[test]
fn yank_and_put_in_visual_modes() {
    table(&[
        ("a|bc", "vly$p", "abcb|c"),
        ("a|b\ncd", "Vy$p", "ab\n|ab\ncd"),
        ("|ab cd", "yiwwviwp", "ab a|b"),
        ("|ab cd", "yiwwviwp0P", "c|dab ab"),
        ("|ab cd", "yiwwviwP0P", "a|bab ab"),
        ("|a\nb", "yyjVp", "a\n|a"),
        ("|a\nb", "yyjVP", "a\n|a"),
    ]);
    reg_table(&[("a|bc d", "vly", "a|bc d", '"', "bc", false)]);
    let mut v = Vim::new("a|bc\nd");
    v.keys("Vy");
    assert_eq!(v.reg1('0'), Some(("abc\n".to_owned(), true)));
    assert_eq!(v.mode(), "normal");
}

#[test]
fn several_cursors_put_their_own_pieces() {
    let mut v = Vim::new("|ab\n|cd");
    v.keys("ylp");
    assert_eq!(v.show(), "a|ab\nc|cd");
    assert_eq!(v.reg('"'), Some((vec!["a".into(), "c".into()], false)));

    // Three cursors and two pieces: every cursor puts both, joined by the
    // editor's line break.
    let mut v = Vim::new("|ab\n|cd\nef");
    v.keys("yl");
    let cursors = [0, 3, 6].map(Selection::cursor).to_vec();
    v.editor
        .set_selections(Selections::new(cursors, 0).unwrap());
    assert_eq!(v.text(), "ab\ncd\nef");
    v.keys("P");
    assert_eq!(v.text(), "a\ncab\na\nccd\na\ncef");
    v.keys("u");
    assert_eq!(v.text(), "ab\ncd\nef");

    // Linewise pieces run together.
    let mut v = Vim::new("|a\n|b");
    v.keys("yy");
    assert_eq!(v.reg('"'), Some((vec!["a\n".into(), "b\n".into()], true)));
    v.editor
        .set_selections(Selections::new(vec![Selection::cursor(0)], 0).unwrap());
    v.keys("P");
    assert_eq!(v.text(), "a\nb\na\nb");
}

#[test]
fn every_put_at_every_cursor_is_one_undo_step() {
    let mut v = Vim::new("|a\n|b\n|c");
    v.keys("yl3p");
    assert_eq!(v.text(), "aaaa\nbbbb\ncccc");
    v.keys("u");
    assert_eq!(v.show(), "|a\n|b\n|c");
}

#[test]
fn an_empty_register_puts_nothing() {
    table(&[
        ("a|b", "p", "a|b"),
        ("a|b", "\"qP", "a|b"),
        ("a|b", "\"_p", "a|b"),
    ]);
}

#[test]
fn the_clipboard_registers_push_requests() {
    let mut v = Vim::new("|ab cd");
    v.keys("\"+yiw");
    assert_eq!(
        v.requests,
        vec![Request::SetClipboard {
            text: "ab".to_owned()
        }]
    );
    assert_eq!(v.reg1('+'), Some(("ab".to_owned(), false)));
    assert_eq!(v.reg1('*'), Some(("ab".to_owned(), false)));
    assert_eq!(v.reg1('"'), Some(("ab".to_owned(), false)));
    v.keys("w\"*dd");
    assert_eq!(
        v.requests[1],
        Request::SetClipboard {
            text: "ab cd\n".to_owned()
        }
    );
    // Other registers make no request.
    let mut v = Vim::new("|ab cd");
    v.keys("yiw\"ayiwdwp");
    assert!(v.requests.is_empty());
}

#[test]
fn the_clipboard_text_comes_from_set_clipboard() {
    let mut v = Vim::new("a|b");
    v.session.set_clipboard(Some("XY".to_owned()));
    v.keys("\"+p");
    assert_eq!(v.show(), "abX|Y");
    // Text ending in a line break is linewise.
    v.session.set_clipboard(Some("L\n".to_owned()));
    v.keys("\"*P");
    assert_eq!(v.show(), "|L\nabXY");
    // The unnamed register doesn't follow the clipboard.
    assert_eq!(v.reg('"'), None);
    v.session.set_clipboard(None);
    assert_eq!(v.reg('+'), None);
    v.keys("\"+p");
    assert_eq!(v.text(), "L\nabXY");
}

#[test]
fn text_helm_wrote_keeps_its_pieces_on_the_clipboard() {
    let mut v = Vim::new("|ab\n|cd");
    v.keys("\"+yl");
    assert_eq!(
        v.requests,
        vec![Request::SetClipboard {
            text: "a\nc".to_owned()
        }]
    );
    v.session.set_clipboard(Some("a\nc".to_owned()));
    assert_eq!(v.reg('+'), Some((vec!["a".into(), "c".into()], false)));
    v.session.set_clipboard(Some("other".to_owned()));
    assert_eq!(v.reg('+'), Some((vec!["other".into()], false)));
}

#[test]
fn a_one_megabyte_clipboard_round_trip_keeps_every_byte() {
    // ED-1: CRLF, lone CR, every char width, and a BOM-looking first char.
    let unit = "\u{feff}ab\r\ncd\re中😀é\n";
    let mut text = String::new();
    while text.len() < 1 << 20 {
        text.push_str(unit);
    }
    text.push('z');
    let mut v = Vim::new("|");
    v.session.set_clipboard(Some(text.clone()));
    v.keys("\"+P");
    assert_eq!(v.text(), text);
    v.keys("gg\"+yG");
    let written = v.requests.iter().rev().find_map(|r| match r {
        Request::SetClipboard { text } => Some(text.clone()),
        _ => None,
    });
    assert_eq!(written.map(|t| t.len()), Some(text.len() + 1));
}

#[test]
fn registers_are_shared_by_every_editor_on_a_session() {
    let mut a = Vim::new("|ab cd");
    a.keys("\"ayiw");
    let mut b = Vim::new("x|y");
    b.session = std::mem::take(&mut a.session);
    b.keys("\"ap");
    assert_eq!(b.show(), "xya|b");
    assert!(Session::new().register('a').is_none());
}
