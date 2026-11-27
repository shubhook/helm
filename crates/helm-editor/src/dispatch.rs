//! Turning key events into commands: the active keymap, each editor's key
//! state, and the rule that picks a binding.

use helm_text::BufferSet;

use crate::{
    Action, Binding, CommandArgs, CommandError, CommandId, CommandRegistry, Editor, Invocation,
    Key, KeyEvent, Keymap, Keystroke, ModeId, Modifiers, Request, Session, Typing,
};

/// One editor's input state: its mode, the keys of a chord typed so far,
/// a pending operator, a pending count (T-108), a pending register and the
/// command line (T-109).
///
/// Each editor has its own, so a chord half typed in one pane doesn't carry
/// over to another. A state belongs to the [`Dispatcher`] that made it with
/// [`Dispatcher::key_state`]. Handing it to another dispatcher is a logic
/// error: keys may resolve in the wrong mode, but nothing panics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyState {
    mode: ModeId,
    /// The keys of a chord typed so far. Empty when no chord is pending.
    pending: Vec<Keystroke>,
    /// The pending operator, the mode it was set in, and the count typed
    /// before it.
    operator: Option<(CommandId, ModeId, Option<usize>)>,
    /// The count typed so far, in a counting mode.
    count: Option<usize>,
    /// The register name typed so far (T-109). It stays here while an
    /// operator is pending, until the operator runs.
    register: Option<char>,
    /// The command line typed so far in a [`Typing::Line`] mode, and the
    /// mode it was entered from (T-109).
    line: Option<(String, ModeId)>,
    /// The dispatcher's keymap generation this state last synced to.
    generation: u64,
}

impl KeyState {
    /// The current mode.
    pub fn mode(&self) -> &ModeId {
        &self.mode
    }

    /// The keys of the chord typed so far, or an empty slice if no chord is
    /// pending. `helm-ui` shows these while it waits for the next key.
    pub fn pending(&self) -> &[Keystroke] {
        &self.pending
    }

    /// The pending operator, if one is held.
    pub fn operator(&self) -> Option<&CommandId> {
        self.operator.as_ref().map(|(command, _, _)| command)
    }

    /// The count typed so far in a [`Typing::Count`] mode (T-108), or
    /// `None` if no digit is pending. The count typed before an operator
    /// moves onto the operator when it is set, so after `2 d` this is
    /// `None` until another digit comes. `helm-ui` shows it in the status
    /// bar while it waits for the next key.
    pub fn count(&self) -> Option<usize> {
        self.count
    }

    /// The register name held for the next command (T-109), or `None` if
    /// none is pending. A name typed before an operator stays here until
    /// the operator runs. `helm-ui` shows it in the status bar while it
    /// waits for the next key.
    pub fn register(&self) -> Option<char> {
        self.register
    }

    /// The command line typed so far (T-109), or `None` when the mode's
    /// typing isn't [`Typing::Line`]. `helm-ui` draws it after a `:`.
    /// [`cancel`](Self::cancel) and a keymap switch drop it and go back to
    /// the mode it was entered from, as for a pending operator.
    pub fn line(&self) -> Option<&str> {
        self.line.as_ref().map(|(line, _)| line.as_str())
    }

    /// Drops the pending chord, the pending operator, the pending count
    /// (T-108) and the pending register (T-109), including a count or a
    /// register held with the operator, and returns to
    /// the mode the operator was set in, if one was held. Returns whether
    /// anything was pending. `helm-ui` calls this when the editor loses
    /// focus or the user clicks in it.
    pub fn cancel(&mut self) -> bool {
        let chord = !self.pending.is_empty();
        let count = self.count.take().is_some();
        let register = self.register.take().is_some();
        self.pending.clear();
        let line = match self.line.take() {
            Some((_, mode)) => {
                self.mode = mode;
                true
            }
            None => false,
        };
        match self.operator.take() {
            Some((_, mode, _)) => {
                self.mode = mode;
                true
            }
            None => chord || count || register || line,
        }
    }
}

