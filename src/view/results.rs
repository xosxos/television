use rustc_hash::{FxHashMap as HashMap, FxHashSet as HashSet};
use serde::Deserialize;
use std::str::FromStr;

use color_eyre::eyre::Result;

use ratatui::layout::{self, Alignment, Constraint, Direction, Layout as RatatuiLayout, Rect};
use ratatui::prelude::{Color, Line, Span, Style};
use ratatui::style::Stylize;
use ratatui::widgets::Paragraph;
use ratatui::widgets::{Block, BorderType, Borders, List, ListDirection, ListState, Padding};
use ratatui::Frame;

use crate::model::entry::Entry;
use crate::model::input::Input;

use crate::colors::{Colorscheme, ResultsColorscheme};
use crate::strings::{make_matched_string_printable, next_char_boundary, slice_at_char_boundaries};
use crate::{
    utils::AppMetadata,
    view::spinner::{Spinner, SpinnerState},
};

const POINTER_SYMBOL: &str = "> ";
const SELECTED_SYMBOL: &str = "● ";
const DESLECTED_SYMBOL: &str = "  ";

#[derive(Debug, Clone, Copy)]
pub struct ResultsLayout {
    pub input: Rect,
    pub results: Rect,
}

impl ResultsLayout {
    pub fn new(area: Rect, input_position: InputPosition) -> Self {
        //-----------------------  input   ------------ results -------
        let constraints_top = [Constraint::Length(3), Constraint::Min(3)];

        //-----------------------  results ------------ input -------
        let constraints_btm = [Constraint::Min(3), Constraint::Length(3)];

        let chunks = layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints(match input_position {
                InputPosition::Top => constraints_top,
                InputPosition::Bottom => constraints_btm,
            })
            .split(area);

        let (input, results) = match input_position {
            InputPosition::Bottom => (chunks[1], chunks[0]),
            InputPosition::Top => (chunks[0], chunks[1]),
        };

        Self { input, results }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, strum::Display)]
pub enum InputPosition {
    #[serde(rename = "top")]
    #[strum(serialize = "top")]
    Top,
    #[default]
    #[serde(rename = "bottom")]
    #[strum(serialize = "bottom")]
    Bottom,
}

pub fn build_results_list<'a, 'b>(
    results_block: Block<'b>,
    entries: &'a [Entry],
    selected_entries: Option<&HashSet<Entry>>,
    list_direction: ListDirection,
    use_icons: bool,
    icon_color_cache: &mut HashMap<String, Color>,
    colorscheme: &ResultsColorscheme,
) -> List<'a>
where
    'b: 'a,
{
    List::new(entries.iter().map(|entry| {
        let mut spans = Vec::new();

        // optional selection symbol
        if let Some(selected_entries) = selected_entries {
            if !selected_entries.is_empty() {
                spans.push(if selected_entries.contains(entry) {
                    Span::styled(
                        SELECTED_SYMBOL,
                        Style::default().fg(colorscheme.result_selected_fg),
                    )
                } else {
                    Span::from(DESLECTED_SYMBOL)
                });
            }
        }

        // optional icon
        if use_icons {
            if let Some(icon) = entry.icon.as_ref() {
                if let Some(icon_color) = icon_color_cache.get(icon.color) {
                    spans.push(Span::styled(
                        icon.to_string(),
                        Style::default().fg(*icon_color),
                    ));
                } else {
                    let icon_color = Color::from_str(icon.color).unwrap();
                    icon_color_cache.insert(icon.color.to_string(), icon_color);
                    spans.push(Span::styled(
                        icon.to_string(),
                        Style::default().fg(icon_color),
                    ));
                }

                spans.push(Span::raw(" "));
            }
        }

        // entry name
        let (entry_name, name_match_ranges) =
            make_matched_string_printable(&entry.name, entry.name_match_ranges.as_deref());

        let mut last_match_end = 0;

        for (start, end) in name_match_ranges
            .iter()
            .map(|(s, e)| (*s as usize, *e as usize))
        {
            // from the end of the last match to the start of the current one
            spans.push(Span::styled(
                slice_at_char_boundaries(&entry_name, last_match_end, start).to_string(),
                Style::default().fg(colorscheme.result_name_fg),
            ));

            // the current match
            spans.push(Span::styled(
                slice_at_char_boundaries(&entry_name, start, end).to_string(),
                Style::default().fg(colorscheme.match_foreground_color),
            ));

            last_match_end = end;
        }

        // we need to push a span for the remainder of the entry name
        // but only if there's something left
        let next_boundary = next_char_boundary(&entry_name, last_match_end);

        if next_boundary < entry_name.len() {
            let remainder = entry_name[next_boundary..].to_string();
            spans.push(Span::styled(
                remainder,
                Style::default().fg(colorscheme.result_name_fg),
            ));
        }

        // optional line number
        if let Some(line_number) = entry.line_number {
            spans.push(Span::styled(
                format!(":{line_number}"),
                Style::default().fg(colorscheme.result_line_number_fg),
            ));
        }

        Line::from(spans)
    }))
    .direction(list_direction)
    .highlight_style(Style::default().bg(colorscheme.result_selected_bg).bold())
    .highlight_symbol(POINTER_SYMBOL)
    .block(results_block)
}

