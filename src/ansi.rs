#![allow(unused_imports)]
//! This module provides a way to parse ansi escape codes and convert them to ratatui objects.
//!
//! This code is a modified version of [ansi_to_tui](https://github.com/ratatui/ansi-to-tui).
pub mod parser {
    use crate::ansi::AnsiCode;
    use nom::{
        branch::alt,
        bytes::complete::{tag, take, take_till, take_while},
        character::{
            complete::{char, i64, not_line_ending, u8},
            is_alphabetic,
        },
        combinator::{map_res, opt, recognize, value},
        error::{self, FromExternalError},
        multi::fold_many0,
        sequence::{delimited, preceded, terminated, tuple},
        IResult, Parser,
    };
    use ratatui::{
        style::{Color, Modifier, Style, Stylize},
        text::{Line, Span, Text},
    };
    use smallvec::{SmallVec, ToSmallVec, smallvec};
    use std::str::FromStr;

    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    enum ColorType {
        EightBit,
        TrueColor,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct AnsiItem {
        code: AnsiCode,
        color: Option<Color>,
    }

    #[derive(Debug, Clone, PartialEq)]
    struct AnsiStates {
        pub items: smallvec::SmallVec<[AnsiItem; 2]>,
        pub style: Style,
    }

    impl From<AnsiStates> for ratatui::style::Style {
        fn from(states: AnsiStates) -> Self {
            let mut style = states.style;
            for item in states.items {
                match item.code {
                    AnsiCode::Bold => style = style.add_modifier(Modifier::BOLD),
                    AnsiCode::Faint => style = style.add_modifier(Modifier::DIM),
                    AnsiCode::Normal => {
                        style = style
                            .remove_modifier(Modifier::BOLD)
                            .remove_modifier(Modifier::DIM);
                    }
                    AnsiCode::Italic => {
                        style = style.add_modifier(Modifier::ITALIC);
                    }
                    AnsiCode::Underline => {
                        style = style.add_modifier(Modifier::UNDERLINED);
                    }
                    AnsiCode::SlowBlink => {
                        style = style.add_modifier(Modifier::SLOW_BLINK);
                    }
                    AnsiCode::RapidBlink => {
                        style = style.add_modifier(Modifier::RAPID_BLINK);
                    }
                    AnsiCode::Reverse => {
                        style = style.add_modifier(Modifier::REVERSED);
                    }
                    AnsiCode::Conceal => {
                        style = style.add_modifier(Modifier::HIDDEN);
                    }
                    AnsiCode::CrossedOut => {
                        style = style.add_modifier(Modifier::CROSSED_OUT);
                    }
                    AnsiCode::DefaultForegroundColor => {
                        style = style.fg(Color::Reset);
                    }
                    AnsiCode::SetForegroundColor => {
                        if let Some(color) = item.color {
                            style = style.fg(color);
                        }
                    }
                    AnsiCode::ForegroundColor(color) => style = style.fg(color),
                    AnsiCode::Reset => style = style.fg(Color::Reset),
                    _ => (),
                }
            }
            style
        }
    }

    pub(crate) fn text(mut bytes: &[u8]) -> Text<'_> {
        let mut lines = Vec::new();
        let mut last_style = Style::new();
        
        while let Ok((remaining_bytes, (line, style))) = line(last_style, bytes) {
            lines.push(line);
            last_style = style;
            
            if remaining_bytes.is_empty() {
                break;
            }
        }
        
        Text::from(lines)
    }

    fn line(style: Style, bytes: &[u8]) -> IResult<&[u8], (Line<'_>, Style)> {
        let (bytes, mut span_bytes) = not_line_ending(bytes)?;
        let (bytes, _) = opt(alt((tag("\r\n"), tag("\n"))))(bytes)?;
        let mut spans = Vec::new();
        let mut last_style = style;
            
        while let Ok((remaining_bytes, span)) = span(last_style, span_bytes) {
            last_style = last_style.patch(span.style);
            
            // If the spans is empty then it might be possible that the style changes
            // but there is no text change
            if !span.content.is_empty() {
                spans.push(span);
            }
            
            remaining_bytes.is_empty() {
                break,
            }
        }

        Ok((bytes, (Line::from(spans), last)))
    }