/// What a key event resolved to. The [`KeyState`] has already changed to
/// match when [`Dispatcher::resolve`] returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// The key started or continued a chord, set a pending operator, or
    /// added a digit to a count. Nothing runs yet.
    Pending,
    /// Run this command. Typed text is an invocation of
    /// [`editor_commands::INSERT`](crate::editor_commands::INSERT).
    Run(Invocation),
    /// A binding matched that runs nothing: it only switched mode, or it
    /// has no action.
    Done,
    /// No binding matched, and the key types nothing in this mode.
    Unbound,
    /// The key broke a pending chord or cancelled a pending operator.
    /// Nothing runs, and the key itself is dropped.
    Cancelled,
}

/// The active keymap and the command registry: what every editor's keys
/// go through.
///
/// # Switching keymaps (ED-6)
///
/// The app holds one dispatcher, and every editor resolves its keys through
/// it with its own [`KeyState`]. [`set_keymap`](Self::set_keymap) replaces
/// the keymap, so every open editor uses the new one from its next key, with
/// no restart and no list of editors to update. Each state notices the
/// switch the next time it is passed to [`resolve`](Self::resolve) or
/// [`sync`](Self::sync), and then:
///
/// - it cancels as [`KeyState::cancel`] does: it drops its pending chord,
///   pending operator, pending count, pending register and command line
///   (T-109), running nothing, and goes back to the mode the operator was
///   set in or the line was entered from, if either was held;
/// - then it keeps its mode if the new keymap has a mode with the same id,
///   and otherwise goes to the new keymap's initial mode.
///
/// The key that arrives after a switch is resolved in the new keymap as if
/// nothing had been pending. Reloading the same keymap after an edit to the
/// keybindings file (ST-1) is a switch too, so a Vim user in insert mode
/// stays in insert mode.
#[derive(Clone, Debug)]
pub struct Dispatcher {
    registry: CommandRegistry,
    keymap: Keymap,
    /// Goes up by one on every `set_keymap`.
    generation: u64,
}

impl Dispatcher {
    /// A dispatcher running commands from `registry`, with `keymap` active.
    pub fn new(registry: CommandRegistry, keymap: Keymap) -> Self {
        Self {
            registry,
            keymap,
            generation: 0,
        }
    }

    /// The command registry.
    pub fn registry(&self) -> &CommandRegistry {
        &self.registry
    }

    /// The active keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// Makes `keymap` the active keymap and returns the old one. See the
    /// type docs for what happens to each editor's state.
    pub fn set_keymap(&mut self, keymap: Keymap) -> Keymap {
        self.generation += 1;
        std::mem::replace(&mut self.keymap, keymap)
    }

    /// A new state for one editor: in the active keymap's initial mode,
    /// with nothing pending.
    pub fn key_state(&self) -> KeyState {
        KeyState {
            mode: self.keymap.initial_mode().clone(),
            pending: Vec::new(),
            operator: None,
            count: None,
            register: None,
            line: None,
            generation: self.generation,
        }
    }

    /// Brings `state` up to date with the active keymap, as the type docs
    /// describe, if the keymap changed since `state` last saw it. `helm-ui`
    /// calls this on every open editor after a switch, so a status bar
    /// shows the right mode before the next key.
    pub fn sync(&self, state: &mut KeyState) {
        if state.generation == self.generation {
            return;
        }
        state.cancel();
        if self.keymap.mode(state.mode.as_str()).is_none() {
            state.mode = self.keymap.initial_mode().clone();
        }
        state.generation = self.generation;
    }

