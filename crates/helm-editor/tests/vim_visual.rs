//! T-108 acceptance tests: Vim's visual and visual-line modes, typed key
//! by key through `vim::keymap()` into an `Editor` (ED-5).
//!
//! Markers and key notation are as in `vim_normal.rs`. A visual selection
//! is written as the bytes it holds: `^` where the selection's anchor is and
//! `|` where its head is, so `a^bc|d` selects `bc` from `b` to `c`, with
//! Vim's cursor on `c`. In visual-line mode the selection is the same
//! characters, and the line commands act on every line it touches.

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
#[ignore = "T-108: not implemented yet"]
fn v_selects_the_char_under_each_cursor() {
    table(&[
        ("a|bc", "v", "a^b|c"),
        ("|abc", "v", "^a|bc"),
        ("ab|c", "v", "ab^c|"),
        ("a\n|\nb", "v", "a\n^\n|b"),
        ("a|\r\nb", "v", "^a|\r\nb"),
        ("a\r\n|\r\nb", "v", "a\r\n^\r\n|b"),
        ("|😀x", "v", "^😀|x"),
        ("a\n|", "v", "a\n|"),
    ]);
    let mut v = Vim::new("a|bc");
    v.keys("v");
    assert_eq!(v.mode(), vim::VISUAL);
    let mut v = Vim::new("a|bc");
    v.keys("V");
    assert_eq!(v.mode(), vim::VISUAL_LINE);
    assert_eq!(v.show(), "a^b|c");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn motions_move_the_visual_cursor_and_keep_the_anchor() {
    table(&[
        ("a|bcd", "vl", "a^bc|d"),
        ("ab|cd", "vh", "a|bc^d"),
        ("a|bcd", "vlh", "a^b|cd"),
        ("a|bcd", "vlhh", "|ab^cd"),
        ("|ab cd", "vw", "^ab c|d"),
        ("|ab cd", "ve", "^ab| cd"),
        ("|ab cd", "v$", "^ab cd|"),
        ("ab |cd", "v0", "|ab c^d"),
        ("a|bc\ndef", "vj", "a^bc\nde|f"),
        ("abc\nd|ef", "vk", "a|bc\nde^f"),
        ("a\n|b\nc", "vG", "a\n^b\nc|"),
        ("a\n|b\nc", "vgg", "|a\nb^\nc"),
        ("|abcdef", "v3l", "^abcd|ef"),
        ("ab|cd\nx\nefgh", "vjj", "ab^cd\nx\nefg|h"),
    ]);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn o_swaps_the_ends() {
    table(&[
        ("a|bcd", "vllo", "a|bcd^"),
        ("a|bcd", "vlloh", "|abcd^"),
        ("a|bcd", "vlloo", "a^bcd|"),
        ("a|b", "vo", "a^b|"),
    ]);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn escape_and_v_leave_visual_mode_on_the_visual_cursor() {
    table(&[
        ("a|bcd", "vl<esc>", "ab|cd"),
        ("a|bcd", "vlv", "ab|cd"),
        ("ab|cd", "vh<esc>", "a|bcd"),
        ("a|bcd", "Vl<esc>", "ab|cd"),
        ("a|bcd", "VlV", "ab|cd"),
        ("a|b\nc", "vj<esc>", "ab\n|c"),
    ]);
    let mut v = Vim::new("a|bc");
    v.keys("vl<esc>");
    assert_eq!(v.mode(), vim::NORMAL);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn d_and_x_delete_the_selection() {
    table(&[
        ("a|bcd", "vld", "a|d"),
        ("a|bcd", "vlx", "a|d"),
        ("ab|cd", "vhd", "a|d"),
        ("a|bc", "v$d", "|a"),
        ("a|b\ncd", "vjd", "|a"),
        ("a|b\ncde", "vjd", "a|e"),
        ("a\n|\nb", "vd", "a\n|b"),
        ("|abc |def", "vld", "|c |f"),
    ]);
    let mut v = Vim::new("a|bcd");
    v.keys("vld");
    assert_eq!(v.mode(), vim::NORMAL);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn c_and_s_change_the_selection() {
    table(&[
        ("a|bcd", "vlcX<esc>", "a|Xd"),
        ("a|bcd", "vlsX<esc>", "a|Xd"),
        ("a|b\ncde", "vjcX<esc>", "a|Xe"),
    ]);
    let mut v = Vim::new("a|bcd");
    v.keys("vlc");
    assert_eq!(v.mode(), vim::INSERT);
    assert_eq!(v.show(), "a|d");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn visual_line_deletes_and_changes_whole_lines() {
    table(&[
        ("a\n|bc\nd", "Vd", "a\n|d"),
        ("a\n|bc\nd", "Vx", "a\n|d"),
        ("|a\nb\nc", "Vjd", "|c"),
        ("a\nb\n|c", "Vkd", "|a"),
        ("a\n|b", "Vd", "|a"),
        ("|a", "Vd", "|"),
        ("a\n  |b\n  c", "Vd", "a\n  |c"),
        ("a\n|bc\nd", "VcX<esc>", "a\n|X\nd"),
        ("|a\nb\nc", "VjsX<esc>", "|X\nc"),
        ("a\r\n|b\r\nc", "Vd", "a\r\n|c"),
        ("a|b\ncd\nef", "Vjod", "|ef"),
    ]);
    let mut v = Vim::new("a\n|bc\nd");
    v.keys("Vd");
    assert_eq!(v.mode(), vim::NORMAL);
    let mut v = Vim::new("a\n|bc\nd");
    v.keys("Vc");
    assert_eq!(v.mode(), vim::INSERT);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn v_and_capital_v_switch_between_the_visual_modes() {
    table(&[
        ("|ab\ncd", "vjVd", "|"),
        ("|ab\ncd", "Vlvd", "|\ncd"),
        ("a|bc\nd", "vlVd", "|d"),
    ]);
    let mut v = Vim::new("a|bc");
    v.keys("vV");
    assert_eq!(v.mode(), vim::VISUAL_LINE);
    v.keys("v");
    assert_eq!(v.mode(), vim::VISUAL);
    assert_eq!(v.show(), "a^b|c");
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn a_visual_edit_undoes_to_a_cursor_where_it_began() {
    table(&[
        ("a|bcd", "vldu", "a|bcd"),
        ("ab|cd", "vhdu", "a|bcd"),
        ("a|bcd", "vlcXY<esc>u", "a|bcd"),
        ("a\n|b\nc", "Vjdu", "a\n|b\nc"),
        ("a\n|b\nc", "VcX<esc>u", "a\n|b\nc"),
    ]);
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn counts_work_in_visual_modes() {
    table(&[
        ("|abcdef", "v2ld", "|def"),
        ("|a\nb\nc\nd", "V2jd", "|d"),
        ("|a b c d", "v2wd", "| d"),
    ]);
    let mut v = Vim::new("|abc");
    v.keys("v2");
    assert_eq!(v.state.count(), Some(2));
}

#[test]
#[ignore = "T-108: not implemented yet"]
fn unbound_keys_do_nothing_in_visual_modes() {
    for keys in ["vq", "Vq", "vz", "Vi"] {
        let mut v = Vim::new("a|bc");
        let before = {
            let mut w = Vim::new("a|bc");
            w.keys(&keys[..1]);
            w.show()
        };
        v.keys(keys);
        assert_eq!(v.show(), before, "{keys}");
        assert_eq!(v.text(), "abc");
    }
}