    fn span(
        last_style: Style,
        bytes: &[u8],
    ) -> IResult<&[u8], Span<'_>, nom::error::Error<&[u8]>> {
        let (bytes, style) = style(last_style, bytes)?;
        
        let style = match style.flatten() {
            Some(style) => last_style.patch(style),
            None => last_style,
        }
            
        #[cfg(feature = "simd")]
        let (bytes, text) = map_res(take_while(|c| c != b'\x1b'), |t| {
            simdutf8::basic::from_utf8(t)
        })(bytes)?;

        #[cfg(not(feature = "simd"))]
        let (bytes, text) = map_res(take_while(|c| c != b'\x1b'), |t| std::str::from_utf8(t))(bytes)?;

        Ok((bytes, Span::styled(text, style)))
    }

    fn style(
        style: Style,
        bytes: &[u8],
    ) -> IResult<&[u8], Option<Style>, nom::error::Error<&[u8]>> {
        let (bytes, ansi_items) = opt(delimited(
            tag("\x1b["),
            fold_many0(parse_ansi_sgr_item, SmallVec::new, |mut items, item| {
                items.push(item);
                items
            }),
            char('m'),
        ))(bytes)?;

        let (bytes, ansi_items) = match ansi_items {
            Some(items) => {
                // This would correspond to an implicit reset code (\x1b[m)
                let items = match items.is_empty() {
                    true => smallvec![AnsiItem { code: AnsiCode::Reset, color: None }; 2],
                    false => items,
                };
                
                (bytes, Some(items))
            }
            None => (any_escape_sequence(bytes)?.0, None),
        };
        
        Ok((bytes, ansi_items.map(|items| Style::from(AnsiStates { style, items }))))
    }

    /// Parse ANSI Select Graphic Rendition (SGR) attributes
    fn parse_ansi_sgr_item(bytes: &[u8]) -> IResult<&[u8], AnsiItem> {
        let (bytes, code) = u8(bytes)?;
        let code = AnsiCode::from(code);
    
        let (bytes, color) = match code {
            AnsiCode::SetForegroundColor | AnsiCode::SetBackgroundColor => {
                let (bytes, _) = opt(tag(";"))(bytes)?;
                let (bytes, color) = color(bytes)?;
                (bytes, Some(color))
            }
            _ => (bytes, None),
        };
    
        let (bytes, _) = opt(tag(";"))(bytes)?;
        
        Ok((bytes, AnsiItem { code, color }))
    }

    fn color(bytes: &[u8]) -> IResult<&[u8], Color> {      
        let (bytes, type_id) = i64(bytes)?;
        // NOTE: This isn't opt because a color type must always be followed by a color
        let (bytes, _) = tag(";")(bytes)?;
    
        let (bytes, color_type) = match type_id {
            2 => Ok((bytes, ColorType::TrueColor)),
            5 => Ok((bytes, ColorType::EightBit)),
            _ => Err(nom::Err::Error(nom::error::Error::new(
                bytes,
                nom::error::ErrorKind::Alt,
            ))),
        };
    
        let (bytes, _) = opt(tag(";"))(bytes)?;
    
        match color_type {
            ColorType::TrueColor => {
                let (bytes, (r, _, g, _, b)) = tuple((u8, tag(";"), u8, tag(";"), u8))(bytes)?;
                Ok((bytes, Color::Rgb(r, g, b)))
            }
            ColorType::EightBit => {
                let (bytes, index) = u8(bytes)?;
                Ok((bytes, Color::Indexed(index)))
            }
        }
    }
    
    fn any_escape_sequence(s: &[u8]) -> IResult<&[u8], Option<&[u8]>> {
        // Attempt to consume most escape codes, including a single escape char.
        //
        // Most escape codes begin with ESC[ and are terminated by an alphabetic character,
        // but OSC codes begin with ESC] and are terminated by an ascii bell (\x07)
        // and a truncated/invalid code may just be a standalone ESC or not be terminated.
        //
        // We should try to consume as much of it as possible to match behavior of most terminals;
        // where we fail at that we should at least consume the escape char to avoid infinitely looping

        let (input, garbage) = preceded(
            char('\x1b'),
            opt(alt((
                delimited(char('['), take_till(is_alphabetic), opt(take(1u8))),
                delimited(char(']'), take_till(|c| c == b'\x07'), opt(take(1u8))),
            ))),
        )(s)?;
        
        Ok((input, garbage))
    }


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
        let t = text(b"\x1b[33msome arbitrary text\x1b[0m\nmore text")
            .unwrap()
            .1;
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
        let t = text(b"\x1b[33msome arbitrary text\x1b[m\nmore text")
            .unwrap()
            .1;
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
}

