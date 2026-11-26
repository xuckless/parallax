//! Text input between GPUI's `InputHandler` and the ported Flutter `TextInputPlugin` (Java). Both count in
//! UTF-16 code units, so offsets pass through unchanged. Pure: the shell in `host.rs` reads and writes GPUI.
//!
//! The sync protocol (docs/android-text-input.md):
//! - **Java → GPUI, synchronous.** After each IME batch edit Java sends its full editing state
//!   (`updateEditingState`); [`edits`] turns the difference to GPUI's state into `InputHandler` calls.
//! - **GPUI → Java, at the end of the frame.** GPUI's notifications fold into one [`Pending`]; `nativeFrame`
//!   hands it to Java as Flutter's `flutter/textinput` calls ([`Pending::calls`]), run after GPUI's draw.
//! - **Conflict:** while GPUI has a change Java has not seen, Java's edit is stale and dropped; GPUI wins.

use std::ops::Range;

use gpui::{
    Autocapitalize, FocusId, TextInputAction, TextInputConfiguration, TextInputStateChange,
};

/// One side's editing state, as Flutter's `TextEditState`: text, selection and composing (marked) range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditingState {
    pub(crate) text: String,
    /// Flutter's `(selectionBase, selectionExtent)`; the extent comes first when the selection is reversed.
    pub(crate) selection: Option<(usize, usize)>,
    /// Never empty.
    pub(crate) composing: Option<Range<usize>>,
}

impl EditingState {
    /// Java's values, `-1` for none. Out-of-range values (an IME's bug) count as none.
    pub(crate) fn from_java(
        text: String,
        selection: (i32, i32),
        composing: (i32, i32),
    ) -> EditingState {
        let length = text.encode_utf16().count();
        let offset = |value: i32| usize::try_from(value).ok().filter(|&value| value <= length);
        let selection = match (offset(selection.0), offset(selection.1)) {
            (Some(base), Some(extent)) => Some((base, extent)),
            (None, _) | (_, None) => None,
        };
        let composing = match (offset(composing.0), offset(composing.1)) {
            (Some(start), Some(end)) if start < end => Some(start..end),
            (Some(_), Some(_)) | (None, _) | (_, None) => None,
        };
        EditingState {
            text,
            selection,
            composing,
        }
    }

    /// Flutter's `TextEditState` JSON (`TextEditState.fromJson`).
    fn to_json(&self) -> String {
        let (base, extent) = self
            .selection
            .map_or((-1, -1), |(base, extent)| (base as i64, extent as i64));
        let (start, end) = self
            .composing
            .as_ref()
            .map_or((-1, -1), |range| (range.start as i64, range.end as i64));
        format!(
            r#"{{"text":{},"selectionBase":{base},"selectionExtent":{extent},"composingBase":{start},"composingExtent":{end}}}"#,
            json_string(&self.text)
        )
    }

    fn selected_range(&self) -> Option<Range<usize>> {
        self.selection
            .map(|(base, extent)| base.min(extent)..base.max(extent))
    }
}

/// One `InputHandler` call, UTF-16 ranges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Edit {
    /// `replace_text_in_range`: plain text; ends any composition, caret after the text.
    Replace { range: Range<usize>, text: String },
    /// `replace_and_mark_text_in_range`: the text becomes the composing range; `selected` is relative to it.
    Mark {
        range: Range<usize>,
        text: String,
        selected: Option<Range<usize>>,
    },
    /// `unmark_text`.
    Unmark,
    /// `set_selected_text_range`.
    Select(Range<usize>),
}

