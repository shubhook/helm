//! T-106 acceptance tests: property tests of chord resolution, precedence,
//! switching keymaps, and typing through the dispatcher.

use helm_editor::{
    Action, Binding, CommandId, CommandRegistry, Dispatcher, Editor, Invocation, Key, KeyEvent,
    KeySequence, KeyState, Keymap, Keystroke, Mode, ModeId, Modifiers, Resolution, Selection,
    Selections, Typing,
};
use helm_text::{Buffer, BufferSet, FileFormat};
use proptest::prelude::*;
use proptest::sample::Index;

/// Keystrokes the generated keymaps and key presses draw from: some that
/// type text and some that don't.
const ALPHABET: [&str; 10] = [
    "a",
    "b",
    "escape",
    "enter",
    "ctrl-a",
    "ctrl-b",
    "cmd-1",
    "cmd-2",
    "alt-cmd-x",
    "shift-tab",
];

/// The keystroke at `i` in `ALPHABET`.
fn key(i: usize) -> Keystroke {
    Keystroke::parse(ALPHABET[i]).expect("valid keystroke")
}

/// A sequence of one to three keystrokes from `ALPHABET`.
fn sequence() -> impl Strategy<Value = Vec<usize>> {
    prop::collection::vec(0..ALPHABET.len(), 1..=3)
}

/// The `KeySequence` of `indexes` into `ALPHABET`.
fn keys(indexes: &[usize]) -> KeySequence {
    KeySequence::new(indexes.iter().map(|&i| key(i)).collect()).expect("non-empty")
}

/// Resolves every keystroke of `indexes` with no contexts.
fn press(dispatcher: &Dispatcher, state: &mut KeyState, indexes: &[usize]) -> Vec<Resolution> {
    indexes
        .iter()
        .map(|&i| dispatcher.resolve(state, &KeyEvent::new(key(i)), &[]))
        .collect()
}

/// The sequences in `sequences` with duplicates removed and with every
/// sequence that is a prefix of another dropped, so none starts another.
fn prefix_free(mut sequences: Vec<Vec<usize>>) -> Vec<Vec<usize>> {
    sequences.sort();
    sequences.dedup();
    let all = sequences.clone();
    sequences.retain(|s| {
        !all.iter()
            .any(|other| other.len() > s.len() && other.starts_with(s))
    });
    sequences
}

/// One generated binding: its keys, mode scope, action kind, command
/// number and mode to switch to. Modes are indexes into the keymap's modes.
#[derive(Clone, Debug)]
struct Spec {
    keys: Vec<usize>,
    mode: Option<usize>,
    /// 0 runs, 1 is an operator, 2 has no action.
    action: u8,
    command: usize,
    then_mode: Option<usize>,
}

fn spec(modes: usize) -> impl Strategy<Value = Spec> {
    (
        sequence(),
        prop::option::of(0..modes),
        prop_oneof![6 => Just(0u8), 2 => Just(1u8), 1 => Just(2u8)],
        0..1000usize,
        prop::option::weighted(0.3, 0..modes),
    )
        .prop_map(|(keys, mode, action, command, then_mode)| Spec {
            keys,
            mode,
            action,
            command,
            then_mode,
        })
}

/// A keymap named `name` whose modes are `{prefix}0`, `{prefix}1` and
/// `{prefix}2`, typing in the first two and not the last, with a binding
/// for each spec.
fn build(name: &str, prefix: &str, specs: &[Spec]) -> Keymap {
    let mode = |i: usize| ModeId::from(format!("{prefix}{i}"));
    let modes = (0..3)
        .map(|i| {
            let typing = if i < 2 {
                Typing::Insert
            } else {
                Typing::Ignore
            };
            Mode::new(mode(i), typing)
        })
        .collect();
    let mut keymap = Keymap::new(name, modes).expect("distinct modes");
    for spec in specs {
        let command = CommandId::from(format!("{prefix}.c{}", spec.command));
        keymap
            .bind(Binding {
                keys: keys(&spec.keys),
                mode: spec.mode.map(mode),
                context: None,
                action: match spec.action {
                    0 => Some(Action::Run(command)),
                    1 => Some(Action::Operator(command)),
                    _ => None,
                },
                then_mode: spec.then_mode.map(mode),
            })
            .expect("modes exist");
    }
    keymap
}

