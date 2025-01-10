use ratatui::layout::{self, Constraint, Direction, Rect};

use crate::screen::help::HelpBarLayout;
use crate::screen::remote_control::RemoteControlLayout;
use crate::screen::results::{InputPosition, ResultsLayout};

// UI size
const UI_WIDTH_PERCENT: u16 = 95;
const UI_HEIGHT_PERCENT: u16 = 95;

pub struct Dimensions {
    pub x: u16,
    pub y: u16,
}

impl Dimensions {
    pub fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

impl From<u16> for Dimensions {
    fn from(x: u16) -> Self {
        Self::new(x, x)
    }
}

impl Default for Dimensions {
    fn default() -> Self {
        Self::new(UI_WIDTH_PERCENT, UI_HEIGHT_PERCENT)
    }
}

pub struct Layout {
    pub help_bar: Option<HelpBarLayout>,
    pub logs: Option<Rect>,
    pub results: ResultsLayout,
    pub preview_window: Option<Rect>,
    pub remote_control: Option<RemoteControlLayout>,
}

impl Layout {
    pub fn new(
        help_bar: Option<HelpBarLayout>,
        logs: Option<Rect>,
        results: ResultsLayout,
        preview_window: Option<Rect>,
        remote_control: Option<RemoteControlLayout>,
    ) -> Self {
        Self {
            help_bar,
            logs,
            results,
            preview_window,
            remote_control,
        }
    }
    
    #[rustfmt::skip]
    pub fn build(
        dimensions: &Dimensions,
        area: Rect,
        with_remote: bool,
        with_help: bool,
        with_logs: bool,
        with_preview: bool,
        input_position: InputPosition,
    ) -> Self {
        let area = centered_rect(dimensions.x, dimensions.y, area);
        
        let main_section: Rect;
        let results: ResultsLayout;
        
        let mut help = None;
        let mut logs = None;
        let mut preview = None;
        let mut remote = None;

        let show_help_logo = false;
        let show_remote_logo = false;

        let new_layout = |area, constraints, direction| {
            layout::Layout::default()
                .direction(direction)
                .constraints(constraints)
                .split(area)
        };
        
        //
        // Sections: Help, Main Section, Logs
        // 
        if with_logs && with_help {
            // Help - Main Section - Logs
            // --------------------- Help -----------  Main Section -------- Logs -------
            let constraints = [Constraint::Max(9), Constraint::Fill(1), Constraint::Max(13)].iter();
            let chunks = new_layout(area, constraints, Direction::Vertical); 
            
            let (top, middle, bottom) = (chunks[0], chunks[1], chunks[2]);
            
            help = Some(HelpBarLayout::new(top, show_help_logo));
            main_section = middle;
            logs = Some(bottom);

        } else if with_help {
            // --------------------- Help -----------  Main Section ---------
            let constraints = [Constraint::Max(9), Constraint::Fill(1)].iter();
            let chunks = new_layout(area, constraints, Direction::Vertical); 
            
            let (top, middle) = (chunks[0], chunks[1]);
        
            help = Some(HelpBarLayout::new(top, show_help_logo));
            main_section = middle;

        } else if with_logs {
            // ------------------- Main Section --------  Logs ---------
            let constraints = [Constraint::Max(15), Constraint::Fill(1)].iter();
            let chunks = new_layout(area, constraints, Direction::Vertical); 
            
            let (middle, bottom) = (chunks[0], chunks[1]);
            
            main_section = middle;
            logs = Some(bottom);

        } else {
            // Draw only the Main Section
            main_section = area;
        }

        //
        // Main Section: Results, Preview, Remote Control
        //
        if with_preview && with_remote {
            // --------------------- Results ----------  Preview ----------- Remote Control -----
            let constraints = [Constraint::Fill(1), Constraint::Fill(1), Constraint::Length(24)].iter();
            let chunks = new_layout(main_section, constraints, Direction::Horizontal); 
            
            let (left, middle, right) = (chunks[0], chunks[1], chunks[2]);
            
            results = ResultsLayout::new(left, input_position);
            preview = Some(middle);
            remote = Some(RemoteControlLayout::new(right, show_remote_logo));

        } else if with_preview {
            // --------------------- Results ---------------  Preview ---------
            let constraints = [Constraint::Fill(1), Constraint::Fill(1)].iter();
            let chunks = new_layout(main_section, constraints, Direction::Horizontal);
            
            let (left, middle) = (chunks[0], chunks[1]);
            
            results = ResultsLayout::new(left, input_position);
            preview = Some(middle);

        } else if with_remote {
            // --------------------- Results ------------  Remote Control ------
            let constraints = [Constraint::Fill(1), Constraint::Length(24)].iter();
            let chunks = new_layout(main_section, constraints, Direction::Horizontal);
            
            let (left, right) = (chunks[0], chunks[1]);
            
            results = ResultsLayout::new(left, input_position);
            remote = Some(RemoteControlLayout::new(right, show_remote_logo));

        } else {
            // Draw only the Results
            results = ResultsLayout::new(main_section, input_position);
        }

        Layout::new(
            help,
            logs,
            results,
            preview,
            remote,
        )
    }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    // Cut the given rectangle into three vertical pieces
    let popup_layout = layout::Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    // Then cut the middle vertical piece into three width-wise pieces
    layout::Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1] // Return the middle chunk
}

