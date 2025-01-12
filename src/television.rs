use rustc_hash::{FxBuildHasher, FxHashMap as HashMap, FxHashSet as HashSet};
use std::sync::{Arc, Mutex};

use color_eyre::Result;
use copypasta::{ClipboardContext, ClipboardProvider};
use ratatui::{layout::Rect, style::Color, Frame};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{info, warn};

use crate::action::Action;
use crate::channel::{Channel, ChannelConfigs};
use crate::config::{Config, Theme};
use crate::entry::{Entry, ENTRY_PLACEHOLDER};
use crate::screen::logs::draw_logs_bar;
use crate::utils::input::InputRequest;
use crate::utils::strings::EMPTY_STRING;
use crate::utils::AppMetadata;
// use crate::input::convert_action_to_input_request;
use crate::picker::Picker;
use crate::previewer::Previewer;

use crate::remote_control::RemoteControl;
use crate::screen::cache::RenderedPreviewCache;
use crate::screen::colors::Colorscheme;
use crate::screen::help::draw_help_bar;
use crate::screen::layout::{Dimensions, Layout};
use crate::screen::preview::draw_preview_content_block;
use crate::screen::remote_control::draw_remote_control;
use crate::screen::results::{draw_input_box, draw_results_list, InputPosition};
use crate::screen::spinner::{Spinner, SpinnerState};

use serde::{Deserialize, Serialize};

use crate::screen::colors::ModeColorscheme;

#[derive(PartialEq, Copy, Clone, Hash, Eq, Debug, Serialize, Deserialize, strum::Display)]
pub enum Mode {
    #[serde(rename = "channel")]
    #[strum(serialize = "Channel")]
    Channel,
    #[serde(rename = "remote_control")]
    #[strum(serialize = "Remote Control")]
    RemoteControl,
    #[serde(rename = "send_to_channel")]
    #[strum(serialize = "Send to Channel")]
    SendToChannel,
}

impl Mode {
    pub fn color(&self, colorscheme: &ModeColorscheme) -> Color {
        match &self {
            Mode::Channel => colorscheme.channel,
            Mode::RemoteControl => colorscheme.remote_control,
            Mode::SendToChannel => colorscheme.send_to_channel,
        }
    }
}

pub trait OnAir: Send {
    /// Find entries that match the given pattern.
    ///
    /// This method does not return anything and instead typically stores the
    /// results internally for later retrieval allowing to perform the search
    /// in the background while incrementally polling the results with
    /// `results`.
    fn find(&mut self, pattern: &str);

    /// Get the results of the search (that are currently available).
    fn results(&mut self, num_entries: u32, offset: u32) -> Vec<Entry>;

    /// Get a specific result by its index.
    fn get_result(&self, index: u32) -> Option<Entry>;

    /// Get the currently selected entries.
    fn selected_entries(&self) -> &HashSet<Entry>;

    /// Toggles selection for the entry under the cursor.
    fn toggle_selection(&mut self, entry: &Entry);

    /// Get the number of results currently available.
    fn result_count(&self) -> u32;

    /// Get the total number of entries currently available.
    fn total_count(&self) -> u32;

    /// Check if the channel is currently running.
    fn running(&self) -> bool;

    /// Turn off
    fn shutdown(&self);
}

pub struct Television {
    pub action_tx: Option<UnboundedSender<Action>>,
    pub config: Config,
    pub(crate) channel: Channel,
    pub channels: ChannelConfigs,
    pub(crate) remote_control: RemoteControl,
    pub mode: Mode,
    pub current_pattern: String,
    pub(crate) results_picker: Picker,
    pub(crate) rc_picker: Picker,
    results_area_height: u32,
    pub previewer: Previewer,
    pub preview_scroll: Option<u16>,
    pub preview_pane_height: u16,
    current_preview_total_lines: u16,
    pub icon_color_cache: HashMap<String, Color>,
    pub rendered_preview_cache: Arc<Mutex<RenderedPreviewCache<'static>>>,
    pub(crate) spinner: Spinner,
    pub(crate) spinner_state: SpinnerState,
    pub app_metadata: AppMetadata,
    pub colorscheme: Colorscheme,
}

