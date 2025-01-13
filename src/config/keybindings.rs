use std::fmt;
use std::{fmt::Display, ops::Deref};

use crossterm::event::{KeyCode, KeyModifiers};
use serde::{Deserialize, Deserializer};

use crate::action::Action;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEvent(pub crossterm::event::KeyEvent);

impl KeyEvent {
    fn new(key: KeyCode, modifiers: KeyModifiers) -> Self {
        KeyEvent(crossterm::event::KeyEvent::new(key, modifiers))
    }
}

impl fmt::Display for KeyEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let modifier = match self.0.modifiers {
            KeyModifiers::SHIFT => String::from("Shift"),
            KeyModifiers::CONTROL => String::from("Ctrl"),
            KeyModifiers::ALT => String::from("Alt"),
            e => e.to_string(),
        };

        if modifier.is_empty() {
            return write!(f, "{}", self.0.code);
        }

        let key = self.0.code.to_string().to_uppercase();

        if key == "BACK TAB" {
            write!(f, "{modifier}-Tab")
        } else {
            write!(f, "{modifier}-{}", key)
        }
    }
}

impl From<crossterm::event::KeyEvent> for KeyEvent {
    fn from(value: crossterm::event::KeyEvent) -> Self {
        KeyEvent(value)
    }
}

impl Deref for KeyEvent {
    type Target = crossterm::event::KeyEvent;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub enum Binding {
    SingleKey(KeyEvent),
    MultipleKeys(Vec<KeyEvent>),
}

#[derive(Clone, Debug, Deserialize)]
pub struct KeyBindings {
    pub quit: Binding,
    pub select_next_entry: Binding,
    pub select_prev_entry: Binding,
    pub select_next_page: Binding,
    pub select_prev_page: Binding,
    pub select_prev_preview: Binding,
    pub select_next_preview: Binding,
    pub select_prev_run: Binding,
    pub select_next_run: Binding,
    pub toggle_remote_control: Binding,
    pub toggle_transition: Binding,
    pub toggle_preview_commands: Binding,
    pub toggle_run_commands: Binding,
    pub toggle_help: Binding,
    pub toggle_logs: Binding,
    pub toggle_preview: Binding,
    pub scroll_preview_half_page_up: Binding,
    pub scroll_preview_half_page_down: Binding,
    pub scroll_log_up: Binding,
    pub scroll_log_down: Binding,
    pub toggle_selection_down: Binding,
    pub toggle_selection_up: Binding,
    pub confirm_selection: Binding,
    pub copy_entry_to_clipboard: Binding,
}

macro_rules! impl_binding {
    ($name:ident, $k:tt) => {
        pub fn $name(&self) -> (&Binding, Action) {
            (&self.$name, Action::$k)
        }
    };
}

impl KeyBindings {
    pub fn check_key_for_action(&self, key: &KeyEvent) -> Option<Action> {
        // Could be mapped to get O(1), but I don't think it matters much
        [
            self.quit(),
            self.select_next_entry(),
            self.select_prev_entry(),
            self.select_next_page(),
            self.select_prev_page(),
            self.select_next_preview(),
            self.select_prev_preview(),
            self.select_next_run(),
            self.select_prev_run(),
            self.toggle_remote_control(),
            self.toggle_transition(),
            self.toggle_run_commands(),
            self.toggle_preview_commands(),
            self.toggle_help(),
            self.toggle_logs(),
            self.toggle_preview(),
            self.scroll_preview_half_page_up(),
            self.scroll_preview_half_page_down(),
            self.scroll_log_up(),
            self.scroll_log_down(),
            self.toggle_selection_down(),
            self.toggle_selection_up(),
            self.confirm_selection(),
            self.copy_entry_to_clipboard(),
        ]
        .into_iter()
        .find_map(|(binding, action)| {
            match binding {
                Binding::SingleKey(k) => k.0.code == key.0.code && k.0.modifiers == key.0.modifiers,
                Binding::MultipleKeys(vec) => vec
                    .iter()
                    .any(|k| k.code == key.code && k.modifiers == key.modifiers),
            }
            .then_some(action)
        })
    }

    // Match bindings and actions
    impl_binding!(quit, Quit);
    impl_binding!(select_next_entry, SelectNextEntry);
    impl_binding!(select_prev_entry, SelectPrevEntry);
    impl_binding!(select_next_page, SelectNextPage);
    impl_binding!(select_prev_page, SelectPrevPage);
    impl_binding!(select_next_preview, SelectNextPreview);
    impl_binding!(select_prev_preview, SelectPrevPreview);
    impl_binding!(select_next_run, SelectNextRun);
    impl_binding!(select_prev_run, SelectPrevRun);
    impl_binding!(toggle_remote_control, ToggleRemoteControl);
    impl_binding!(toggle_transition, ToggleTransition);
    impl_binding!(toggle_run_commands, ToggleRunCommands);
    impl_binding!(toggle_preview_commands, TogglePreviewCommands);
    impl_binding!(toggle_help, ToggleHelp);
    impl_binding!(toggle_logs, ToggleLogs);
    impl_binding!(toggle_preview, TogglePreview);
    impl_binding!(scroll_preview_half_page_up, ScrollPreviewHalfPageUp);
    impl_binding!(scroll_preview_half_page_down, ScrollPreviewHalfPageDown);
    impl_binding!(scroll_log_up, ScrollLogUp);
    impl_binding!(scroll_log_down, ScrollLogDown);
    impl_binding!(toggle_selection_down, ToggleSelectionDown);
    impl_binding!(toggle_selection_up, ToggleSelectionUp);
    impl_binding!(confirm_selection, ConfirmSelection);
    impl_binding!(copy_entry_to_clipboard, CopyEntryToClipboard);
}

impl Display for Binding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Binding::SingleKey(key) => write!(f, "{key}"),
            Binding::MultipleKeys(keys) => {
                let output = keys
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");

                write!(f, "{output}")
            }
        }
    }
}

