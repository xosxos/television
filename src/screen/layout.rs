use std::fmt::Display;

use ratatui::layout;
use ratatui::layout::{Constraint, Direction, Rect};
use serde::Deserialize;

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

#[derive(Debug, Clone, Copy)]
pub struct ResultsLayout {
    pub input: Rect,
    pub results: Rect,
}

impl ResultsLayout {
    pub fn new(area: Rect, input_position: InputPosition) -> Self {
        let constraints = vec![Constraint::Min(3), Constraint::Length(3)];
        
        let chunks = layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints(match input_position {
                InputPosition::Top => constraints.into_iter().rev().collect(),
                InputPosition::Bottom => constraints,
            })
            .split(area);

        let (input, results) = match input_position {
            InputPosition::Bottom => (chunks[1], chunks[0]),
            InputPosition::Top => (chunks[0], chunks[1]),
        };
        
        Self { 
            input,
            results,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct HelpBarLayout {
    pub left: Rect,
    pub right: Rect,
}

impl HelpBarLayout {
    pub fn new(area: Rect) -> Self {
        let chunks = layout::Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                // metadata
                Constraint::Fill(1),
                // keymaps
                Constraint::Fill(1),
        ])
        .split(area);
                
        Self {
            left: chunks[0],
            right: chunks[1],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RemoteControlLayout {
    pub top: Rect,
    pub bottom: Rect,
}

impl RemoteControlLayout {
    pub fn new(area: Rect) -> Self {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Max(3),
            ])
            .split(rect);
        
        Self {
            top: chunks[0],
            bottom: chunks[1],
        }
    }


#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq)]
pub enum InputPosition {
    #[serde(rename = "top")]
    Top,
    #[serde(rename = "bottom")]
    #[default]
    Bottom,
}

impl Display for InputPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InputPosition::Top => write!(f, "top"),
            InputPosition::Bottom => write!(f, "bottom"),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum PreviewTitlePosition {
    #[serde(rename = "top")]
    #[default]
    Top,
    #[serde(rename = "bottom")]
    Bottom,
}

impl Display for PreviewTitlePosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreviewTitlePosition::Top => write!(f, "top"),
            PreviewTitlePosition::Bottom => write!(f, "bottom"),
        }
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
    #[allow(clippy::too_many_arguments)]
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
            input,
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

        let new_layout = |area, constraints, direction| {
            layout::Layout::default()
                .direction(direction)
                .constraints(constraints)
                .split(area)
        };
        
        // Helper windows : Help - Main Block - Logs
        if with_logs && with_help {
            let constraints = [Constraint::Max(9), Constraint::Fill(1), Constraint::Max(13)];
            let chunks = new_layout(area, constraints, Direction::Vertical); 
            
            let (top, middle, bottom) = (chunks[0], chunks[1], chunks[2]);
            
            help = Some(HelpBarLayout::new(top));
            main_section = middle;
            logs = Some(bottom);
        } else if with_help {
            let constraints = [Constraint::Max(9), Constraint::Fill(1)];
            let chunks = new_layout(area, constraints, Direction::Vertical); 
            
            let (top, middle) = (chunks[0], chunks[1]);
        
            help = Some(HelpBarLayout::new(top));
            main_section = middle;
        } else if with_logs {
            let constraints = [Constraint::Max(15), Constraint::Fill(1)];
            let chunks = new_layout(area, constraints, Direction::Vertical); 
            
            let (middle, bottom) = (chunks[0], chunks[1]);
            
            main_section = middle;
            logs = Some(bottom);
        } else {
            main_section = main_block;
        }

        // Main section: Results - Preview - Remote control
        if with_preview && with_remote {
            let constraints = [Constraint::Fill(1), Constraint::Fill(1), Constraint::Length(24)];
            let chunks = new_layout(main_section, constraints, Direction::Horizontal); 
            
            let (left, middle, right) = (chunks[0], chunks[1], chunks[2]);
            
            results = ResultsLayout::new(left);
            preview = Some(middle);
            remote = Some(RemoteControlLayout::new(right);
        } else if with_preview {
            let constraints = [Constraint::Fill(1), Constraint::Fill(1)];
            let chunks = new_layout(main_section, constraints, Direction::Horizontal);
            
            let (left, middle) = (chunks[0], chunks[1]);
            
            results_layout = ResultsLayout::new(left);
            preview = Some(middle);
        } else if with_remote {
            let constraints = [Constraint::Fill(1), Constraint::Length(24)];
            let chunks = new_layout(main_section, constraints, Direction::Horizontal);
            
            let (left, right) = (chunks[0], chunks[1]);
            
            results = ResultsLayout::new(left);
            remote = Some(RemoteControlLayout::new(right);
        } else {
            results = ResultsLayout::new(main_section)
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

