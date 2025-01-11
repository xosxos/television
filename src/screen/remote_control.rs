use rustc_hash::FxHashMap as HashMap;

use color_eyre::eyre::Result;

use ratatui::layout::{self, Alignment, Constraint, Direction, Rect};
use ratatui::prelude::Style;
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, ListDirection, ListState, Padding, Paragraph};
use ratatui::Frame;

use crate::entry::Entry;

use crate::screen::colors::Colorscheme;
use crate::screen::results::build_results_list;
use crate::television::Mode;
use crate::utils::input::Input;

#[derive(Debug, Clone, Copy)]
pub struct RemoteControlLayout {
    pub top: Rect,
    pub bottom: Rect,
}

impl RemoteControlLayout {
    pub fn new(area: Rect, _show_logo: bool) -> Self {
        let chunks = layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Max(3)])
            .split(area);

        Self {
            top: chunks[0],
            bottom: chunks[1],
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw_remote_control(
    f: &mut Frame,
    layout: RemoteControlLayout,
    entries: &[Entry],
    use_nerd_font_icons: bool,
    picker_state: &mut ListState,
    input_state: &mut Input,
    icon_color_cache: &mut HashMap<String, Color>,
    _mode: &Mode,
    colorscheme: &Colorscheme,
) -> Result<()> {
    draw_rc_channels(
        f,
        layout.top,
        entries,
        use_nerd_font_icons,
        picker_state,
        icon_color_cache,
        colorscheme,
    );

    draw_rc_input(f, layout.bottom, input_state, colorscheme)?;

    Ok(())
}

fn draw_rc_channels(
    f: &mut Frame,
    area: Rect,
    entries: &[Entry],
    use_nerd_font_icons: bool,
    picker_state: &mut ListState,
    icon_color_cache: &mut HashMap<String, Color>,
    colorscheme: &Colorscheme,
) {
    let rc_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()))
        .padding(Padding::right(1));

    let channel_list = build_results_list(
        rc_block,
        entries,
        None,
        ListDirection::TopToBottom,
        use_nerd_font_icons,
        icon_color_cache,
        &colorscheme.results,
    );

    f.render_stateful_widget(channel_list, area, picker_state);
}

fn draw_rc_input(
    f: &mut Frame,
    area: Rect,
    input: &mut Input,
    colorscheme: &Colorscheme,
) -> Result<()> {
    let input_block = Block::default()
        .title_top(Line::from("Remote Control").alignment(Alignment::Center))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()));

    let input_block_inner = input_block.inner(area);

    let split = |area, constraints, direction| {
        layout::Layout::default()
            .direction(direction)
            .constraints(constraints)
            .split(area)
    };

    // Split the block into Symbol and Paragraph Area
    let constraints = [Constraint::Length(2), Constraint::Fill(1)].iter();
    let chunks = split(input_block_inner, constraints, Direction::Horizontal);
    let (symbol_area, paragraph_area) = (chunks[0], chunks[1]);

    // Define the symbol
    let arrow = Paragraph::new(Span::styled(
        "> ",
        Style::default().fg(colorscheme.input.input_fg).bold(),
    ))
    .block(Block::default());

    // keep 2 for borders and 1 for cursor
    let width = (paragraph_area.width.max(3) - 3) as usize;
    let scroll = input.visual_scroll(width);

    // Define the Input paragraph
    let input_paragraph = Paragraph::new(input.value())
        .scroll((0, u16::try_from(scroll)?))
        .block(Block::default())
        .style(
            Style::default()
                .fg(colorscheme.input.input_fg)
                .bold()
                .italic(),
        )
        .alignment(Alignment::Left);

    // Render
    f.render_widget(input_block, area);
    f.render_widget(arrow, symbol_area);
    f.render_widget(input_paragraph, paragraph_area);

    // Make the cursor visible and ask tui-rs to put it at the
    // specified coordinates after rendering
    f.set_cursor_position((
        // Put cursor past the end of the input text
        paragraph_area.x + u16::try_from(input.visual_cursor().max(scroll) - scroll)?,
        // Move one line down, from the border to the input line
        paragraph_area.y,
    ));

    Ok(())
}
