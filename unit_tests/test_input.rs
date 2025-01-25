const TEXT: &str = "first second, third.";

use super::*;

#[test]
fn format() {
    let input: Input = TEXT.into();
    println!("{}", input);
    println!("{}", input);
}

#[test]
fn set_cursor() {
    let mut input: Input = TEXT.into();

    let req = InputRequest::SetCursor(3);
    let resp = input.handle(req);

    assert_eq!(
        resp,
        Some(StateChanged {
            value: false,
            cursor: true,
        })
    );

    assert_eq!(input.value(), "first second, third.");
    assert_eq!(input.cursor(), 3);

    let req = InputRequest::SetCursor(30);
    let resp = input.handle(req);

    assert_eq!(input.cursor(), TEXT.chars().count());
    assert_eq!(
        resp,
        Some(StateChanged {
            value: false,
            cursor: true,
        })
    );

    let req = InputRequest::SetCursor(TEXT.chars().count());
    let resp = input.handle(req);

    assert_eq!(input.cursor(), TEXT.chars().count());
    assert_eq!(resp, None);
}

#[test]
fn insert_char() {
    let mut input: Input = TEXT.into();

    let req = InputRequest::InsertChar('x');
    let resp = input.handle(req);

    assert_eq!(
        resp,
        Some(StateChanged {
            value: true,
            cursor: true,
        })
    );

    assert_eq!(input.value(), "first second, third.x");
    assert_eq!(input.cursor(), TEXT.chars().count() + 1);
    input.handle(req);
    assert_eq!(input.value(), "first second, third.xx");
    assert_eq!(input.cursor(), TEXT.chars().count() + 2);

    let mut input = input.with_cursor(3);
    input.handle(req);
    assert_eq!(input.value(), "firxst second, third.xx");
    assert_eq!(input.cursor(), 4);

    input.handle(req);
    assert_eq!(input.value(), "firxxst second, third.xx");
    assert_eq!(input.cursor(), 5);
}

#[test]
fn go_to_prev_char() {
    let mut input: Input = TEXT.into();

    let req = InputRequest::GoToPrevChar;
    let resp = input.handle(req);

    assert_eq!(
        resp,
        Some(StateChanged {
            value: false,
            cursor: true,
        })
    );

    assert_eq!(input.value(), "first second, third.");
    assert_eq!(input.cursor(), TEXT.chars().count() - 1);

    let mut input = input.with_cursor(3);
    input.handle(req);
    assert_eq!(input.value(), "first second, third.");
    assert_eq!(input.cursor(), 2);

    input.handle(req);
    assert_eq!(input.value(), "first second, third.");
    assert_eq!(input.cursor(), 1);
}

#[test]
fn remove_unicode_chars() {
    let mut input: Input = "¡test¡".into();

    let req = InputRequest::DeletePrevChar;
    let resp = input.handle(req);

    assert_eq!(
        resp,
        Some(StateChanged {
            value: true,
            cursor: true,
        })
    );

    assert_eq!(input.value(), "¡test");
    assert_eq!(input.cursor(), 5);

    input.handle(InputRequest::GoToStart);

    let req = InputRequest::DeleteNextChar;
    let resp = input.handle(req);

    assert_eq!(
        resp,
        Some(StateChanged {
            value: true,
            cursor: false,
        })
    );

    assert_eq!(input.value(), "test");
    assert_eq!(input.cursor(), 0);
}

#[test]
fn insert_unicode_chars() {
    let mut input = Input::from("¡test¡").with_cursor(5);

    let req = InputRequest::InsertChar('☆');
    let resp = input.handle(req);

    assert_eq!(
        resp,
        Some(StateChanged {
            value: true,
            cursor: true,
        })
    );

    assert_eq!(input.value(), "¡test☆¡");
    assert_eq!(input.cursor(), 6);

    input.handle(InputRequest::GoToStart);
    input.handle(InputRequest::GoToNextChar);

    let req = InputRequest::InsertChar('☆');
    let resp = input.handle(req);

    assert_eq!(
        resp,
        Some(StateChanged {
            value: true,
            cursor: true,
        })
    );

    assert_eq!(input.value(), "¡☆test☆¡");
    assert_eq!(input.cursor(), 2);
}

#[test]
fn multispace_characters() {
    let input: Input = "Ｈｅｌｌｏ, ｗｏｒｌｄ!".into();
    assert_eq!(input.cursor(), 13);
    assert_eq!(input.visual_cursor(), 23);
    assert_eq!(input.visual_scroll(6), 18);
}
