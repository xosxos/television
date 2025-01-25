use super::*;

#[test]
fn test_parse_style_default() {
    let style = parse_style("");
    assert_eq!(style, Style::default());
}

#[test]
fn test_parse_style_foreground() {
    let style = parse_style("red");
    assert_eq!(style.fg, Some(Color::Indexed(1)));
}

#[test]
fn test_parse_style_background() {
    let style = parse_style("on blue");
    assert_eq!(style.bg, Some(Color::Indexed(4)));
}

#[test]
fn test_parse_style_modifiers() {
    let style = parse_style("underline red on blue");
    assert_eq!(style.fg, Some(Color::Indexed(1)));
    assert_eq!(style.bg, Some(Color::Indexed(4)));
}

#[test]
fn test_process_color_string() {
    let (color, modifiers) = process_color_string("underline bold inverse gray");
    assert_eq!(color, "gray");
    assert!(modifiers.contains(Modifier::UNDERLINED));
    assert!(modifiers.contains(Modifier::BOLD));
    assert!(modifiers.contains(Modifier::REVERSED));
}

#[test]
fn test_parse_color_rgb() {
    let color = parse_color("rgb123");
    let expected = 16 + 36 + 2 * 6 + 3;
    assert_eq!(color, Some(Color::Indexed(expected)));
}

#[test]
fn test_parse_color_unknown() {
    let color = parse_color("unknown");
    assert_eq!(color, None);
}
