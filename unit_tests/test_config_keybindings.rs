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