/// A dispatcher with an empty registry and `keymap`.
fn dispatcher(keymap: Keymap) -> Dispatcher {
    Dispatcher::new(CommandRegistry::new(), keymap)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Every binding of a prefix-free keymap, typed in any order, waits on
    /// each key but its last and then runs its command, leaving nothing
    /// pending.
    #[test]
    fn chords_resolve_to_their_command(
        sequences in prop::collection::vec(sequence(), 1..40),
        order in prop::collection::vec(any::<Index>(), 1..60),
    ) {
        let sequences = prefix_free(sequences);
        let mut keymap = Keymap::new("k", vec![Mode::new("m", Typing::Insert)]).unwrap();
        for (i, sequence) in sequences.iter().enumerate() {
            keymap.bind(Binding::run(&keys(sequence).to_string(), format!("k.c{i}")).unwrap()).unwrap();
        }
        let dispatcher = dispatcher(keymap);
        let mut state = dispatcher.key_state();
        for index in order {
            let i = index.index(sequences.len());
            let resolutions = press(&dispatcher, &mut state, &sequences[i]);
            let (last, waiting) = resolutions.split_last().unwrap();
            prop_assert!(waiting.iter().all(|r| *r == Resolution::Pending));
            prop_assert_eq!(last, &Resolution::Run(Invocation::new(format!("k.c{i}"))));
            prop_assert!(state.pending().is_empty());
        }
    }

    /// Of several bindings on one key, the one that wins is the applying
    /// binding with a mode over one without, then a context over none,
    /// then the later one.
    #[test]
    fn precedence_matches_the_documented_order(
        bindings in prop::collection::vec(
            (prop::option::of(0..2usize), prop::option::of(0..3usize)),
            1..10,
        ),
        in_second_mode in any::<bool>(),
        holding in prop::collection::vec(any::<bool>(), 3),
    ) {
        let modes = ["one", "two"];
        let contexts = ["x", "y", "z"];
        let mut keymap = Keymap::new(
            "k",
            vec![Mode::new("one", Typing::Insert), Mode::new("two", Typing::Insert)],
        )
        .unwrap();
        for (i, &(mode, context)) in bindings.iter().enumerate() {
            let mut binding = Binding::run("cmd-1", format!("k.c{i}")).unwrap();
            binding.mode = mode.map(|m| ModeId::from(modes[m]));
            binding.context = context.map(|c| contexts[c].to_owned());
            keymap.bind(binding).unwrap();
        }
        keymap.bind(Binding::switch_mode("cmd-2", "two").unwrap().in_mode("one")).unwrap();
        let dispatcher = dispatcher(keymap);
        let mut state = dispatcher.key_state();
        if in_second_mode {
            let switch = KeyEvent::parse("cmd-2").unwrap();
            prop_assert_eq!(dispatcher.resolve(&mut state, &switch, &[]), Resolution::Done);
        }
        let mode = usize::from(in_second_mode);
        let held: Vec<&str> = contexts
            .iter()
            .zip(&holding)
            .filter(|&(_, &held)| held)
            .map(|(&c, _)| c)
            .collect();

        // The oracle.
        let winner = bindings
            .iter()
            .enumerate()
            .filter(|&(_, &(m, c))| {
                m.is_none_or(|m| m == mode) && c.is_none_or(|c| holding[c])
            })
            .max_by_key(|&(i, &(m, c))| (m.is_some(), c.is_some(), i))
            .map(|(i, _)| i);
        let expected = match winner {
            Some(i) => Resolution::Run(Invocation::new(format!("k.c{i}"))),
            None => Resolution::Unbound,
        };
        let event = KeyEvent::parse("cmd-1").unwrap();
        prop_assert_eq!(dispatcher.resolve(&mut state, &event, &held), expected);
    }

    /// After a switch to a keymap that shares no mode with the old one, a
    /// state that had any chord, operator or mode resolves keys exactly as
    /// a new state does.
    #[test]
    fn after_a_switch_a_state_acts_like_a_new_one(
        old in prop::collection::vec(spec(3), 0..30),
        new in prop::collection::vec(spec(3), 0..30),
        before in prop::collection::vec(0..ALPHABET.len(), 0..20),
        after in prop::collection::vec(0..ALPHABET.len(), 1..20),
        sync_first in any::<bool>(),
    ) {
        let mut switched = dispatcher(build("old", "o", &old));
        let mut state = switched.key_state();
        press(&switched, &mut state, &before);
        let previous = switched.set_keymap(build("new", "n", &new));
        prop_assert_eq!(previous.name(), "old");
        if sync_first {
            switched.sync(&mut state);
            prop_assert_eq!(state.mode().as_str(), "n0");
            prop_assert!(state.pending().is_empty());
            prop_assert_eq!(state.operator(), None);
        }

        let fresh = dispatcher(build("new", "n", &new));
        let mut fresh_state = fresh.key_state();
        for &i in &after {
            let event = KeyEvent::new(key(i));
            prop_assert_eq!(
                switched.resolve(&mut state, &event, &[]),
                fresh.resolve(&mut fresh_state, &event, &[])
            );
            prop_assert_eq!(&state.mode(), &fresh_state.mode());
            prop_assert_eq!(state.pending(), fresh_state.pending());
            prop_assert_eq!(state.operator(), fresh_state.operator());
        }
    }

    /// With no bindings on plain chars, typing any text one key per char
    /// through the dispatcher puts exactly that text at the cursor.
    #[test]
    fn typed_keys_insert_their_text(
        initial in "[a-c\n]{0,10}",
        at in any::<Index>(),
        typed in prop::collection::vec(any::<char>().prop_filter("not a control char", |c| !c.is_control()), 0..40),
    ) {
        let mut keymap = Keymap::new("k", vec![Mode::new("m", Typing::Insert)]).unwrap();
        keymap.bind(Binding::run("cmd-z", "editor.undo").unwrap()).unwrap();
        let dispatcher = Dispatcher::new(CommandRegistry::with_editor_commands(), keymap);
        let mut state = dispatcher.key_state();
        let mut set = BufferSet::new();
        let id = set.add(Buffer::from(initial.as_str()));
        let mut editor = Editor::new(id, FileFormat::default());
        let at = at.index(initial.len() + 1);
        editor.set_selections(Selections::single(Selection::cursor(at)));

        for &c in &typed {
            let event = KeyEvent::new(Keystroke::new(Key::Char(c), Modifiers::NONE));
            let resolution = dispatcher
                .dispatch(&mut state, &event, &[], &mut editor, &mut set)
                .unwrap();
            prop_assert_eq!(resolution, Resolution::Run(Invocation::insert(c.to_string())));
        }
        let typed: String = typed.into_iter().collect();
        let expected = format!("{}{}{}", &initial[..at], typed, &initial[at..]);
        prop_assert_eq!(set.get(id).unwrap().to_string(), expected);
        prop_assert_eq!(editor.selections().as_slice(), &[Selection::cursor(at + typed.len())]);
    }
}