impl Television {
    #[must_use]
    pub fn new(
        mut channel: Channel,
        config: Config,
        input: Option<String>,
        channels: ChannelConfigs,
    ) -> Self {
        let mut results_picker = Picker::new(input.clone());

        if config.ui.input_bar_position == InputPosition::Bottom {
            results_picker = results_picker.inverted();
        }

        let app_metadata = AppMetadata::new(
            env!("CARGO_PKG_VERSION").to_string(),
            std::env::current_dir()
                .expect("Could not get current directory")
                .to_string_lossy()
                .to_string(),
        );

        let colorscheme = (&Theme::from_name(&config.ui.theme)).into();

        channel.find(&input.unwrap_or(EMPTY_STRING.to_string()));
        let spinner = Spinner::default();
        Self {
            action_tx: None,
            config,
            previewer: Previewer::new(),
            channel,
            remote_control: RemoteControl::new(channels.clone()),
            channels,
            mode: Mode::Channel,
            current_pattern: EMPTY_STRING.to_string(),
            results_picker,
            rc_picker: Picker::default(),
            results_area_height: 0,
            preview_scroll: None,
            preview_pane_height: 0,
            current_preview_total_lines: 0,
            icon_color_cache: HashMap::default(),
            rendered_preview_cache: Arc::new(Mutex::new(RenderedPreviewCache::default())),
            spinner,
            spinner_state: SpinnerState::from(&spinner),
            app_metadata,
            colorscheme,
        }
    }

