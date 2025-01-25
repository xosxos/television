#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::too_many_arguments)]

use std::str::FromStr;
use std::sync::{Arc, Mutex};

use color_eyre::eyre::Result;
use ratatui::layout::{Alignment, Rect};
use ratatui::prelude::{Color, Line, Modifier, Span, Style, Stylize, Text};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};
use ratatui::Frame;
use serde::Deserialize;
use tracing::debug;

use crate::model::channel::PreviewCommand;
use crate::model::previewer::rendered_cache::RenderedPreviewCache;
use crate::model::previewer::{
    Preview, PreviewContent, FILE_TOO_LARGE_MSG, PREVIEW_NOT_SUPPORTED_MSG,
};

use crate::colors::Colorscheme;
use crate::entry::Entry;
use crate::strings::{
    replace_non_printable, shrink_with_ellipsis, ReplaceNonPrintableConfig, EMPTY_STRING,
};

#[allow(dead_code)]
const FILL_CHAR_SLANTED: char = '╱';
const FILL_CHAR_EMPTY: char = ' ';

#[derive(Debug, Clone, Copy, Deserialize, Default, strum::Display)]
pub enum PreviewTitlePosition {
    #[default]
    #[serde(rename = "top")]
    #[strum(serialize = "top")]
    Top,
    #[serde(rename = "bottom")]
    #[strum(serialize = "bottom")]
    Bottom,
}

pub fn draw_preview(
    f: &mut Frame,
    rect: Rect,
    entry: &Entry,
    preview: &Arc<Preview>,
    rendered_preview_cache: &Arc<Mutex<RenderedPreviewCache<'static>>>,
    command: &PreviewCommand,
    preview_scroll: u16,
    use_nerd_font_icons: bool,
    colorscheme: &Colorscheme,
) -> Result<()> {
    let mut preview_title_spans = vec![Span::from(" ")];

    if preview.icon.is_some() && use_nerd_font_icons {
        let icon = preview.icon.as_ref().unwrap();
        preview_title_spans.push(Span::styled(
            {
                let mut icon_str = String::from(icon.icon);
                icon_str.push(' ');
                icon_str
            },
            Style::default().fg(Color::from_str(icon.color)?),
        ));
    }

    preview_title_spans.push(Span::styled(
        shrink_with_ellipsis(
            &replace_non_printable(
                preview.title.as_bytes(),
                &ReplaceNonPrintableConfig::default(),
            )
            .0,
            rect.width.saturating_sub(4) as usize,
        ),
        Style::default().fg(colorscheme.preview.title_fg).bold(),
    ));

    preview_title_spans.push(Span::from(" "));

    let preview_outer_block = Block::default()
        .title_top(
            Line::from(preview_title_spans)
                .alignment(Alignment::Center)
                .style(Style::default().fg(colorscheme.preview.title_fg)),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()))
        .padding(Padding::new(0, 1, 1, 0));

    let preview_block = Block::default().style(Style::default()).padding(Padding {
        top: 0,
        right: 1,
        bottom: 0,
        left: 1,
    });

    let inner = preview_outer_block.inner(rect);

    f.render_widget(preview_outer_block, rect);

    // Compute cache key
    let mut cache_key = entry.name.clone();

    if let Some(line_number) = entry.line_number {
        cache_key.push_str(&line_number.to_string());
    }

    cache_key.push_str(&command.command);

    // Check if the rendered preview content is already in the cache
    if let Some(preview_paragraph) = rendered_preview_cache.lock().unwrap().get(&cache_key) {
        let p = preview_paragraph.as_ref().clone();
        f.render_widget(p.scroll((preview_scroll, 0)), inner);
        return Ok(());
    }

    debug!(
        "Preview not {} found in rendered cache, key: {}",
        command.command, cache_key
    );

    println!("fuck fuuk");

    // let target_line = entry.line_number.map(|l| u16::try_from(l).unwrap_or(0));

    let rp = match preview.content.clone() {
        PreviewContent::AnsiText(text) => {
            let (text, _) = replace_non_printable(
                text.as_bytes(),
                &ReplaceNonPrintableConfig {
                    replace_line_feed: false,
                    replace_control_characters: false,
                    ..Default::default()
                },
            );

            let text = text.as_bytes();
            let text = crate::ansi::parser::text(text);

            Paragraph::new(text)
                .block(preview_block)
                .wrap(Wrap { trim: true })
                .scroll((preview_scroll, 0))
        }
        PreviewContent::Loading => {
            build_meta_preview_paragraph(inner, "Loading...", FILL_CHAR_EMPTY)
                .block(preview_block)
                .alignment(Alignment::Left)
                .style(Style::default().add_modifier(Modifier::ITALIC))
        }
        PreviewContent::NotSupported => {
            build_meta_preview_paragraph(inner, PREVIEW_NOT_SUPPORTED_MSG, FILL_CHAR_EMPTY)
                .block(preview_block)
                .alignment(Alignment::Left)
                .style(Style::default().add_modifier(Modifier::ITALIC))
        }
        PreviewContent::FileTooLarge => {
            build_meta_preview_paragraph(inner, FILE_TOO_LARGE_MSG, FILL_CHAR_EMPTY)
                .block(preview_block)
                .alignment(Alignment::Left)
                .style(Style::default().add_modifier(Modifier::ITALIC))
        }
        PreviewContent::Empty => Paragraph::new(Text::raw(EMPTY_STRING)),
    };

    if !preview.stale {
        debug!("preview not stale, save to rendered preview cache");

        rendered_preview_cache
            .lock()
            .unwrap()
            .insert(cache_key, &Arc::new(rp.clone()));
    }

    // f.render_widget(
    //     Arc::new(rp).as_ref().clone().scroll((preview_scroll, 0)),
    //     inner,
    // );

    Ok(())
}

pub fn build_meta_preview_paragraph(inner: Rect, message: &str, fill_char: char) -> Paragraph {
    let message_len = message.len();

    if message_len + 8 > inner.width as usize {
        return Paragraph::new(Text::from(EMPTY_STRING));
    }

    let fill_char_str = fill_char.to_string();
    let fill_line = fill_char_str.repeat(inner.width as usize);

    // Build the paragraph content with slanted lines and center the custom message
    let mut lines = Vec::new();

    // Calculate the vertical center
    let vertical_center = inner.height as usize / 2;
    let horizontal_padding = (inner.width as usize - message_len) / 2 - 4;

    // Fill the paragraph with slanted lines and insert the centered custom message
    for i in 0..inner.height {
        if i as usize == vertical_center {
            // Center the message horizontally in the middle line
            let line = format!(
                "{}  {}  {}",
                fill_char_str.repeat(horizontal_padding),
                message,
                fill_char_str.repeat(inner.width as usize - horizontal_padding - message_len)
            );

            lines.push(Line::from(line));
        } else if i as usize + 1 == vertical_center
            || (i as usize).saturating_sub(1) == vertical_center
        {
            let line = format!(
                "{}  {}  {}",
                fill_char_str.repeat(horizontal_padding),
                " ".repeat(message_len),
                fill_char_str.repeat(inner.width as usize - horizontal_padding - message_len)
            );

            lines.push(Line::from(line));
        } else {
            lines.push(Line::from(fill_line.clone()));
        }
    }

    // Create a paragraph with the generated content
    Paragraph::new(Text::from(lines))
}
