use ratatui::{
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders, Padding},
    Frame,
};

use crate::logger_widget::LogWidget;

use super::colors::Colorscheme;

pub fn draw_logs_bar(frame: &mut Frame, area: Rect, colorscheme: &Colorscheme) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colorscheme.general.border_fg))
        .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()))
        .padding(Padding::horizontal(1));

    let paragraph = LogWidget::default()
        .draw(frame.area().width as usize)
        .block(block);

    frame.render_widget(paragraph, area);
}
