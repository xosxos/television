use ratatui::layout::{self, Constraint, Direction, Rect};
use ratatui::prelude::Style;
use ratatui::style::Color;
use ratatui::widgets::{Block, BorderType, Borders, Padding, Table};
use ratatui::Frame;
use ratatui::{
    text::{Line, Span},
    widgets::{Cell, Row},
};

use crate::channel::Channel;
use crate::config::KeyBindings;
use crate::screen::colors::{Colorscheme, GeneralColorscheme};
use crate::television::Mode;
use crate::utils::AppMetadata;

#[derive(Debug, Clone, Copy)]
pub struct HelpLayout {
    pub left: Rect,
    pub right: Rect,
}

impl HelpLayout {
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

pub fn draw_help(
    f: &mut Frame,
    help: &HelpLayout,
    channel: &Channel,
    keybindings: &KeyBindings,
    mode: Mode,
    app_metadata: &AppMetadata,
    colorscheme: &Colorscheme,
) {
    draw_metadata_block(f, help.left, mode, channel, app_metadata, colorscheme);

    let keymap_table = build_keybindings_table(keybindings, colorscheme);

    draw_keymaps_block(f, help.right, keymap_table, &colorscheme.general);
}

fn draw_metadata_block(
    f: &mut Frame,
    area: Rect,
    _mode: Mode,
    channel: &Channel,
    _app_metadata: &AppMetadata,
    colorscheme: &Colorscheme,
) {
    let metadata_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()));

    let metadata_table = build_metadata_table(channel, colorscheme).block(metadata_block);

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

pub fn build_metadata_table<'a>(channel: &'a Channel, colorscheme: &'a Colorscheme) -> Table<'a> {
    let build_row = |name: &str, value: String| {
        Row::new([
            Cell::from(Span::styled(
                name.to_string(),
                Style::default().fg(colorscheme.help.metadata_field_name_fg),
            )),
            Cell::from(Span::styled(
                value,
                Style::default().fg(colorscheme.help.metadata_field_value_fg),
            )),
        ])
    };

    let build_row_selected = |name: &str, value: String| {
        Row::new([
            Cell::from(Span::styled(
                name.to_string(),
                Style::default().fg(colorscheme.preview.content_fg),
            )),
            Cell::from(Span::styled(
                value,
                Style::default().fg(colorscheme.preview.content_fg),
            )),
        ])
    };

    let mut rows = vec![];

    for (i, cmd) in channel.preview_command.iter().enumerate() {
        let preview_cmd = match cmd == channel.current_preview_command() {
            true => build_row_selected(&format!("preview {}: ", i + 1), cmd.command.to_string()),
            false => build_row(&format!("preview {}: ", i + 1), cmd.command.to_string()),
        };

        rows.push(preview_cmd);
    }

    for (i, cmd) in channel.run_command.iter().enumerate() {
        let preview_cmd = match cmd == channel.current_run_command() {
            true => build_row_selected(&format!("run {}: ", i + 1), cmd.to_string()),
            false => build_row(&format!("run {}: ", i + 1), cmd.to_string()),
        };

        rows.push(preview_cmd);
    }

    // ---------------------- Col 1 ------------- Col 2 ------
    let widths = vec![Constraint::Fill(1), Constraint::Fill(2)];

    Table::new(rows, widths)
}

pub fn build_keybindings_table<'a>(
    keybindings: &'a KeyBindings,
    colorscheme: &'a Colorscheme,
) -> Table<'a> {
    let build_row = |name, bindings: &[String]| {
        Row::new(build_cells_for_group(
            name,
            bindings,
            colorscheme.help.metadata_field_name_fg,
            colorscheme.mode.channel,
        ))
    };

    let results = build_row(
        "Results nav",
        &[
            keybindings.select_next_entry.to_string(),
            keybindings.select_prev_entry.to_string(),
        ],
    );

    let preview = build_row(
        "Preview nav",
        &[
            keybindings.scroll_preview_half_page_down.to_string(),
            keybindings.scroll_preview_half_page_up.to_string(),
        ],
    );

    let select_entry = build_row("Select entry", &[keybindings.confirm_selection.to_string()]);

    let send_to_channel = build_row(
        "Send results to",
        &[keybindings.toggle_send_to_channel.to_string()],
    );

    let switch_channels = build_row(
        "Toggle Remote control",
        &[keybindings.toggle_remote_control.to_string()],
    );

    let copy_entry = build_row("Copy", &[keybindings.copy_entry_to_clipboard.to_string()]);

    // ---------------------------- Col 1 ------------- Col 2 ------
    let column_widths = vec![Constraint::Fill(1), Constraint::Fill(2)];

    Table::new(
        vec![
            results,
            preview,
            select_entry,
            copy_entry,
            send_to_channel,
            switch_channels,
        ],
        column_widths,
    )
}

fn build_cells_for_group<'a>(
    group_name: &str,
    keys: &[String],
    key_color: Color,
    value_color: Color,
) -> Vec<Cell<'a>> {
    // Group name
    let group_name = group_name.to_owned();
    let group_name = Cell::from(Span::styled(
        group_name + ": ",
        Style::default().fg(key_color),
    ));

    // Keys
    let first_key = keys[0].clone();
    let spans = vec![Span::styled(first_key, Style::default().fg(value_color))];

    let spans = keys.iter().skip(1).fold(spans, |mut acc, key| {
        let key = key.to_owned();

        acc.push(Span::raw(" / "));
        acc.push(Span::styled(key, Style::default().fg(value_color)));
        acc
    });

    let spans = Cell::from(Line::from(spans));

    vec![group_name, spans]
}