    /// Resolves one key event against the active keymap, updates `state`,
    /// and says what to run. `contexts` are the context names that hold
    /// right now.
    ///
    /// First `state` syncs, as [`sync`](Self::sync) does. Then, with `M` the
    /// state's mode and `S` the pending keys followed by the event's
    /// keystroke, a binding *applies* when its mode is `None` or `M` and
    /// its context is `None` or one of `contexts`. The first rule that fits
    /// decides:
    ///
    /// 0. **Count (T-108).** If `M`'s [`Typing`](crate::Typing) is
    ///    `Count`, no chord is pending, and the keystroke is a
    ///    [`Key::Char`](crate::Key::Char) digit with no modifiers that is
    ///    `1` to `9`, or `0` with a count already pending, the digit is
    ///    added to the state's count, saturating at `usize::MAX`:
    ///    `Pending`. No binding is looked at.
    /// 1. **Chord prefix.** If `S` is a strict prefix of an applying
    ///    binding's keys, `S` becomes the pending chord: `Pending`. This
    ///    holds even when another applying binding's keys equal `S`. A
    ///    chord waits for its next key with no timeout.
    /// 2. **Match.** If applying bindings' keys equal `S`, one wins: the
    ///    one with a mode over one without, then the one with a context
    ///    over one without, then the one added later. The pending chord
    ///    clears, and the winner acts:
    ///    - [`Action::Run`](crate::Action::Run) with no operator pending:
    ///      `Run` of the command. The pending count goes with it as its
    ///      [`count`](crate::CommandArgs::count), and clears.
    ///    - `Action::Run` with an operator pending: `Run` of the operator,
    ///      with the binding's command as its
    ///      [`motion`](crate::CommandArgs::motion) and the counts as its
    ///      `count` (see "Counts" below). The operator and the counts clear
    ///      and the mode goes back to the one the operator was set in.
    ///    - [`Action::Operator`](crate::Action::Operator) with no operator
    ///      pending: the command becomes the pending operator, set in mode
    ///      `M`, and the pending count moves onto it: `Pending`.
    ///    - `Action::Operator` with an operator pending: the pending one is
    ///      cancelled as in rule 3: `Cancelled`. The binding's `then_mode`
    ///      doesn't apply.
    ///    - [`Action::Register`](crate::Action::Register) with no operator
    ///      pending (T-109): the name becomes the pending register,
    ///      replacing any pending one, and a pending count stays: `Pending`.
    ///      With an operator pending, the operator is cancelled as in rule
    ///      3: `Cancelled`.
    ///    - No action: a pending operator, a pending count and a pending
    ///      register are cancelled, without returning `Cancelled`: `Done`.
    ///
    ///    Then, except where noted, the binding's `then_mode`, if any,
    ///    becomes the mode, after any return to the operator's mode.
    /// 3. **Broken chord, operator, count or register.** If a chord, an
    ///    operator, a count or a register is pending, all of them clear, the mode goes back to the
    ///    one the operator was set in, if one was held, and the key is
    ///    dropped: `Cancelled`. So Escape cancels anything pending unless a
    ///    binding says otherwise.
    /// 4. **Typing.** If mode `M`'s [`Typing`](crate::Typing) is `Insert`,
    ///    the event has non-empty [`text`](KeyEvent::text), and neither
    ///    Ctrl nor Cmd is held: `Run` of [`Invocation::insert`] with the
    ///    text. Typing takes the same path as a bound command.
    /// 5. **The command line (T-109).** If `M`'s typing is
    ///    [`Line`](crate::Typing::Line): an event with non-empty text and
    ///    neither Ctrl nor Cmd held adds the text to the end of the line:
    ///    `Pending`. Backspace with no modifiers removes the line's last
    ///    char: `Pending`. On an empty line it clears the line and goes back
    ///    to the mode the line was entered from: `Done`.
    /// 6. Otherwise `Unbound`.
    ///
    /// Mode changes happen here, before the caller runs the returned
    /// command, and stand even if the command then fails.
    ///
    /// # Counts (T-108)
    ///
    /// Only a `Run` carries a count, in its invocation's
    /// [`CommandArgs::count`](crate::CommandArgs::count). It is `None` when
    /// no digits were typed before it. For a `Run` of a pending operator it
    /// is the count typed before the operator times the count typed after
    /// it, or whichever one was typed, saturating at `usize::MAX`. So in
    /// Vim's keymap `2 d 3 w` gives 6, `d 3 w` gives 3 and `d w` gives
    /// `None`. A count survives a chord: `1 2 g g` gives 12. `helm-ui` reads
    /// it from the invocation for the ids it runs itself.
    ///
    /// # Registers (T-109)
    ///
    /// A `Run` carries the pending register as its
    /// [`CommandArgs::register`](crate::CommandArgs::register) and clears
    /// it. A register typed before an operator moves onto it, so `" a d w`
    /// gives `vim.delete` with register `a`, and a count can come before or
    /// after the register: `2 " a y y` and `" a 2 y y` give the same `Run`.
    ///
    /// # The command line (T-109)
    ///
    /// When a binding's `then_mode` switches from a mode whose typing isn't
    /// [`Line`](crate::Typing::Line) into one whose typing is, the line
    /// starts empty and remembers the mode it came from. A `Run` that
    /// resolves while the mode's typing is `Line` carries the line as its
    /// [`CommandArgs::text`](crate::CommandArgs::text). The line clears
    /// whenever the mode becomes one whose typing isn't `Line`, by a
    /// binding or by rule 5. [`KeyState::cancel`] and a keymap switch drop
    /// it and go back to the mode it was entered from, as they do for a
    /// pending operator, since a key state can't see the keymap's modes.
    pub fn resolve(&self, state: &mut KeyState, event: &KeyEvent, contexts: &[&str]) -> Resolution {
        self.sync(state);
        let keystroke = event.keystroke;

        // Rule 0. A keymap with no counting mode skips it with one check.
        if self.keymap.counts()
            && state.pending.is_empty()
            && let Key::Char(c) = keystroke.key()
            && keystroke.modifiers() == Modifiers::NONE
            && let Some(digit) = c.to_digit(10)
            && (digit != 0 || state.count.is_some())
            && self
                .keymap
                .mode(state.mode.as_str())
                .is_some_and(|mode| mode.typing == Typing::Count)
        {
            let count = state.count.unwrap_or(0).saturating_mul(10);
            state.count = Some(count.saturating_add(digit as usize));
            return Resolution::Pending;
        }

        let depth = state.pending.len();
        let first = state.pending.first().copied().unwrap_or(keystroke);

        // Rules 1 and 2: look for a longer binding, and rank the exact ones.
        let mut prefix = false;
        let mut winner: Option<((bool, bool), &Binding)> = None;
        for binding in self.keymap.bindings_starting_with(first) {
            let keys = binding.keys.as_slice();
            if keys.len() <= depth
                || keys[depth] != keystroke
                || keys[..depth] != state.pending[..]
                || !applies(binding, &state.mode, contexts)
            {
                continue;
            }
            if keys.len() > depth + 1 {
                prefix = true;
                break;
            }
            // Later bindings come later, so `>=` lets one of equal rank win.
            let rank = (binding.mode.is_some(), binding.context.is_some());
            if winner.is_none_or(|(best, _)| rank >= best) {
                winner = Some((rank, binding));
            }
        }

        if prefix {
            state.pending.push(keystroke);
            return Resolution::Pending;
        }
        if let Some((_, binding)) = winner {
            state.pending.clear();
            let resolution = match &binding.action {
                Some(Action::Run(command)) => match state.operator.take() {
                    None => Resolution::Run(Invocation {
                        command: command.clone(),
                        args: CommandArgs {
                            count: state.count.take(),
                            register: state.register.take(),
                            text: state.line.as_ref().map(|(line, _)| line.clone()),
                            ..CommandArgs::default()
                        },
                    }),
                    Some((operator, mode, before)) => {
                        state.mode = mode;
                        let count = match (before, state.count.take()) {
                            (Some(before), Some(after)) => Some(before.saturating_mul(after)),
                            (before, after) => before.or(after),
                        };
                        Resolution::Run(Invocation {
                            command: operator,
                            args: CommandArgs {
                                motion: Some(command.clone()),
                                count,
                                register: state.register.take(),
                                ..CommandArgs::default()
                            },
                        })
                    }
                },
                Some(Action::Operator(_)) if state.operator.is_some() => {
                    state.cancel();
                    return Resolution::Cancelled;
                }
                Some(Action::Register(_)) if state.operator.is_some() => {
                    state.cancel();
                    return Resolution::Cancelled;
                }
                Some(Action::Register(name)) => {
                    state.register = Some(*name);
                    Resolution::Pending
                }
                Some(Action::Operator(command)) => {
                    let count = state.count.take();
                    state.operator = Some((command.clone(), state.mode.clone(), count));
                    Resolution::Pending
                }
                None => {
                    state.cancel();
                    Resolution::Done
                }
            };
            if let Some(mode) = &binding.then_mode {
                // `from` is the mode after any return to the operator's.
                let from = std::mem::replace(&mut state.mode, mode.clone());
                if self.keymap.lines() {
                    self.enter_line(state, from);
                }
            }
            return resolution;
        }

        // Rule 3. Most keys reach here with nothing pending, and then
        // `cancel` would change nothing, so it is skipped.
        if (!state.pending.is_empty()
            || state.operator.is_some()
            || state.count.is_some()
            || state.register.is_some())
            && state.cancel()
        {
            return Resolution::Cancelled;
        }

        // Rules 4 and 5.
        let modifiers = keystroke.modifiers();
        let typing = self
            .keymap
            .mode(state.mode.as_str())
            .map(|mode| mode.typing);
        let text = event
            .text
            .as_deref()
            .filter(|text| !text.is_empty() && !modifiers.ctrl && !modifiers.cmd);
        match (typing, text) {
            (Some(Typing::Insert), Some(text)) => Resolution::Run(Invocation::insert(text)),
            (Some(Typing::Line), _) => line_key(state, keystroke, text),
            _ => Resolution::Unbound,
        }
    }