    /// Update the state of the component based on a received action.
    pub fn update(&mut self, action: &Action) -> Result<Option<Action>> {
        match action {
            // handle input actions
            Action::AddInputChar(_)
            | Action::DeletePrevChar
            | Action::DeletePrevWord
            | Action::DeleteNextChar
            | Action::GoToInputEnd
            | Action::GoToInputStart
            | Action::GoToNextChar
            | Action::GoToPrevChar => {
                let input = match self.mode {
                    Mode::Channel => &mut self.results_picker.input,
                    Mode::RemoteControl | Mode::SendToChannel => &mut self.rc_picker.input,
                };

                let request = match action {
                    Action::AddInputChar(c) => InputRequest::InsertChar(*c),
                    Action::DeletePrevChar => InputRequest::DeletePrevChar,
                    Action::DeletePrevWord => InputRequest::DeletePrevWord,
                    Action::DeleteNextChar => InputRequest::DeleteNextChar,
                    Action::GoToPrevChar => InputRequest::GoToPrevChar,
                    Action::GoToNextChar => InputRequest::GoToNextChar,
                    Action::GoToInputStart => InputRequest::GoToStart,
                    Action::GoToInputEnd => InputRequest::GoToEnd,
                    _ => unreachable!(),
                };

                input.handle(request);

                match action {
                    Action::AddInputChar(_)
                    | Action::DeletePrevChar
                    | Action::DeletePrevWord
                    | Action::DeleteNextChar => {
                        let new_pattern = input.value().to_string();
                        if new_pattern != self.current_pattern {
                            self.current_pattern.clone_from(&new_pattern);
                            self.find(&new_pattern);
                            self.reset_picker_selection();
                            self.reset_preview_scroll();
                        }
                    }
                    _ => {}
                }
            }
            Action::SelectNextEntry => {
                self.reset_preview_scroll();
                self.select_next_entry(1);
            }
            Action::SelectPrevEntry => {
                self.reset_preview_scroll();
                self.select_prev_entry(1);
            }
            Action::SelectNextPage => {
                self.reset_preview_scroll();
                self.select_next_entry(self.results_area_height);
            }
            Action::SelectPrevPage => {
                self.reset_preview_scroll();
                self.select_prev_entry(self.results_area_height);
            }
            Action::ScrollPreviewDown => self.scroll_preview_down(1),
            Action::ScrollPreviewUp => self.scroll_preview_up(1),
            Action::ScrollPreviewHalfPageDown => self.scroll_preview_down(20),
            Action::ScrollPreviewHalfPageUp => self.scroll_preview_up(20),
            Action::ToggleRemoteControl => {
                self.config.ui.show_remote_control = !self.config.ui.show_remote_control;

                match self.mode {
                    Mode::Channel => {
                        self.mode = Mode::RemoteControl;
                        self.init_remote_control();
                    }
                    Mode::RemoteControl => {
                        info!("Toggle remote");
                        // this resets the RC picker
                        self.reset_picker_input();
                        self.init_remote_control();
                        self.remote_control.find(EMPTY_STRING);
                        self.reset_picker_selection();
                        self.mode = Mode::Channel;
                    }
                    Mode::SendToChannel => {}
                }
            }
            Action::ToggleSelectionDown | Action::ToggleSelectionUp => {
                if matches!(self.mode, Mode::Channel) {
                    if let Some(entry) = self.get_selected_entry(None) {
                        self.channel.toggle_selection(&entry);
                        if matches!(action, Action::ToggleSelectionDown) {
                            self.select_next_entry(1);
                        } else {
                            self.select_prev_entry(1);
                        }
                    }
                }
            }
            Action::ConfirmSelection => {
                match self.mode {
                    Mode::Channel => {
                        self.action_tx
                            .as_ref()
                            .unwrap()
                            .send(Action::SelectAndExit)?;
                    }
                    Mode::RemoteControl => {
                        if let Some(entry) = self.get_selected_entry(Some(Mode::RemoteControl)) {
                            let new_channel = self.remote_control.zap(entry.name.as_str())?;
                            // this resets the RC picker
                            self.reset_picker_selection();
                            self.reset_picker_input();
                            self.remote_control.find(EMPTY_STRING);
                            self.mode = Mode::Channel;
                            self.change_channel(new_channel);
                        }
                    }
                    Mode::SendToChannel => {
                        if let Some(_entry) = self.get_selected_entry(Some(Mode::RemoteControl)) {
                            self.reset_picker_selection();
                            self.reset_picker_input();
                            self.remote_control.find(EMPTY_STRING);
                            self.mode = Mode::Channel;

                            todo!()
                            // let new_channel = self.channel.transition_to(
                            //     entry.name.as_str().try_into().unwrap(),
                            // );
                            // self.change_channel(new_channel);
                        }
                    }
                }
            }
            Action::CopyEntryToClipboard => {
                if self.mode == Mode::Channel {
                    if let Some(entries) = self.get_selected_entries(None) {
                        let mut ctx = ClipboardContext::new().unwrap();
                        ctx.set_contents(
                            entries
                                .iter()
                                .map(|e| e.name.clone())
                                .collect::<Vec<_>>()
                                .join(" ")
                                .to_string()
                                .to_string(),
                        )
                        .unwrap();
                    }
                }
            }
            Action::ToggleSendToChannel => match self.mode {
                Mode::Channel | Mode::RemoteControl => {
                    self.mode = Mode::SendToChannel;
                    warn!("Hit toggle send to channel path, remote_control not set");
                    // self.remote_control = TelevisionChannel::RemoteControl(
                    // RemoteControl::with_transitions_from(&self.channel),
                    // );
                }
                Mode::SendToChannel => {
                    self.reset_picker_input();
                    self.remote_control.find(EMPTY_STRING);
                    self.reset_picker_selection();
                    self.mode = Mode::Channel;
                }
            },
            Action::ToggleHelp => {
                self.config.ui.show_help_bar = !self.config.ui.show_help_bar;
            }
            Action::ToggleLogs => {
                self.config.ui.show_logs = !self.config.ui.show_logs;
            }
            Action::TogglePreview => {
                self.config.ui.show_preview_panel = !self.config.ui.show_preview_panel;
            }
            Action::Render
            | Action::Resize(_, _)
            | Action::ClearScreen
            | Action::SelectPassthrough(_)
            | Action::SelectAndExit
            | Action::OpenEntry
            | Action::Tick
            | Action::Suspend
            | Action::Resume
            | Action::Quit
            | Action::Error(_) => (),
            Action::NoOp => {
                // self.config.ui.show_remote_control = !self.config.ui.show_remote_control;
            }
        }
        Ok(None)
    }

