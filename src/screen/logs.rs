use ratatui::{
    style::Style,
    widgets::{Block, BorderType, Borders, Padding},
    Frame,
};
use tui_logger::LogWidget;

use super::{colors::Colorscheme, layout::LogsLayout};

pub fn draw_logs_bar(frame: &mut Frame, layout: &Option<LogsLayout>, colorscheme: &Colorscheme) {
    if let Some(help_bar) = layout {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(colorscheme.general.border_fg))
            .style(Style::default().bg(colorscheme.general.background.unwrap_or_default()))
            .padding(Padding::horizontal(1));

        let paragraph = LogWidget::default()
            .draw(frame.area().width as usize)
            .block(block);

        frame.render_widget(paragraph, help_bar.area);
    }
}