/// The calls that bring GPUI from `gpui` to Java's `java` state. A text change is one replace (a mark when the
/// new text is the composing range, as `setComposingText`); then the composing range and the selection are fixed
/// up only where the calls before left them different.
pub(crate) fn edits(gpui: &EditingState, java: &EditingState) -> Vec<Edit> {
    let old: Vec<u16> = gpui.text.encode_utf16().collect();
    let new: Vec<u16> = java.text.encode_utf16().collect();
    let mut edits = Vec::new();
    // What GPUI holds after the calls so far.
    let mut marked = gpui.composing.clone();
    let mut caret = gpui.selected_range();
    if old != new {
        let (mut replaced, mut inserted) = changed_span(&old, &new);
        // The composing range covers the change: replace all of it in one mark (setComposingText's shape).
        let covering = java
            .composing
            .clone()
            .filter(|composing| composing.start <= inserted.start && inserted.end <= composing.end);
        if let Some(composing) = &covering {
            replaced = composing.start..replaced.end + (composing.end - inserted.end);
            inserted = composing.clone();
        }
        let text = String::from_utf16_lossy(&new[inserted.clone()]);
        match covering {
            Some(composing) => {
                let selected = relative(java.selected_range(), &composing);
                caret = Some(match &selected {
                    Some(selected) => {
                        composing.start + selected.start..composing.start + selected.end
                    }
                    None => composing.end..composing.end,
                });
                edits.push(Edit::Mark {
                    range: replaced,
                    text,
                    selected,
                });
                marked = Some(composing);
            }
            None => {
                edits.push(Edit::Replace {
                    range: replaced,
                    text,
                });
                marked = None;
                caret = Some(inserted.end..inserted.end);
            }
        }
    }
    if java.composing != marked {
        match java.composing.clone() {
            Some(composing) => {
                let selected = relative(java.selected_range(), &composing);
                caret = Some(match &selected {
                    Some(selected) => {
                        composing.start + selected.start..composing.start + selected.end
                    }
                    None => composing.end..composing.end,
                });
                edits.push(Edit::Mark {
                    text: String::from_utf16_lossy(&new[composing.clone()]),
                    range: composing,
                    selected,
                });
            }
            None => edits.push(Edit::Unmark),
        }
    }
    if let Some(selected) = java.selected_range()
        && caret.as_ref() != Some(&selected)
    {
        edits.push(Edit::Select(selected));
    }
    edits
}

/// The changed span of `old` → `new`, as (`old` range, `new` range): what is left after the common prefix and
/// suffix, widened so it never splits a surrogate pair.
fn changed_span(old: &[u16], new: &[u16]) -> (Range<usize>, Range<usize>) {
    let is_high = |unit: u16| (0xD800..0xDC00).contains(&unit);
    let is_low = |unit: u16| (0xDC00..0xE000).contains(&unit);
    let mut prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    if prefix > 0 && is_high(old[prefix - 1]) {
        prefix -= 1;
    }
    let most = old.len().min(new.len()) - prefix;
    let mut suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take(most)
        .take_while(|(a, b)| a == b)
        .count();
    if suffix > 0 && is_low(old[old.len() - suffix]) {
        suffix -= 1;
    }
    (prefix..old.len() - suffix, prefix..new.len() - suffix)
}

/// `selected` relative to `within`'s start, if `within` holds it.
fn relative(selected: Option<Range<usize>>, within: &Range<usize>) -> Option<Range<usize>> {
    selected
        .filter(|selected| within.start <= selected.start && selected.end <= within.end)
        .map(|selected| selected.start - within.start..selected.end - within.start)
}

/// A GPUI text-input notification, as the window receives them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    /// `TextInputStateChange::FocusGained`, or the focused field changed (another `FocusId`).
    Focused,
    /// `TextInputStateChange::FocusLost`.
    Blurred,
    /// `SelectionChanged` or `ContentChanged`, from the app (not from Java's own edit).
    Changed,
    /// `set_text_input_configuration` while a field is focused.
    Configured,
    /// `show_soft_keyboard` (`Window::request_virtual_keyboard`).
    Show,
    /// `hide_soft_keyboard` (`Window::dismiss_virtual_keyboard`).
    Hide,
}

/// The text input client on the Java side (Flutter's `TextInput.setClient` / `clearClient`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Client {
    #[default]
    Keep,
    /// Set a new client with the current configuration (restarts the IME).
    Set,
    Clear,
}

/// The soft keyboard (Flutter's `TextInput.show` / `hide`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Keyboard {
    #[default]
    Keep,
    Show,
    Hide,
}