    /// After a binding's `then_mode` switched `state` out of mode `from`:
    /// starts an empty command line if the new mode's typing is
    /// [`Typing::Line`] and none is open, and drops the line if it isn't.
    fn enter_line(&self, state: &mut KeyState, from: ModeId) {
        let line = self
            .keymap
            .mode(state.mode.as_str())
            .is_some_and(|mode| mode.typing == Typing::Line);
        if !line {
            state.line = None;
        } else if state.line.is_none() {
            state.line = Some((String::new(), from));
        }
    }

    /// [`resolve`](Self::resolve), then, for [`Resolution::Run`], runs the
    /// invocation on `editor` and `set` with [`CommandRegistry::run`], so
    /// the command gets the invocation's count (T-108). Returns the
    /// resolution, or the command's error.
    ///
    /// The command runs with a fresh [`Session`], so registers, `.` and
    /// the requests a command makes don't outlive the call (T-109).
    /// `helm-ui` uses [`dispatch_in`](Self::dispatch_in).
    pub fn dispatch(
        &self,
        state: &mut KeyState,
        event: &KeyEvent,
        contexts: &[&str],
        editor: &mut Editor,
        set: &mut BufferSet,
    ) -> Result<Resolution, CommandError> {
        let resolution = self.resolve(state, event, contexts);
        if let Resolution::Run(invocation) = &resolution {
            self.registry.run(invocation, editor, set)?;
        }
        Ok(resolution)
    }