    /// Render the television on the screen.
    pub fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let selected_entry = self
            .get_selected_entry(Some(Mode::Channel))
            .unwrap_or(ENTRY_PLACEHOLDER);

        let layout = Layout::build(
            &Dimensions::from(self.config.ui.ui_scale),
            area,
            self.config.ui.show_remote_control,
            self.config.ui.show_help_bar,
            self.config.ui.show_logs,
            self.config.ui.show_preview_panel,
            self.config.ui.input_bar_position,
        );

        // Draw Help Bar
        if let Some(help_bar) = &layout.help_bar {
            draw_help_bar(
                f,
                help_bar,
                self.current_channel(),
                &self.config.keybindings,
                self.mode,
                &self.app_metadata,
                &self.colorscheme,
            );
        }

        // Draw Logs
        if let Some(logs) = layout.logs {
            draw_logs_bar(f, logs, &self.colorscheme);
        }

        // Draw Results Section
        {
            // 2 for the borders
            self.results_area_height = u32::from(layout.results.results.height.saturating_sub(2));

            let result_count = self.channel.result_count();

            if result_count > 0 && self.results_picker.selected().is_none() {
                self.results_picker.select(Some(0));
                self.results_picker.relative_select(Some(0));
            }

            let entries = self.channel.results(
                self.results_area_height,
                u32::try_from(self.results_picker.offset())?,
            );

            draw_results_list(
                f,
                layout.results.results,
                &entries,
                self.channel.selected_entries(),
                &mut self.results_picker.relative_state,
                self.config.ui.input_bar_position,
                self.config.ui.use_nerd_font_icons,
                &mut self.icon_color_cache,
                &self.colorscheme,
                &self.config.keybindings.toggle_help.to_string(),
                &self.config.keybindings.toggle_preview.to_string(),
                &self.channel.name,
            )?;

            // input box
            draw_input_box(
                f,
                layout.results.input,
                result_count,
                self.channel.total_count(),
                &mut self.results_picker.input,
                &mut self.results_picker.state,
                self.channel.running(),
                &self.spinner,
                &mut self.spinner_state,
                &self.colorscheme,
                &self.app_metadata,
            )?;
        }

        // Draw Preview Content
        if self.config.ui.show_preview_panel {
            self.preview_pane_height = layout.preview_window.map_or(0, |preview| preview.height);

            let preview = self
                .previewer
                .preview(&selected_entry, &self.channel.preview_command);

            self.current_preview_total_lines = preview.total_lines();

            // initialize preview scroll
            self.maybe_init_preview_scroll(
                selected_entry
                    .line_number
                    .map(|l| u16::try_from(l).unwrap_or(0)),
                layout.preview_window.unwrap().height,
            );

            draw_preview_content_block(
                f,
                // This deviates from help and logs, wny exactly?
                layout.preview_window.unwrap(),
                &selected_entry,
                &preview,
                &self.rendered_preview_cache,
                self.preview_scroll.unwrap_or(0),
                self.config.ui.use_nerd_font_icons,
                &self.colorscheme,
            )?;
        }

        // Draw Remote Control
        if self.config.ui.show_remote_control {
            // NOTE: this should be done in the `update` method
            let result_count = self.remote_control.result_count();

            if result_count > 0 && self.rc_picker.selected().is_none() {
                self.rc_picker.select(Some(0));
                self.rc_picker.relative_select(Some(0));
            }

            let entries = self.remote_control.results(
                area.height.saturating_sub(2).into(),
                u32::try_from(self.rc_picker.offset())?,
            );

            draw_remote_control(
                f,
                layout.remote_control.unwrap(),
                &entries,
                self.config.ui.use_nerd_font_icons,
                &mut self.rc_picker.state,
                &mut self.rc_picker.input,
                &mut self.icon_color_cache,
                &self.mode,
                &self.colorscheme,
            )?;
        }
        Ok(())
    }
}

impl Television {
    pub fn init_remote_control(&mut self) {
        self.remote_control = RemoteControl::new(self.channels.clone());
    }

