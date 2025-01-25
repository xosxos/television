#![allow(unused_imports)]
//! This module provides a way to parse ansi escape codes and convert them to ratatui objects.
//!
//! This code is a modified version of [ansi_to_tui](https://github.com/ratatui/ansi-to-tui).

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
use smallvec::{smallvec, SmallVec, ToSmallVec};
use std::str::FromStr;

#[cfg(test)]
#[path = "../unit_tests/test_ansi.rs"]
mod tests;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ColorType {
    /// Eight Bit color
    EightBit,
    /// 24-bit color or true color
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

pub fn ansi_to_text(mut s: &[u8]) -> Text<'static> {
    let mut lines = Vec::new();
    let mut last_style = Style::new();

    while let Ok((remaining, (line, style))) = line(last_style, s) {
        lines.push(line);
        last_style = style;
        s = remaining;
        if s.is_empty() {
            break;
        }
    }

    Text::from(lines)
}

pub fn line(style: Style, s: &[u8]) -> IResult<&[u8], (Line<'static>, Style)> {
    // let style_: Style = Default::default();
    // consume s until a line ending is found
    let (s, mut text) = not_line_ending(s)?;
    // discard the line ending
    let (s, _) = opt(alt((tag("\r\n"), tag("\n"))))(s)?;
    let mut spans = Vec::new();
    // carry over the style from the previous line (passed in as an argument)
    let mut last_style = style;
    // parse spans from the given text
    while let Ok((remaining, span)) = span(last_style, text) {
        // Since reset now tracks separately we can skip the reset check
        last_style = last_style.patch(span.style);

        if !span.content.is_empty() {
            spans.push(span);
        }
        text = remaining;
        if text.is_empty() {
            break;
        }
    }

    // NOTE: what is last_style here
    Ok((s, (Line::from(spans), last_style)))
}

pub fn span(last: Style, s: &[u8]) -> IResult<&[u8], Span<'static>, nom::error::Error<&[u8]>> {
    let mut last_style = last;
    // optionally consume a style
    let (s, maybe_style) = opt(style(last_style))(s)?;

    // consume until an escape sequence is found
    #[cfg(feature = "simd")]
    let (s, text) = map_res(take_while(|c| c != b'\x1b'), |t| {
        simdutf8::basic::from_utf8(t)
    })(s)?;

    #[cfg(not(feature = "simd"))]
    let (s, text) = map_res(take_while(|c| c != b'\x1b'), |t| std::str::from_utf8(t))(s)?;

    // if a style was found, patch the last style with it
    if let Some(st) = maybe_style.flatten() {
        last_style = last_style.patch(st);
    }

    Ok((s, Span::styled(text.to_owned(), last_style)))
}

pub fn style(
    style: Style,
) -> impl Fn(&[u8]) -> IResult<&[u8], Option<Style>, nom::error::Error<&[u8]>> {
    move |s: &[u8]| -> IResult<&[u8], Option<Style>> {
        let (s, ansi_items) = opt(move |s| {
            delimited(
                tag("\x1b["),
                fold_many0(
                    parse_ansi_sgr_item,
                    smallvec::SmallVec::new,
                    |mut items, item| {
                        items.push(item);
                        items
                    },
                ),
                char('m'),
            )(s)
        })(s)?;

        let (s, ansi_items) = match ansi_items {
            Some(items) => {
                // This would correspond to an implicit reset code (\x1b[m)
                let items = match items.is_empty() {
                    true => smallvec![AnsiItem { code: AnsiCode::Reset, color: None }; 2],
                    false => items,
                };

                (s, Some(items))
            }
            None => (any_escape_sequence(s)?.0, None),
        };

        Ok((
            s,
            ansi_items.map(|items| Style::from(AnsiStates { items, style })),
        ))
    }
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

pub fn color(bytes: &[u8]) -> IResult<&[u8], Color> {
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
    }?;

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
    // 26 ?
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
            // 26 ?
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
