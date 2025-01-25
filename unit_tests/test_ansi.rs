use super::*;

#[test]
fn color_test() {
    let c = color(b"2;255;255;255").unwrap();
    assert_eq!(c.1, Color::Rgb(255, 255, 255));
    let c = color(b"5;255").unwrap();
    assert_eq!(c.1, Color::Indexed(255));
    let err = color(b"10;255");
    assert_ne!(err, Ok(c));
}

#[test]
fn test_color_reset() {
    let t = ansi_to_text(b"\x1b[33msome arbitrary text\x1b[0m\nmore text");
    assert_eq!(
        t,
        Text::from(vec![
            Line::from(vec![Span::styled(
                "some arbitrary text",
                Style::default().fg(Color::Yellow)
            ),]),
            Line::from(Span::from("more text").fg(Color::Reset)),
        ])
    );
}

#[test]
fn test_color_reset_implicit_escape() {
    let t = ansi_to_text(b"\x1b[33msome arbitrary text\x1b[m\nmore text");
    assert_eq!(
        t,
        Text::from(vec![
            Line::from(vec![Span::styled(
                "some arbitrary text",
                Style::default().fg(Color::Yellow)
            ),]),
            Line::from(Span::from("more text").fg(Color::Reset)),
        ])
    );
}

#[test]
fn ansi_items_test() {
    let sc = Style::default();
    let t = style(sc)(b"\x1b[38;2;3;3;3m").unwrap().1.unwrap();
    assert_eq!(
        t,
        Style::from(AnsiStates {
            style: sc,
            items: vec![AnsiItem {
                code: AnsiCode::SetForegroundColor,
                color: Some(Color::Rgb(3, 3, 3))
            }]
            .into()
        })
    );
    assert_eq!(
        style(sc)(b"\x1b[38;5;3m").unwrap().1.unwrap(),
        Style::from(AnsiStates {
            style: sc,
            items: vec![AnsiItem {
                code: AnsiCode::SetForegroundColor,
                color: Some(Color::Indexed(3))
            }]
            .into()
        })
    );
    assert_eq!(
        style(sc)(b"\x1b[38;5;3;48;5;3m").unwrap().1.unwrap(),
        Style::from(AnsiStates {
            style: sc,
            items: vec![
                AnsiItem {
                    code: AnsiCode::SetForegroundColor,
                    color: Some(Color::Indexed(3))
                },
                AnsiItem {
                    code: AnsiCode::SetBackgroundColor,
                    color: Some(Color::Indexed(3))
                }
            ]
            .into()
        })
    );
    assert_eq!(
        style(sc)(b"\x1b[38;5;3;48;5;3;1m").unwrap().1.unwrap(),
        Style::from(AnsiStates {
            style: sc,
            items: vec![
                AnsiItem {
                    code: AnsiCode::SetForegroundColor,
                    color: Some(Color::Indexed(3))
                },
                AnsiItem {
                    code: AnsiCode::SetBackgroundColor,
                    color: Some(Color::Indexed(3))
                },
                AnsiItem {
                    code: AnsiCode::Bold,
                    color: None
                }
            ]
            .into()
        })
    );
}