    /// [`resolve`](Self::resolve), then, for [`Resolution::Run`], runs the
    /// invocation on `editor` and `set` with
    /// [`CommandRegistry::run_in`] and `session` (T-109). Returns the
    /// resolution and the [`Request`]s the command made, in order, for
    /// `helm-ui` to carry out. On the command's error it returns the error
    /// and no request.
    ///
    /// This is the dispatch `helm-ui` uses. [`dispatch`](Self::dispatch) is
    /// this with a fresh [`Session`] that is dropped afterwards, along with
    /// the requests.
    pub fn dispatch_in(
        &self,
        state: &mut KeyState,
        event: &KeyEvent,
        contexts: &[&str],
        editor: &mut Editor,
        set: &mut BufferSet,
        session: &mut Session,
    ) -> Result<(Resolution, Vec<Request>), CommandError> {
        let resolution = self.resolve(state, event, contexts);
        let requests = match &resolution {
            Resolution::Run(invocation) => {
                self.registry.run_in(invocation, editor, set, session)?
            }
            _ => Vec::new(),
        };
        Ok((resolution, requests))
    }
}

/// Rule 5 of [`Dispatcher::resolve`]: an unbound key in a
/// [`Typing::Line`] mode, with `text` the text it types, if any.
fn line_key(state: &mut KeyState, keystroke: Keystroke, text: Option<&str>) -> Resolution {
    let mode = &state.mode;
    let (line, _) = state
        .line
        .get_or_insert_with(|| (String::new(), mode.clone()));
    if let Some(text) = text {
        line.push_str(text);
        return Resolution::Pending;
    }
    if keystroke.key() != Key::Backspace || keystroke.modifiers() != Modifiers::NONE {
        return Resolution::Unbound;
    }
    if line.pop().is_some() {
        return Resolution::Pending;
    }
    if let Some((_, mode)) = state.line.take() {
        state.mode = mode;
    }
    Resolution::Done
}

/// Whether `binding` applies in mode `mode` with `contexts` holding.
fn applies(binding: &Binding, mode: &ModeId, contexts: &[&str]) -> bool {
    binding.mode.as_ref().is_none_or(|m| m == mode)
        && binding
            .context
            .as_deref()
            .is_none_or(|context| contexts.contains(&context))
}
