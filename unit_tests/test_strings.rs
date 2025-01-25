use super::*;

fn test_next_char_boundary(input: &str, start: usize, expected: usize) {
    let actual = next_char_boundary(input, start);
    assert_eq!(actual, expected);
}

#[test]
fn test_next_char_boundary_ascii() {
    test_next_char_boundary("Hello, World!", 0, 0);
    test_next_char_boundary("Hello, World!", 1, 1);
    test_next_char_boundary("Hello, World!", 13, 13);
    test_next_char_boundary("Hello, World!", 30, 13);
}

#[test]
fn test_next_char_boundary_emoji() {
    test_next_char_boundary("👋🌍!", 0, 0);
    test_next_char_boundary("👋🌍!", 1, 4);
    test_next_char_boundary("👋🌍!", 4, 4);
    test_next_char_boundary("👋🌍!", 8, 8);
    test_next_char_boundary("👋🌍!", 7, 8);
}

fn test_previous_char_boundary(input: &str, start: usize, expected: usize) {
    let actual = prev_char_boundary(input, start);
    assert_eq!(actual, expected);
}

#[test]
fn test_previous_char_boundary_ascii() {
    test_previous_char_boundary("Hello, World!", 0, 0);
    test_previous_char_boundary("Hello, World!", 1, 1);
    test_previous_char_boundary("Hello, World!", 5, 5);
}

#[test]
fn test_previous_char_boundary_emoji() {
    test_previous_char_boundary("👋🌍!", 0, 0);
    test_previous_char_boundary("👋🌍!", 4, 4);
    test_previous_char_boundary("👋🌍!", 6, 4);
    test_previous_char_boundary("👋🌍!", 8, 8);
}

fn test_slice_at_char_boundaries(input: &str, start: usize, end: usize, expected: &str) {
    let actual = slice_at_char_boundaries(input, start, end);
    assert_eq!(actual, expected);
}

#[test]
fn test_slice_at_char_boundaries_ascii() {
    test_slice_at_char_boundaries("Hello, World!", 0, 0, "");
    test_slice_at_char_boundaries("Hello, World!", 0, 1, "H");
    test_slice_at_char_boundaries("Hello, World!", 0, 13, "Hello, World!");
    test_slice_at_char_boundaries("Hello, World!", 0, 30, "");
}

#[test]
fn test_slice_at_char_boundaries_emoji() {
    test_slice_at_char_boundaries("👋🌍!", 0, 0, "");
    test_slice_at_char_boundaries("👋🌍!", 0, 4, "👋");
    test_slice_at_char_boundaries("👋🌍!", 0, 8, "👋🌍");
    test_slice_at_char_boundaries("👋🌍!", 0, 7, "👋🌍");
    test_slice_at_char_boundaries("👋🌍!", 0, 9, "👋🌍!");
}

fn test_replace_non_printable(input: &str, expected: &str) {
    let (actual, _offset) = replace_non_printable(
        input.as_bytes(),
        ReplaceNonPrintableConfig::default().tab_width(2),
    );
    assert_eq!(actual, expected);
}

#[test]
fn test_replace_non_printable_ascii() {
    test_replace_non_printable("Hello, World!", "Hello, World!");
}

#[test]
fn test_replace_non_printable_tab() {
    test_replace_non_printable("Hello\tWorld!", "Hello  World!");
    test_replace_non_printable(
        "	-- AND
", "  -- AND",
    );
}

#[test]
fn test_replace_non_printable_line_feed() {
    test_replace_non_printable("Hello\nWorld!", "HelloWorld!");
}

#[test]
fn test_replace_non_printable_null() {
    test_replace_non_printable("Hello\x00World!", "Hello␀World!");
    test_replace_non_printable("Hello World!\0", "Hello World!␀");
}

#[test]
fn test_replace_non_printable_delete() {
    test_replace_non_printable("Hello\x7FWorld!", "Hello␀World!");
}

#[test]
fn test_replace_non_printable_bom() {
    test_replace_non_printable("Hello\u{FEFF}World!", "Hello␀World!");
}

#[test]
fn test_replace_non_printable_start_txt() {
    test_replace_non_printable("Àì", "Àì␀");
}

#[test]
fn test_replace_non_printable_range_tab() {
    let input = b"Hello,\tWorld!";
    let (output, offsets) = replace_non_printable(input, &ReplaceNonPrintableConfig::default());
    assert_eq!(output, "Hello,    World!");
    assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3]);
}

#[test]
fn test_replace_non_printable_range_line_feed() {
    let input = b"Hello,\nWorld!";
    let (output, offsets) =
        replace_non_printable(input, ReplaceNonPrintableConfig::default().tab_width(2));
    assert_eq!(output, "Hello,World!");
    assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, -1, -1, -1, -1, -1, -1]);
}

#[test]
fn test_replace_non_printable_no_range_changes() {
    let input = b"Hello,\x00World!";
    let (output, offsets) =
        replace_non_printable(input, ReplaceNonPrintableConfig::default().tab_width(2));
    assert_eq!(output, "Hello,␀World!");
    assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    let input = b"Hello,\x7FWorld!";
    let (output, offsets) =
        replace_non_printable(input, ReplaceNonPrintableConfig::default().tab_width(2));
    assert_eq!(output, "Hello,␀World!");
    assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

fn test_proportion_of_printable_ascii_characters(input: &str, expected: f32) {
    let actual = proportion_of_printable_ascii_characters(input.as_bytes());
    assert_eq!(actual, expected);
}

#[test]
fn test_proportion_of_printable_ascii_characters_ascii() {
    test_proportion_of_printable_ascii_characters("Hello, World!", 1.0);
    test_proportion_of_printable_ascii_characters("Hello, World!\x00", 0.928_571_4);
    test_proportion_of_printable_ascii_characters(
        "\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F",
        0.0,
    );
}

fn test_preprocess_line(input: &str, expected: &str) {
    let (actual, _offset) = preprocess_line(input);
    assert_eq!(actual, expected, "input: {:?}", input);
}

#[test]
fn test_preprocess_line_cases() {
    test_preprocess_line("Hello, World!", "Hello, World!");
    test_preprocess_line("Hello, World!\n", "Hello, World!");
    test_preprocess_line("Hello, World!\x00", "Hello, World!␀");
    test_preprocess_line("Hello, World!\x7F", "Hello, World!␀");
    test_preprocess_line("Hello, World!\u{FEFF}", "Hello, World!␀");
    test_preprocess_line(&"a".repeat(400), &"a".repeat(300));
}
