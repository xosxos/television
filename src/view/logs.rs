use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders, ListState, Padding},
    Frame,
};

use crate::logger::LogWidget;

use crate::colors::Colorscheme;

pub fn draw_logs(frame: &mut Frame, area: Rect, colorscheme: &Colorscheme, scroll: &mut ListState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()))
        .padding(Padding::horizontal(1));

    let list = LogWidget::default()
        .draw(frame.area().width as usize)
        .block(block);

    frame.render_stateful_widget(list, area, scroll);
}
