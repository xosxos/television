use super::*;

#[test]
fn test_theme_deserialization() {
    let theme_content = r##"
            background = "#000000"
            border_fg = "black"
            text_fg = "white"
            dimmed_text_fg = "bright-black"
            input_text_fg = "bright-white"
            result_count_fg = "bright-white"
            result_name_fg = "bright-white"
            result_line_number_fg = "bright-white"
            result_value_fg = "bright-white"
            selection_bg = "bright-white"
            selection_fg = "bright-white"
            match_fg = "bright-white"
            preview_title_fg = "bright-white"
            channel_mode_fg = "bright-white"
            remote_control_mode_fg = "bright-white"
            send_to_channel_mode_fg = "bright-white"
        "##;
    let theme: Theme = toml::from_str(theme_content).unwrap();
    assert_eq!(
        theme.background,
        Some(Color::Rgb(RGBColor::from_str("000000").unwrap()))
    );
    assert_eq!(theme.border_fg, Color::Ansi(ANSIColor::Black));
    assert_eq!(theme.text_fg, Color::Ansi(ANSIColor::White));
    assert_eq!(theme.dimmed_text_fg, Color::Ansi(ANSIColor::BrightBlack));
    assert_eq!(theme.input_text_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.result_count_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.result_name_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(
        theme.result_line_number_fg,
        Color::Ansi(ANSIColor::BrightWhite)
    );
    assert_eq!(theme.result_value_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.selection_bg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.selection_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.match_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.preview_title_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.channel_mode_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(
        theme.remote_control_mode_fg,
        Color::Ansi(ANSIColor::BrightWhite)
    );
    assert_eq!(
        theme.send_to_channel_mode_fg,
        Color::Ansi(ANSIColor::BrightWhite)
    );
}

#[test]
fn test_theme_deserialization_no_background() {
    let theme_content = r##"
            border_fg = "black"
            text_fg = "white"
            dimmed_text_fg = "bright-black"
            input_text_fg = "bright-white"
            result_count_fg = "#ffffff"
            result_name_fg = "bright-white"
            result_line_number_fg = "#ffffff"
            result_value_fg = "bright-white"
            selection_bg = "bright-white"
            selection_fg = "bright-white"
            match_fg = "bright-white"
            preview_title_fg = "bright-white"
            channel_mode_fg = "bright-white"
            remote_control_mode_fg = "bright-white"
            send_to_channel_mode_fg = "bright-white"
        "##;
    let theme: Theme = toml::from_str(theme_content).unwrap();
    assert_eq!(theme.background, None);
    assert_eq!(theme.border_fg, Color::Ansi(ANSIColor::Black));
    assert_eq!(theme.text_fg, Color::Ansi(ANSIColor::White));
    assert_eq!(theme.dimmed_text_fg, Color::Ansi(ANSIColor::BrightBlack));
    assert_eq!(theme.input_text_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(
        theme.result_count_fg,
        Color::Rgb(RGBColor::from_str("ffffff").unwrap())
    );
    assert_eq!(theme.result_name_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(
        theme.result_line_number_fg,
        Color::Rgb(RGBColor::from_str("ffffff").unwrap())
    );
    assert_eq!(theme.result_value_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.selection_bg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.selection_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.match_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.preview_title_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(theme.channel_mode_fg, Color::Ansi(ANSIColor::BrightWhite));
    assert_eq!(
        theme.remote_control_mode_fg,
        Color::Ansi(ANSIColor::BrightWhite)
    );
    assert_eq!(
        theme.send_to_channel_mode_fg,
        Color::Ansi(ANSIColor::BrightWhite)
    );
}