pub fn draw_results(
    f: &mut Frame,
    rect: Rect,
    entries: &[Entry],
    selected_entries: &HashSet<Entry>,
    relative_picker_state: &mut ListState,
    input_bar_position: InputPosition,
    use_nerd_font_icons: bool,
    icon_color_cache: &mut HashMap<String, Color>,
    colorscheme: &Colorscheme,
    help_keybinding: &str,
    preview_keybinding: &str,
    channel_name: &str,
) -> Result<()> {
    let results_block = Block::default()
        .title_top(Line::from(format!(" {channel_name} ")).alignment(Alignment::Center))
        .title_bottom(
            Line::from(format!(
                " help: <{help_keybinding}>  preview: <{preview_keybinding}> "
            ))
            .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()))
        .padding(Padding::right(1));

    let results_list = build_results_list(
        results_block,
        entries,
        Some(selected_entries),
        match input_bar_position {
            InputPosition::Bottom => ListDirection::BottomToTop,
            InputPosition::Top => ListDirection::TopToBottom,
        },
        use_nerd_font_icons,
        icon_color_cache,
        &colorscheme.results,
    );

    f.render_stateful_widget(results_list, rect, relative_picker_state);

    Ok(())
}

pub fn draw_input(
    f: &mut Frame,
    rect: Rect,
    results_count: u32,
    total_count: u32,
    input_state: &mut Input,
    results_picker_state: &mut ListState,
    matcher_running: bool,
    spinner: &Spinner,
    spinner_state: &mut SpinnerState,
    colorscheme: &Colorscheme,
    app_metadata: &AppMetadata,
) -> Result<()> {
    let input_block = Block::default()
        // .title_top(Line::from("foo").alignment(Alignment::Center))
        .title_top(
            Line::from(format!(" {} ", app_metadata.current_directory))
                .alignment(Alignment::Center),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()));

    let input_block_inner = input_block.inner(rect);
    if input_block_inner.area() == 0 {
        return Ok(());
    }

    f.render_widget(input_block, rect);

    // split input block into 4 parts: prompt symbol, input, result count, spinner
    let inner_input_chunks = RatatuiLayout::default()
        .direction(Direction::Horizontal)
        .constraints([
            // prompt symbol
            Constraint::Length(2),
            // input field
            Constraint::Fill(1),
            // result count
            Constraint::Length(3 * (u16::try_from((total_count.max(1)).ilog10()).unwrap() + 1) + 3),
            // spinner
            Constraint::Length(1),
        ])
        .split(input_block_inner);

    let arrow_block = Block::default();
    let arrow = Paragraph::new(Span::styled(
        "> ",
        Style::default().fg(colorscheme.input.input_fg).bold(),
    ))
    .block(arrow_block);
    f.render_widget(arrow, inner_input_chunks[0]);

    let interactive_input_block = Block::default();
    // keep 2 for borders and 1 for cursor
    let width = inner_input_chunks[1].width.max(3) - 3;
    let scroll = input_state.visual_scroll(width as usize);
    let input = Paragraph::new(input_state.value())
        .scroll((0, u16::try_from(scroll)?))
        .block(interactive_input_block)
        .style(
            Style::default()
                .fg(colorscheme.input.input_fg)
                .bold()
                .italic(),
        )
        .alignment(Alignment::Left);
    f.render_widget(input, inner_input_chunks[1]);

    if matcher_running {
        f.render_stateful_widget(spinner, inner_input_chunks[3], spinner_state);
    }

    let result_count_block = Block::default();
    let result_count_paragraph = Paragraph::new(Span::styled(
        format!(
            " {} / {} ",
            match results_count == 0 {
                true => 0,
                false => results_picker_state.selected().unwrap_or(0) + 1,
            },
            results_count,
        ),
        Style::default()
            .fg(colorscheme.input.results_count_fg)
            .italic(),
    ))
    .block(result_count_block)
    .alignment(Alignment::Right);
    f.render_widget(result_count_paragraph, inner_input_chunks[2]);

    // Make the cursor visible and ask tui-rs to put it at the
    // specified coordinates after rendering
    f.set_cursor_position((
        // Put cursor past the end of the input text
        inner_input_chunks[1].x + u16::try_from(input_state.visual_cursor().max(scroll) - scroll)?,
        // Move one line down, from the border to the input line
        inner_input_chunks[1].y,
    ));
    Ok(())
}