/// This enum stores most types of ansi escape sequences
///
/// You can turn an escape sequence to this enum variant using
/// `AnsiCode::from(code: u8)`
/// This doesn't support all of them but does support most of them.

#[derive(Debug, PartialEq, Clone)]
#[non_exhaustive]
pub enum AnsiCode {
    /// Reset the terminal
    Reset,
    /// Set font to bold
    Bold,
    /// Set font to faint
    Faint,
    /// Set font to italic
    Italic,
    /// Set font to underline
    Underline,
    /// Set cursor to slowblink
    SlowBlink,
    /// Set cursor to rapidblink
    RapidBlink,
    /// Invert the colors
    Reverse,
    /// Conceal text
    Conceal,
    /// Display crossed out text
    CrossedOut,
    /// Choose primary font
    PrimaryFont,
    /// Choose alternate font
    AlternateFont,
    /// Choose alternate fonts 1-9
    #[allow(dead_code)]
    AlternateFonts(u8), // = 11..19, // from 11 to 19
    /// Fraktur ? No clue
    Fraktur,
    /// Turn off bold
    BoldOff,
    /// Set text to normal
    Normal,
    /// Turn off Italic
    NotItalic,
    /// Turn off underline
    UnderlineOff,
    /// Turn off blinking
    BlinkOff,
    /// Don't invert colors
    InvertOff,
    /// Reveal text
    Reveal,
    /// Turn off Crossedout text
    CrossedOutOff,
    /// Set foreground color (4-bit)
    ForegroundColor(Color), //, 31..37//Issue 60553 https://github.com/rust-lang/rust/issues/60553
    /// Set foreground color (8-bit and 24-bit)
    SetForegroundColor,
    /// Default foreground color
    DefaultForegroundColor,
    /// Set background color (4-bit)
    BackgroundColor(Color), // 41..47
    /// Set background color (8-bit and 24-bit)
    SetBackgroundColor,
    /// Default background color
    DefaultBackgroundColor, // 49
    /// Other / non supported escape codes
    Code(Vec<u8>),
}