/// What Java must do at the end of this frame. Empty (the default) sends nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pending {
    pub(crate) client: Client,
    /// Send GPUI's editing state (`TextInput.setEditingState`).
    pub(crate) sync: bool,
    pub(crate) keyboard: Keyboard,
}

impl Pending {
    /// Folds one notice in. Focus replaces everything; later keyboard requests win.
    pub(crate) fn next(self, notice: Notice) -> Pending {
        match (notice, self.client) {
            (Notice::Focused, _) => Pending {
                client: Client::Set,
                sync: true,
                keyboard: Keyboard::Show,
            },
            (Notice::Blurred, _) => Pending {
                client: Client::Clear,
                sync: false,
                keyboard: Keyboard::Hide,
            },
            // A cleared client has no state to sync or configure.
            (Notice::Changed | Notice::Configured, Client::Clear) => self,
            (Notice::Changed, Client::Keep | Client::Set) => Pending { sync: true, ..self },
            (Notice::Configured, Client::Keep | Client::Set) => Pending {
                client: Client::Set,
                sync: true,
                ..self
            },
            (Notice::Show, _) => Pending {
                keyboard: Keyboard::Show,
                ..self
            },
            (Notice::Hide, _) => Pending {
                keyboard: Keyboard::Hide,
                ..self
            },
        }
    }

    /// Whether Java's state is behind GPUI's: an edit Java makes now is stale.
    pub(crate) fn behind(&self) -> bool {
        self.sync || self.client != Client::Keep
    }

