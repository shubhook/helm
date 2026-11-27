//! What T-109's Vim tests share: marked text, Vim key notation, and one
//! editor typing through Vim's keymap with a `Session`.
//!
//! Texts are written with markers, as in T-108's tests: `|` is a head and
//! `^` the anchor of the selection whose head is next to it. In normal mode
//! every selection is a cursor, and `|` sits right before the char the
//! block cursor is on. The first selection is the primary.
//!
//! Keys are Vim notation: each char is one key, and `<esc>`, `<cr>`,
//! `<bs>`, `<del>`, `<c-r>` and `<lt>` are Escape, Enter, Backspace,
//! Delete, Ctrl+R and a literal `<`.
//!
//! [`keymap`] and [`registry`] are the only places these tests name the
//! keymap and the registry. Open question 2 of `tasks/109-vim-objects.md`
//! decides whether T-109's bindings join `vim::keymap()` and
//! `with_vim_commands()`, or live in new functions; only these two lines
//! would change.

// Each test file uses a different part of this module.
#![allow(dead_code)]

use helm_editor::{
    CommandError, CommandRegistry, Dispatcher, Editor, Key, KeyEvent, KeyState, Keymap, Keystroke,
    Modifiers, Request, Resolution, Selection, Selections, Session, vim,
};
use helm_text::{Buffer, BufferSet, FileFormat, LineEndings};

/// The keymap T-109's commands are bound in.
pub fn keymap() -> Keymap {
    vim::keymap()
}

/// The registry holding T-109's commands.
pub fn registry() -> CommandRegistry {
    CommandRegistry::with_vim_commands()
}

/// The text with its markers removed, and the selections they mark, in
/// the order they appear.
pub fn parse(marked: &str) -> (String, Vec<Selection>) {
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
pub fn show(text: &str, selections: &Selections) -> String {
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
pub fn events(keys: &str) -> Vec<KeyEvent> {
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
                "lt" => Keystroke::new(Key::Char('<'), Modifiers::NONE),
                other => panic!("unknown key <{other}>"),
            }
        } else {
            Keystroke::new(Key::Char(c), Modifiers::NONE)
        };
        out.push(KeyEvent::new(key));
    }
    out
}

/// A buffer set holding `text`, and an editor on it with `selections`.
pub fn editor_on(text: &str, selections: Vec<Selection>) -> (BufferSet, Editor) {
    let mut set = BufferSet::new();
    let id = set.add(Buffer::from(text));
    let format = FileFormat {
        bom: false,
        line_endings: LineEndings::count(text),
    };
    let mut editor = Editor::new(id, format);
    editor.set_selections(Selections::new(selections, 0).expect("non-empty"));
    (set, editor)
}

/// One editor typing through Vim's keymap, with a session and every
/// request its commands made.
pub struct Vim {
    pub dispatcher: Dispatcher,
    pub state: KeyState,
    pub set: BufferSet,
    pub editor: Editor,
    pub session: Session,
    pub requests: Vec<Request>,
}

impl Vim {
    pub fn new(marked: &str) -> Self {
        let (text, selections) = parse(marked);
        let (set, editor) = editor_on(&text, selections);
        let dispatcher = Dispatcher::new(registry(), keymap());
        let state = dispatcher.key_state();
        Self {
            dispatcher,
            state,
            set,
            editor,
            session: Session::new(),
            requests: Vec::new(),
        }
    }

    /// Types `keys`, panicking if a command errors.
    pub fn keys(&mut self, keys: &str) -> Vec<Resolution> {
        events(keys)
            .iter()
            .map(|event| self.press(event).expect("the command runs"))
            .collect()
    }

    /// Types `keys`, stopping at the first command that errors.
    pub fn try_keys(&mut self, keys: &str) -> Result<Vec<Resolution>, CommandError> {
        events(keys).iter().map(|event| self.press(event)).collect()
    }

    /// Dispatches one event, keeping the requests.
    pub fn press(&mut self, event: &KeyEvent) -> Result<Resolution, CommandError> {
        let (resolution, requests) = self.dispatcher.dispatch_in(
            &mut self.state,
            event,
            &[],
            &mut self.editor,
            &mut self.set,
            &mut self.session,
        )?;
        self.requests.extend(requests);
        Ok(resolution)
    }

    pub fn text(&self) -> String {
        self.set
            .get(self.editor.buffer())
            .expect("in the set")
            .to_string()
    }

    pub fn show(&self) -> String {
        show(&self.text(), self.editor.selections())
    }

    pub fn mode(&self) -> &str {
        self.state.mode().as_str()
    }

    /// Register `name`'s pieces and whether it is linewise, or `None` if
    /// it is empty.
    pub fn reg(&self, name: char) -> Option<(Vec<String>, bool)> {
        self.session
            .register(name)
            .map(|r| (r.pieces.clone(), r.linewise))
    }

    /// Register `name` as one charwise or linewise text, for registers
    /// written from one cursor.
    pub fn reg1(&self, name: char) -> Option<(String, bool)> {
        self.reg(name).map(|(pieces, linewise)| {
            assert_eq!(pieces.len(), 1, "register {name:?} has {pieces:?}");
            (pieces[0].clone(), linewise)
        })
    }
}

/// `marked` after typing `keys`, with markers.
pub fn after(marked: &str, keys: &str) -> String {
    let mut v = Vim::new(marked);
    v.keys(keys);
    v.show()
}

/// Checks each row: start, keys, expected. Each row ends in normal mode.
pub fn table(rows: &[(&str, &str, &str)]) {
    for &(start, keys, expected) in rows {
        let mut v = Vim::new(start);
        v.keys(keys);
        assert_eq!(v.show(), expected, "{start:?} then {keys:?}");
        assert_eq!(v.mode(), vim::NORMAL, "{start:?} then {keys:?}");
    }
}

/// Checks each row: start, keys, expected, then register `name` holding
/// `text`, linewise or not, written from one cursor.
pub fn reg_table(rows: &[(&str, &str, &str, char, &str, bool)]) {
    for &(start, keys, expected, name, text, linewise) in rows {
        let mut v = Vim::new(start);
        v.keys(keys);
        let at = format!("{start:?} then {keys:?}");
        assert_eq!(v.show(), expected, "{at}");
        assert_eq!(v.mode(), vim::NORMAL, "{at}");
        assert_eq!(
            v.reg1(name),
            Some((text.to_owned(), linewise)),
            "{at}, register {name:?}"
        );
    }
}