impl From<u8> for AnsiCode {
    fn from(code: u8) -> Self {
        match code {
            0 => AnsiCode::Reset,
            1 => AnsiCode::Bold,
            2 => AnsiCode::Faint,
            3 => AnsiCode::Italic,
            4 => AnsiCode::Underline,
            5 => AnsiCode::SlowBlink,
            6 => AnsiCode::RapidBlink,
            7 => AnsiCode::Reverse,
            8 => AnsiCode::Conceal,
            9 => AnsiCode::CrossedOut,
            10 => AnsiCode::PrimaryFont,
            11 => AnsiCode::AlternateFont,
            // AnsiCode::// AlternateFont = 11..19, // from 11 to 19
            20 => AnsiCode::Fraktur,
            21 => AnsiCode::BoldOff,
            22 => AnsiCode::Normal,
            23 => AnsiCode::NotItalic,
            24 => AnsiCode::UnderlineOff,
            25 => AnsiCode::BlinkOff,
            // 26, Proportional spacing, not known to be used in terminals
            27 => AnsiCode::InvertOff,
            28 => AnsiCode::Reveal,
            29 => AnsiCode::CrossedOutOff,
            30 => AnsiCode::ForegroundColor(Color::Black),
            31 => AnsiCode::ForegroundColor(Color::Red),
            32 => AnsiCode::ForegroundColor(Color::Green),
            33 => AnsiCode::ForegroundColor(Color::Yellow),
            34 => AnsiCode::ForegroundColor(Color::Blue),
            35 => AnsiCode::ForegroundColor(Color::Magenta),
            36 => AnsiCode::ForegroundColor(Color::Cyan),
            37 => AnsiCode::ForegroundColor(Color::Gray),
            38 => AnsiCode::SetForegroundColor,
            39 => AnsiCode::DefaultForegroundColor,
            40 => AnsiCode::BackgroundColor(Color::Black),
            41 => AnsiCode::BackgroundColor(Color::Red),
            42 => AnsiCode::BackgroundColor(Color::Green),
            43 => AnsiCode::BackgroundColor(Color::Yellow),
            44 => AnsiCode::BackgroundColor(Color::Blue),
            45 => AnsiCode::BackgroundColor(Color::Magenta),
            46 => AnsiCode::BackgroundColor(Color::Cyan),
            47 => AnsiCode::BackgroundColor(Color::Gray),
            48 => AnsiCode::SetBackgroundColor,
            49 => AnsiCode::DefaultBackgroundColor,
            90 => AnsiCode::ForegroundColor(Color::DarkGray),
            91 => AnsiCode::ForegroundColor(Color::LightRed),
            92 => AnsiCode::ForegroundColor(Color::LightGreen),
            93 => AnsiCode::ForegroundColor(Color::LightYellow),
            94 => AnsiCode::ForegroundColor(Color::LightBlue),
            95 => AnsiCode::ForegroundColor(Color::LightMagenta),
            96 => AnsiCode::ForegroundColor(Color::LightCyan),
            #[allow(clippy::match_same_arms)]
            97 => AnsiCode::ForegroundColor(Color::White),
            100 => AnsiCode::BackgroundColor(Color::DarkGray),
            101 => AnsiCode::BackgroundColor(Color::LightRed),
            102 => AnsiCode::BackgroundColor(Color::LightGreen),
            103 => AnsiCode::BackgroundColor(Color::LightYellow),
            104 => AnsiCode::BackgroundColor(Color::LightBlue),
            105 => AnsiCode::BackgroundColor(Color::LightMagenta),
            106 => AnsiCode::BackgroundColor(Color::LightCyan),
            107 => AnsiCode::ForegroundColor(Color::White),
            code => AnsiCode::Code(vec![code]),
        }
    }
}

#[derive(Debug, strum::Display)]
pub enum Error {
    /// Stack is empty (should never happen)
    #[strum(serialize = "Internal error: stack is empty")]
    NomError(String),

    /// Error parsing the input as utf-8
    #[strum(serialize = "{0}")]
    Utf8ErrorStd(std::string::FromUtf8Error),

    #[cfg(feature = "simd")]
    #[strum(serialize = "{0}")]
    Utf8Error(simdutf8::basic::Utf8Error),
}

impl From<simdutf8::basic::Utf8Error> for Error {
    fn from(source: simdutf8::basic::Utf8Error) -> Self {
        Error::Utf8Error(source)
    }
}

impl From<std::string::FromUtf8Error> for Error {
    fn from(source: std::string::FromUtf8Error) -> Self {
        Error::Utf8ErrorStd(source)
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Error {
    fn from(e: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Self::NomError(format!("{:?}", e))
    }
}