    pub fn current_channel(&self) -> &Channel {
        &self.channel
    }

    pub fn change_channel(&mut self, channel: Channel) {
        self.reset_preview_scroll();
        self.reset_picker_selection();
        self.reset_picker_input();
        self.current_pattern = EMPTY_STRING.to_string();
        self.channel.shutdown();
        self.channel = channel;
    }

    fn find(&mut self, pattern: &str) {
        match self.mode {
            Mode::Channel => {
                self.channel.find(pattern);
            }
            Mode::RemoteControl | Mode::SendToChannel => {
                self.remote_control.find(pattern);
            }
        }
    }

    #[must_use]
    pub fn get_selected_entry(&mut self, mode: Option<Mode>) -> Option<Entry> {
        match mode.unwrap_or(self.mode) {
            Mode::Channel => {
                if let Some(i) = self.results_picker.selected() {
                    return self.channel.get_result(i.try_into().unwrap());
                }
                None
            }
            Mode::RemoteControl | Mode::SendToChannel => {
                if let Some(i) = self.rc_picker.selected() {
                    return self.remote_control.get_result(i.try_into().unwrap());
                }
                None
            }
        }
    }

    #[must_use]
    pub fn get_selected_entries(&mut self, mode: Option<Mode>) -> Option<HashSet<Entry>> {
        if self.channel.selected_entries().is_empty() || matches!(mode, Some(Mode::RemoteControl)) {
            return self.get_selected_entry(mode).map(|e| {
                let mut set = HashSet::with_hasher(FxBuildHasher);
                set.insert(e);
                set
            });
        }
        Some(self.channel.selected_entries().clone())
    }

    pub fn select_prev_entry(&mut self, step: u32) {
        let (result_count, picker) = match self.mode {
            Mode::Channel => (self.channel.result_count(), &mut self.results_picker),
            Mode::RemoteControl | Mode::SendToChannel => {
                (self.remote_control.total_count(), &mut self.rc_picker)
            }
        };
        if result_count == 0 {
            return;
        }
        picker.select_prev(
            step,
            result_count as usize,
            self.results_area_height as usize,
        );
    }

    pub fn select_next_entry(&mut self, step: u32) {
        let (result_count, picker) = match self.mode {
            Mode::Channel => (self.channel.result_count(), &mut self.results_picker),
            Mode::RemoteControl | Mode::SendToChannel => {
                (self.remote_control.total_count(), &mut self.rc_picker)
            }
        };
        if result_count == 0 {
            return;
        }
        picker.select_next(
            step,
            result_count as usize,
            self.results_area_height as usize,
        );
    }

    pub fn maybe_init_preview_scroll(&mut self, target_line: Option<u16>, height: u16) {
        if self.preview_scroll.is_none() && !self.channel.running() {
            self.preview_scroll = Some(target_line.unwrap_or(0).saturating_sub(height / 3));
        }
    }

    fn reset_preview_scroll(&mut self) {
        self.preview_scroll = None;
    }

    fn reset_picker_selection(&mut self) {
        match self.mode {
            Mode::Channel => self.results_picker.reset_selection(),
            Mode::RemoteControl | Mode::SendToChannel => {
                self.rc_picker.reset_selection();
            }
        }
    }

    fn reset_picker_input(&mut self) {
        match self.mode {
            Mode::Channel => self.results_picker.reset_input(),
            Mode::RemoteControl | Mode::SendToChannel => {
                self.rc_picker.reset_input();
            }
        }
    }

    pub fn scroll_preview_down(&mut self, offset: u16) {
        if self.preview_scroll.is_none() {
            self.preview_scroll = Some(0);
        }
        if let Some(scroll) = self.preview_scroll {
            self.preview_scroll = Some(
                (scroll + offset).min(
                    self.current_preview_total_lines
                        .saturating_sub(2 * self.preview_pane_height / 3),
                ),
            );
        }
    }

    pub fn scroll_preview_up(&mut self, offset: u16) {
        if let Some(scroll) = self.preview_scroll {
            self.preview_scroll = Some(scroll.saturating_sub(offset));
        }
    }
}