impl<'de> Deserialize<'de> for Binding {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Do this to not consume `deserializer` on the first .deserialize()
        let content = <serde::__private::de::Content as Deserialize>::deserialize(deserializer)?;
        let deserializer = serde::__private::de::ContentRefDeserializer::<D::Error>::new(&content);

        // Parse SingleKey to String first
        if let Ok(key) = <String>::deserialize(deserializer) {
            let key = parse_key(&key).unwrap_or_else(|_| panic!("failed to parse key {key}"));
            return Ok(Binding::SingleKey(key));
        }

        // Parse MultipleKey to Vec<String> first
        if let Ok(keys) = <Vec<String>>::deserialize(deserializer) {
            return Ok(Binding::MultipleKeys(
                keys.into_iter()
                    .map(|key| {
                        parse_key(&key).unwrap_or_else(|_| panic!("failed to parse key {key}"))
                    })
                    .collect(),
            ));
        }

        Err(serde::de::Error::custom(format!(
            "data {content:?} did not match any variant of untagged enum Binding"
        )))
    }
}

pub fn parse_key(raw: &str) -> color_eyre::Result<KeyEvent, String> {
    if raw.chars().filter(|c| *c == '>').count() != raw.chars().filter(|c| *c == '<').count() {
        return Err(format!("Unable to parse `{raw}`"));
    }

    let raw = match raw.contains("><") {
        true => raw,
        false => {
            let raw = raw.strip_prefix('<').unwrap_or(raw);
            raw.strip_suffix('>').unwrap_or(raw)
        }
    }
    .to_ascii_lowercase();

    let mut raw_keycode = raw.as_str();
    let mut modifiers = KeyModifiers::empty();

    loop {
        match raw_keycode {
            rest if rest.starts_with("ctrl-") => {
                modifiers.insert(KeyModifiers::CONTROL);
                raw_keycode = &rest[5..];
            }
            rest if rest.starts_with("alt-") => {
                modifiers.insert(KeyModifiers::ALT);
                raw_keycode = &rest[4..];
            }
            rest if rest.starts_with("shift-") => {
                modifiers.insert(KeyModifiers::SHIFT);
                raw_keycode = &rest[6..];
            }
            _ => break, // break out of the loop if no known prefix is detected
        };
    }

    parse_key_code_with_modifiers(raw_keycode, modifiers)
}

fn parse_key_code_with_modifiers(
    raw_keycode: &str,
    mut modifiers: KeyModifiers,
) -> color_eyre::Result<KeyEvent, String> {
    let keycode = match raw_keycode {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "home" => KeyCode::Home,
        "end" => KeyCode::End,
        "pageup" => KeyCode::PageUp,
        "pagedown" => KeyCode::PageDown,
        "backtab" => {
            modifiers.insert(KeyModifiers::SHIFT);
            KeyCode::BackTab
        }
        "backspace" => KeyCode::Backspace,
        "delete" => KeyCode::Delete,
        "insert" => KeyCode::Insert,
        "f1" => KeyCode::F(1),
        "f2" => KeyCode::F(2),
        "f3" => KeyCode::F(3),
        "f4" => KeyCode::F(4),
        "f5" => KeyCode::F(5),
        "f6" => KeyCode::F(6),
        "f7" => KeyCode::F(7),
        "f8" => KeyCode::F(8),
        "f9" => KeyCode::F(9),
        "f10" => KeyCode::F(10),
        "f11" => KeyCode::F(11),
        "f12" => KeyCode::F(12),
        "space" => KeyCode::Char(' '),
        "hyphen" | "minus" => KeyCode::Char('-'),
        "tab" => KeyCode::Tab,
        c if c.len() == 1 => {
            let mut c = c.chars().next().unwrap();
            if modifiers.contains(KeyModifiers::SHIFT) {
                c = c.to_ascii_uppercase();
            }
            KeyCode::Char(c)
        }
        _ => return Err(format!("Unable to parse {raw_keycode}")),
    };

    Ok(KeyEvent::new(keycode, modifiers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_keys() {
        assert_eq!(
            parse_key("a").unwrap(),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty())
        );

        assert_eq!(
            parse_key("enter").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::empty())
        );

        assert_eq!(
            parse_key("esc").unwrap(),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::empty())
        );
    }

    #[test]
    fn test_with_modifiers() {
        assert_eq!(
            parse_key("ctrl-a").unwrap(),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        assert_eq!(
            parse_key("alt-enter").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );

        assert_eq!(
            parse_key("shift-esc").unwrap(),
            KeyEvent::new(KeyCode::Esc, KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_multiple_modifiers() {
        assert_eq!(
            parse_key("ctrl-alt-a").unwrap(),
            KeyEvent::new(
                KeyCode::Char('a'),
                KeyModifiers::CONTROL | KeyModifiers::ALT
            )
        );

        assert_eq!(
            parse_key("ctrl-shift-enter").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL | KeyModifiers::SHIFT)
        );
    }

    #[test]
    fn test_invalid_keys() {
        assert!(parse_key("invalid-key").is_err());
        assert!(parse_key("ctrl-invalid-key").is_err());
    }

    #[test]
    fn test_case_insensitivity() {
        assert_eq!(
            parse_key("CTRL-a").unwrap(),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)
        );

        assert_eq!(
            parse_key("AlT-eNtEr").unwrap(),
            KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT)
        );
    }
}