    /// Flutter's `flutter/textinput` calls for Java (`TextInputChannel.handleFrameworkCalls`), in its order:
    /// clear or set the client, then the editing state, then the keyboard. `None` when there is nothing to send.
    /// `state` is GPUI's (`None`: no readable field, so the state is skipped).
    pub(crate) fn calls(
        &self,
        configuration: &TextInputConfiguration,
        state: Option<&EditingState>,
    ) -> Option<String> {
        let client = match self.client {
            Client::Keep => None,
            Client::Set => Some(format!(
                r#"{{"method":"TextInput.setClient","args":[0,{}]}}"#,
                configuration_json(configuration)
            )),
            Client::Clear => Some(r#"{"method":"TextInput.clearClient"}"#.to_string()),
        };
        let sync = state.filter(|_| self.sync).map(|state| {
            format!(
                r#"{{"method":"TextInput.setEditingState","args":{}}}"#,
                state.to_json()
            )
        });
        let keyboard = match self.keyboard {
            Keyboard::Keep => None,
            Keyboard::Show => Some(r#"{"method":"TextInput.show"}"#.to_string()),
            Keyboard::Hide => Some(r#"{"method":"TextInput.hide"}"#.to_string()),
        };
        let calls: Vec<String> = [client, sync, keyboard].into_iter().flatten().collect();
        (!calls.is_empty()).then(|| format!("[{}]", calls.join(",")))
    }
}

/// Who is editing GPUI's text right now.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Origin {
    /// The app: its change notices are news for Java.
    #[default]
    Gpui,
    /// Java's edit is being applied: change notices are its echo.
    Java,
}

/// The window's text-input session, as GPUI reports it. Main thread only, in the shell.
#[derive(Debug, Default)]
pub(crate) struct Session {
    /// The focused field GPUI last installed; `None` once focus is lost.
    field: Option<FocusId>,
    pub(crate) configuration: TextInputConfiguration,
    pub(crate) pending: Pending,
    pub(crate) origin: Origin,
}

impl Session {
    /// `set_input_handler`: GPUI installs the focused field's handler (every frame, and around each query).
    pub(crate) fn installed(&mut self, field: Option<FocusId>) {
        // A first field is announced by FocusGained; another field needs a new client.
        if let Some(field) = field
            && self
                .field
                .replace(field)
                .is_some_and(|previous| previous != field)
        {
            self.notice(Notice::Focused);
        }
    }

    pub(crate) fn state_changed(&mut self, change: TextInputStateChange) {
        let notice = match change {
            TextInputStateChange::FocusGained => Notice::Focused,
            TextInputStateChange::FocusLost => {
                self.field = None;
                Notice::Blurred
            }
            TextInputStateChange::SelectionChanged | TextInputStateChange::ContentChanged => {
                Notice::Changed
            }
        };
        self.notice(notice);
    }

    pub(crate) fn configure(&mut self, configuration: TextInputConfiguration) {
        self.configuration = configuration;
        if self.field.is_some() {
            self.notice(Notice::Configured);
        }
    }

    /// Folds `notice` into the pending calls, except the echo of Java's own edit.
    pub(crate) fn notice(&mut self, notice: Notice) {
        match (self.origin, notice) {
            (Origin::Java, Notice::Changed) => {}
            (
                Origin::Java,
                Notice::Focused
                | Notice::Blurred
                | Notice::Configured
                | Notice::Show
                | Notice::Hide,
            )
            | (Origin::Gpui, _) => self.pending = self.pending.next(notice),
        }
    }
}

/// GPUI's capitalization → Flutter's `TextCapitalization` name. Add a row when GPUI adds a variant.
const CAPITALIZATION: [(Autocapitalize, &str); 4] = [
    (Autocapitalize::None, "TextCapitalization.none"),
    (Autocapitalize::Words, "TextCapitalization.words"),
    (Autocapitalize::Sentences, "TextCapitalization.sentences"),
    (Autocapitalize::Characters, "TextCapitalization.characters"),
];

/// GPUI's confirm-key action → Flutter's `TextInputAction` name (`Configuration.fromJson` maps it to
/// `EditorInfo.IME_ACTION_*`). Unspecified lets the IME choose. Add a row when GPUI adds a variant.
const ACTIONS: [(TextInputAction, &str); 8] = [
    (TextInputAction::Unspecified, "TextInputAction.unspecified"),
    (TextInputAction::Enter, "TextInputAction.newline"),
    (TextInputAction::Done, "TextInputAction.done"),
    (TextInputAction::Go, "TextInputAction.go"),
    (TextInputAction::Next, "TextInputAction.next"),
    (TextInputAction::Previous, "TextInputAction.previous"),
    (TextInputAction::Search, "TextInputAction.search"),
    (TextInputAction::Send, "TextInputAction.send"),
];

/// Flutter's `TextInputConfiguration` JSON (`Configuration.fromJson`) for GPUI's configuration.
fn configuration_json(configuration: &TextInputConfiguration) -> String {
    // Patch 0005 seam: keyboard type → "inputType" ("TextInputType.*"), secure → "obscureText" (and
    // "enableIMEPersonalizedLearning": false), autofill → "autofill" (dropped in Java for now).
    format!(
        r#"{{"inputType":{{"name":"TextInputType.text"}},"obscureText":false,"autocorrect":{},"enableSuggestions":{},"enableIMEPersonalizedLearning":true,"textCapitalization":"{}","inputAction":"{}"}}"#,
        configuration.autocorrect,
        configuration.suggestions,
        name(&CAPITALIZATION, configuration.autocapitalize),
        name(&ACTIONS, configuration.input_action),
    )
}

/// `key`'s name in a name table; the tables have a row per variant.
fn name<K: PartialEq>(table: &[(K, &'static str)], key: K) -> &'static str {
    table
        .iter()
        .find(|(value, _)| *value == key)
        .map_or("", |(_, name)| *name)
}

/// `text` as a JSON string literal.
fn json_string(text: &str) -> String {
    let mut json = String::with_capacity(text.len() + 2);
    json.push('"');
    for character in text.chars() {
        match character {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            character if character.is_control() && (character as u32) < 0x20 => {
                json.push_str(&format!("\\u{:04x}", character as u32))
            }
            character => json.push(character),
        }
    }
    json.push('"');
    json
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str, selection: (i32, i32), composing: (i32, i32)) -> EditingState {
        EditingState::from_java(text.to_string(), selection, composing)
    }

    fn replace(range: Range<usize>, text: &str) -> Edit {
        Edit::Replace {
            range,
            text: text.to_string(),
        }
    }

    fn mark(range: Range<usize>, text: &str, selected: Option<Range<usize>>) -> Edit {
        Edit::Mark {
            range,
            text: text.to_string(),
            selected,
        }
    }

    #[test]
    fn java_values() {
        // (name, Java's text, selection and composing) → the state's selection and composing range.
        let rows = [
            ("none", state("ab", (-1, -1), (-1, -1)), None, None),
            (
                "caret and composing",
                state("ab", (2, 2), (0, 2)),
                Some((2, 2)),
                Some(0..2),
            ),
            (
                "reversed selection kept",
                state("abc", (3, 1), (-1, -1)),
                Some((3, 1)),
                None,
            ),
            (
                "empty composing is none",
                state("ab", (1, 1), (1, 1)),
                Some((1, 1)),
                None,
            ),
            (
                "past the end is none",
                state("ab", (3, 3), (0, 3)),
                None,
                None,
            ),
            (
                "utf-16 length: emoji is 2",
                state("😀", (2, 2), (0, 2)),
                Some((2, 2)),
                Some(0..2),
            ),
        ];
        for (name, state, selection, composing) in rows {
            assert_eq!(state.selection, selection, "{name}: selection");
            assert_eq!(state.composing, composing, "{name}: composing");
        }
    }

    #[test]
    fn edits_from_java() {
        // (name, GPUI state, Java state, calls). States: (text, selection, composing), -1 for none.
        type Row = (&'static str, EditingState, EditingState, Vec<Edit>);
        let rows: Vec<Row> = vec![
            (
                "same state: nothing",
                state("ab", (2, 2), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                vec![],
            ),
            (
                "commitText appends",
                state("ab", (2, 2), (-1, -1)),
                state("abc", (3, 3), (-1, -1)),
                vec![replace(2..2, "c")],
            ),
            (
                "commitText inserts mid",
                state("ac", (1, 1), (-1, -1)),
                state("abc", (2, 2), (-1, -1)),
                vec![replace(1..1, "b")],
            ),
            (
                "delete before caret",
                state("abc", (3, 3), (-1, -1)),
                state("ab", (2, 2), (-1, -1)),
                vec![replace(2..3, "")],
            ),
            (
                "replace selection",
                state("abc", (0, 3), (-1, -1)),
                state("x", (1, 1), (-1, -1)),
                vec![replace(0..3, "x")],
            ),
            (
                "insert, caret elsewhere",
                state("ab", (2, 2), (-1, -1)),
                state("abc", (0, 0), (-1, -1)),
                vec![replace(2..2, "c"), Edit::Select(0..0)],
            ),
            (
                "setComposingText starts",
                state("a ", (2, 2), (-1, -1)),
                state("a h", (3, 3), (2, 3)),
                vec![mark(2..2, "h", Some(1..1))],
            ),
            (
                "setComposingText grows",
                state("a h", (3, 3), (2, 3)),
                state("a he", (4, 4), (2, 4)),
                vec![mark(2..3, "he", Some(2..2))],
            ),
            (
                "repeated letter stays one mark",
                state("a", (1, 1), (0, 1)),
                state("aa", (2, 2), (0, 2)),
                vec![mark(0..1, "aa", Some(2..2))],
            ),
            (
                "autocorrect replaces composing",
                state("teh", (3, 3), (0, 3)),
                state("the", (3, 3), (0, 3)),
                vec![mark(0..3, "the", Some(3..3))],
            ),
            (
                "finishComposingText unmarks",
                state("he", (2, 2), (0, 2)),
                state("he", (2, 2), (-1, -1)),
                vec![Edit::Unmark],
            ),
            (
                "commitText over composing",
                state("hel", (3, 3), (0, 3)),
                state("hello ", (6, 6), (-1, -1)),
                vec![replace(3..3, "lo ")],
            ),
            (
                "setComposingRegion on old text",
                state("hello", (5, 5), (-1, -1)),
                state("hello", (5, 5), (0, 5)),
                vec![mark(0..5, "hello", Some(5..5))],
            ),
            (
                "composing moved, caret outside",
                state("ab cd", (5, 5), (3, 5)),
                state("ab cd", (4, 4), (0, 2)),
                vec![mark(0..2, "ab", None), Edit::Select(4..4)],
            ),
            (
                "setSelection only",
                state("abc", (3, 3), (-1, -1)),
                state("abc", (0, 3), (-1, -1)),
                vec![Edit::Select(0..3)],
            ),
            (
                "reversed selection is a range",
                state("abc", (3, 3), (-1, -1)),
                state("abc", (3, 1), (-1, -1)),
                vec![Edit::Select(1..3)],
            ),
            (
                "CJK composing (pinyin → 你好)",
                state("nihao", (5, 5), (0, 5)),
                state("你好", (2, 2), (-1, -1)),
                vec![replace(0..5, "你好")],
            ),
            (
                "Arabic append",
                state("سلا", (3, 3), (-1, -1)),
                state("سلام", (4, 4), (-1, -1)),
                vec![replace(3..3, "م")],
            ),
            (
                "emoji is two units",
                state("a", (1, 1), (-1, -1)),
                state("a😀", (3, 3), (-1, -1)),
                vec![replace(1..1, "😀")],
            ),
            (
                "delete emoji whole",
                state("a😀", (3, 3), (-1, -1)),
                state("a", (1, 1), (-1, -1)),
                vec![replace(1..3, "")],
            ),
            // 😀 is D83D DE00, 😁 is D83D DE01: same high surrogate; the change must not split the pair.
            (
                "swap emoji, shared high surrogate",
                state("😀", (2, 2), (-1, -1)),
                state("😁", (2, 2), (-1, -1)),
                vec![replace(0..2, "😁")],
            ),
            // 😀 is D83D DE00, 𝘀 (U+1D600) is D835 DE00: same low surrogate.
            (
                "swap emoji, shared low surrogate",
                state("😀", (2, 2), (-1, -1)),
                state("𝘀", (2, 2), (-1, -1)),
                vec![replace(0..2, "𝘀")],
            ),
            (
                "insert before emoji",
                state("😀", (0, 0), (-1, -1)),
                state("x😀", (1, 1), (-1, -1)),
                vec![replace(0..0, "x")],
            ),
            (
                "ZWJ family appended",
                state("", (0, 0), (-1, -1)),
                state("👨‍👩‍👧", (8, 8), (-1, -1)),
                vec![replace(0..0, "👨‍👩‍👧")],
            ),
        ];
        for (name, gpui, java, want) in rows {
            assert_eq!(edits(&gpui, &java), want, "{name}");
        }
    }

    #[test]
    fn pending_transitions() {
        // One row per (state, notice) → next. States: empty, after focus, after blur.
        let empty = Pending::default();
        let set = Pending {
            client: Client::Set,
            sync: true,
            keyboard: Keyboard::Show,
        };
        let cleared = Pending {
            client: Client::Clear,
            sync: false,
            keyboard: Keyboard::Hide,
        };
        let synced = Pending {
            sync: true,
            ..empty
        };
        let reconfigured = Pending {
            client: Client::Set,
            sync: true,
            keyboard: Keyboard::Keep,
        };
        let rows = [
            ("empty, focused", empty, Notice::Focused, set),
            ("empty, blurred", empty, Notice::Blurred, cleared),
            ("empty, changed", empty, Notice::Changed, synced),
            ("empty, configured", empty, Notice::Configured, reconfigured),
            (
                "empty, show",
                empty,
                Notice::Show,
                Pending {
                    keyboard: Keyboard::Show,
                    ..empty
                },
            ),
            (
                "empty, hide",
                empty,
                Notice::Hide,
                Pending {
                    keyboard: Keyboard::Hide,
                    ..empty
                },
            ),
            ("set, focused", set, Notice::Focused, set),
            ("set, blurred", set, Notice::Blurred, cleared),
            ("set, changed", set, Notice::Changed, set),
            ("set, configured", set, Notice::Configured, set),
            (
                "set, hide",
                set,
                Notice::Hide,
                Pending {
                    keyboard: Keyboard::Hide,
                    ..set
                },
            ),
            ("cleared, focused", cleared, Notice::Focused, set),
            (
                "cleared, changed: ignored",
                cleared,
                Notice::Changed,
                cleared,
            ),
            (
                "cleared, configured: ignored",
                cleared,
                Notice::Configured,
                cleared,
            ),
            (
                "cleared, show",
                cleared,
                Notice::Show,
                Pending {
                    keyboard: Keyboard::Show,
                    ..cleared
                },
            ),
        ];
        for (name, state, notice, want) in rows {
            assert_eq!(state.next(notice), want, "{name}");
        }
        assert!(!empty.behind() && set.behind() && cleared.behind() && synced.behind());
    }

    #[test]
    fn session_notices() {
        // (name, origin, GPUI's change) → whether Java gets a state sync. Java's own edit is not news.
        let rows = [
            (
                "app edit",
                Origin::Gpui,
                TextInputStateChange::ContentChanged,
                true,
            ),
            (
                "app selection",
                Origin::Gpui,
                TextInputStateChange::SelectionChanged,
                true,
            ),
            (
                "echo of Java's edit",
                Origin::Java,
                TextInputStateChange::ContentChanged,
                false,
            ),
            (
                "echo of Java's selection",
                Origin::Java,
                TextInputStateChange::SelectionChanged,
                false,
            ),
            (
                "focus during Java's edit",
                Origin::Java,
                TextInputStateChange::FocusGained,
                true,
            ),
        ];
        for (name, origin, change, sync) in rows {
            let mut session = Session {
                origin,
                ..Session::default()
            };
            session.state_changed(change);
            assert_eq!(session.pending.sync, sync, "{name}");
        }
        // Configuration matters only while a field is focused.
        let mut session = Session::default();
        session.configure(TextInputConfiguration::default());
        assert_eq!(session.pending, Pending::default(), "no field: no client");
        session.installed(Some(FocusId::default()));
        session.configure(TextInputConfiguration::default());
        assert_eq!(session.pending.client, Client::Set, "focused: new client");
    }

    #[test]
    fn calls_for_java() {
        let configuration = TextInputConfiguration {
            autocorrect: true,
            autocapitalize: Autocapitalize::Sentences,
            suggestions: true,
            input_action: TextInputAction::Send,
        };
        let gpui = state("a\"😀\n", (4, 4), (1, 4));
        let rows: [(&str, Pending, Option<&str>); 4] = [
            ("nothing", Pending::default(), None),
            (
                "focus: client, state, show",
                Pending::default().next(Notice::Focused),
                Some(concat!(
                    r#"[{"method":"TextInput.setClient","args":[0,{"inputType":{"name":"TextInputType.text"},"#,
                    r#""obscureText":false,"autocorrect":true,"enableSuggestions":true,"#,
                    r#""enableIMEPersonalizedLearning":true,"textCapitalization":"TextCapitalization.sentences","#,
                    r#""inputAction":"TextInputAction.send"}]},"#,
                    r#"{"method":"TextInput.setEditingState","args":{"text":"a\"😀\u000a","selectionBase":4,"#,
                    r#""selectionExtent":4,"composingBase":1,"composingExtent":4}},"#,
                    r#"{"method":"TextInput.show"}]"#
                )),
            ),
            (
                "blur: clear, hide",
                Pending::default().next(Notice::Blurred),
                Some(r#"[{"method":"TextInput.clearClient"},{"method":"TextInput.hide"}]"#),
            ),
            (
                "hide only",
                Pending::default().next(Notice::Hide),
                Some(r#"[{"method":"TextInput.hide"}]"#),
            ),
        ];
        for (name, pending, want) in rows {
            assert_eq!(
                pending.calls(&configuration, Some(&gpui)).as_deref(),
                want,
                "{name}"
            );
        }
        let unreadable = Pending::default().next(Notice::Changed);
        assert_eq!(
            unreadable.calls(&configuration, None),
            None,
            "no field: no state"
        );
    }
}
