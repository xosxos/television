use ratatui::layout::{self, Constraint, Direction, Rect};
use ratatui::prelude::Style;
use ratatui::widgets::{Block, BorderType, Borders, Padding, Table};
use ratatui::Frame;
use ratatui::{
    text::Span,
    widgets::{Cell, Row},
};

use crate::channel::Channel;
use crate::screen::colors::{Colorscheme, GeneralColorscheme};
use crate::television::Mode;
use crate::utils::AppMetadata;

#[derive(Debug, Clone, Copy)]
pub struct HelpBarLayout {
    pub left: Rect,
    pub right: Rect,
}

impl HelpBarLayout {
    pub fn new(area: Rect, _show_logo: bool) -> Self {
        //-------------------  metadata ------------ keymaps -------
        let constraints = [Constraint::Fill(1), Constraint::Fill(1)];

        let chunks = layout::Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        Self {
            // metadata
            left: chunks[0],
            // keymaps
            right: chunks[1],
        }
    }
}

fn draw_metadata_block(
    f: &mut Frame,
    area: Rect,
    mode: Mode,
    current_channel: &Channel,
    app_metadata: &AppMetadata,
    colorscheme: &Colorscheme,
) {
    let metadata_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()));

    let metadata_table = build_metadata_table(mode, current_channel, app_metadata, colorscheme)
        .block(metadata_block);

    f.render_widget(metadata_table, area);
}

fn draw_keymaps_block(
    f: &mut Frame,
    area: Rect,
    keymap_table: Table,
    colorscheme: &GeneralColorscheme,
) {
    let keymaps_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.border_fg))
        .style(Style::default().bg(colorscheme.background.unwrap_or_default()))
        .padding(Padding::horizontal(1));

    let table = keymap_table.block(keymaps_block);

    f.render_widget(table, area);
}

pub fn draw_help_bar(
    f: &mut Frame,
    help_bar: &HelpBarLayout,
    current_channel: &Channel,
    keymap_table: Table,
    mode: Mode,
    app_metadata: &AppMetadata,
    colorscheme: &Colorscheme,
) {
    draw_metadata_block(
        f,
        help_bar.left,
        mode,
        current_channel,
        app_metadata,
        colorscheme,
    );

    draw_keymaps_block(f, help_bar.right, keymap_table, &colorscheme.general);
}

pub fn build_metadata_table<'a>(
    mode: Mode,
    current_channel: &Channel,
    app_metadata: &'a AppMetadata,
    colorscheme: &'a Colorscheme,
) -> Table<'a> {
    let version_row = Row::new(vec![
        Cell::from(Span::styled(
            "version: ",
            Style::default().fg(colorscheme.help.metadata_field_name_fg),
        )),
        Cell::from(Span::styled(
            &app_metadata.version,
            Style::default().fg(colorscheme.help.metadata_field_value_fg),
        )),
    ]);

    let current_dir_row = Row::new(vec![
        Cell::from(Span::styled(
            "current directory: ",
            Style::default().fg(colorscheme.help.metadata_field_name_fg),
        )),
        Cell::from(Span::styled(
            std::env::current_dir()
                .expect("Could not get current directory")
                .display()
                .to_string(),
            Style::default().fg(colorscheme.help.metadata_field_value_fg),
        )),
    ]);

    let current_channel_row = Row::new(vec![
        Cell::from(Span::styled(
            "current channel: ",
            Style::default().fg(colorscheme.help.metadata_field_name_fg),
        )),
        Cell::from(Span::styled(
            current_channel.name.to_string(),
            Style::default().fg(colorscheme.help.metadata_field_value_fg),
        )),
    ]);

    let current_mode_row = Row::new(vec![
        Cell::from(Span::styled(
            "current mode: ",
            Style::default().fg(colorscheme.help.metadata_field_name_fg),
        )),
        Cell::from(Span::styled(
            mode.to_string(),
            Style::default().fg(mode.color(&colorscheme.mode)),
        )),
    ]);

    let widths = vec![Constraint::Fill(1), Constraint::Fill(2)];

    Table::new(
        vec![
            version_row,
            current_dir_row,
            current_channel_row,
            current_mode_row,
        ],
        widths,
    )
}
