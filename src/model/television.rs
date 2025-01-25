use ratatui::widgets::ListState;
use rustc_hash::{FxBuildHasher, FxHashMap as HashMap, FxHashSet as HashSet};
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::sync::{Arc, Mutex};

use color_eyre::Result;
use copypasta::{ClipboardContext, ClipboardProvider};
use ratatui::{layout::Rect, style::Color, Frame};
use rayon::prelude::*;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info};

use crate::action::Action;
use crate::channel::PreviewCommand;
use crate::colors::{Colorscheme, ModeColorscheme};
use crate::config::{Config, Theme};
use crate::previewer::format_command;
use crate::strings::EMPTY_STRING;
use crate::utils::{shell_command, AppMetadata};

use crate::model::channel::{Channel, ChannelConfigs};
use crate::model::entry::{Entry, ENTRY_PLACEHOLDER};
use crate::model::input::InputRequest;
use crate::model::picker::Picker;
use crate::model::previewer::rendered_cache::RenderedPreviewCache;
use crate::model::previewer::Previewer;
use crate::model::remote_control::RemoteControl;

use crate::view::help::draw_help;
use crate::view::layout::{Dimensions, Layout};
use crate::view::logs::draw_logs;
use crate::view::preview::draw_preview;
use crate::view::remote_control::draw_remote_control;
use crate::view::results::{draw_input, draw_results, InputPosition};
use crate::view::spinner::{Spinner, SpinnerState};

use serde::{Deserialize, Serialize};

#[derive(PartialEq, Copy, Clone, Hash, Eq, Debug, Serialize, Deserialize, strum::Display)]
pub enum Mode {
    #[serde(rename = "channel")]
    #[strum(serialize = "Channel")]
    Channel,
    #[serde(rename = "remote_control")]
    #[strum(serialize = "Remote Control")]
    RemoteControl,
    #[serde(rename = "transition")]
    #[strum(serialize = "Transition")]
    Transition,
    #[serde(rename = "preview")]
    #[strum(serialize = "Preview")]
    Preview,
    #[serde(rename = "run")]
    #[strum(serialize = "Run")]
    Run,
}

impl Mode {
    pub fn color(&self, colorscheme: &ModeColorscheme) -> Color {
        match &self {
            Mode::Channel => colorscheme.channel,
            Mode::RemoteControl => colorscheme.remote_control,
            Mode::Transition | Mode::Preview | Mode::Run => colorscheme.send_to_channel,
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
    pub log_scroll: ListState,
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
            log_scroll: ListState::default(),
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
                    Mode::RemoteControl => &mut self.rc_picker.input,
                    Mode::Preview | Mode::Transition | Mode::Run => return Ok(Some(Action::NoOp)),
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
            Action::SelectNextPreview => {
                self.channel.select_next_preview_command();
                self.reset_preview_scroll();
            }
            Action::SelectPrevPreview => {
                self.channel.select_prev_preview_command();
                self.reset_preview_scroll();
            }
            Action::SelectPreview(index) => {
                self.reset_preview_scroll();
                self.channel.set_current_preview_command(*index);
            }
            Action::SelectNextTransition => {
                self.channel.select_next_transition_command();
                self.reset_preview_scroll();
            }
            Action::SelectPrevTransition => {
                self.channel.select_prev_transition_command();
                self.reset_preview_scroll();
            }
            Action::SelectTransition(index) => {
                self.reset_preview_scroll();
                self.channel.set_current_transition_command(*index);
            }
            Action::SelectNextRun => self.channel.select_next_run_command(),
            Action::SelectPrevRun => self.channel.select_prev_run_command(),
            Action::SelectRun(index) => {
                self.channel.set_current_run_command(*index);
            }
            Action::ScrollPreviewDown => self.scroll_preview_down(1),
            Action::ScrollPreviewUp => self.scroll_preview_up(1),
            Action::ScrollLogUp => {
                let offset = self.log_scroll.offset_mut();
                *offset = offset.saturating_sub(5);
            }
            Action::ScrollLogDown => {
                let offset = self.log_scroll.offset_mut();
                *offset = offset.saturating_add(5);
            }
            Action::ScrollPreviewHalfPageDown => self.scroll_preview_down(20),
            Action::ScrollPreviewHalfPageUp => self.scroll_preview_up(20),
            Action::ToggleRemoteControl => {
                self.config.ui.show_remote_control = !self.config.ui.show_remote_control;

                debug!("Mode before toggle: {}", self.mode);

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
                    Mode::Preview | Mode::Transition | Mode::Run => {}
                }

                debug!("Mode after toggle: {}", self.mode);
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
                    Mode::Channel | Mode::Run => {
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
                            self.config.ui.show_remote_control = false;
                        }
                    }
                    Mode::Preview => unreachable!(),
                    Mode::Transition => {
                        let transition = self.channel.current_transition_command().clone();

                        let channel = self.channels.get(&transition.channel).unwrap().clone();

                        let preview_commands = channel
                            .preview_command
                            .iter()
                            .map(|s| PreviewCommand::new(s))
                            .collect();

                        let mut lines = if let Some(entries) = self.get_selected_entries(None) {
                            debug!("perform transition on entries");
                            println!("perform transition on entries");

                            entries
                                .par_iter()
                                .flat_map(|entry| {
                                    if let Some(command) = format_command(
                                        &transition.command,
                                        &channel.delimiter,
                                        entry,
                                    ) {
                                        debug!("Formatted preview command: {:?}", command);
                                        println!("Formatted preview command: {:?}", command);

                                        let mut child = shell_command()
                                            .arg(command)
                                            .stdout(Stdio::piped())
                                            .stderr(Stdio::piped())
                                            .spawn()
                                            .expect("failed to execute process");

                                        let mut lines = vec![];
                                        if let Some(out) = child.stdout.take() {
                                            let reader = BufReader::new(out);

                                            for line in reader.lines() {
                                                let line = line.unwrap();

                                                lines.push(line);
                                            }
                                        }
                                        lines
                                    } else {
                                        vec![]
                                    }
                                })
                                .collect::<Vec<_>>()
                        } else {
                            debug!("perform transition on singles");
                            println!("perform transition on singles");
                            self.channel
                                .results(1_000_000, 0)
                                .par_iter()
                                .flat_map(|entry| {
                                    if let Some(command) = format_command(
                                        &transition.command,
                                        &channel.delimiter,
                                        entry,
                                    ) {
                                        debug!("Formatted preview command: {:?}", command);
                                        println!("Formatted preview command: {:?}", command);

                                        let mut child = shell_command()
                                            .arg(command)
                                            .stdout(Stdio::piped())
                                            .stderr(Stdio::piped())
                                            .spawn()
                                            .expect("failed to execute process");

                                        let mut lines = vec![];
                                        if let Some(out) = child.stdout.take() {
                                            let reader = BufReader::new(out);

                                            for line in reader.lines() {
                                                let line = line.unwrap();

                                                lines.push(line);
                                            }
                                        }
                                        lines
                                    } else {
                                        vec![]
                                    }
                                })
                                .collect::<Vec<_>>()
                        };

                        lines.sort();
                        lines.dedup();

                        let new_channel = Channel::new(
                            channel.name.clone(),
                            Some(channel.source_command.clone()),
                            preview_commands,
                            channel.run_command.clone(),
                            channel.transition_command.clone(),
                            channel.delimiter,
                            Some(lines),
                            channel.refresh,
                        );

                        self.channel = new_channel;
                        self.reset_picker_input();
                        self.reset_picker_selection();
                        self.config.ui.show_help_bar = false;
                        self.mode = Mode::Channel;
                        println!("finishedd transitioning");
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
            Action::ToggleTransition => {
                if self.mode == Mode::Transition {
                    self.config.ui.show_help_bar = false;
                    self.mode = Mode::Channel;
                } else {
                    self.config.ui.show_help_bar = true;
                    self.mode = Mode::Transition;
                }
            }
            Action::TogglePreviewCommands => {
                if self.mode == Mode::Preview {
                    self.config.ui.show_help_bar = false;
                    self.mode = Mode::Channel;
                } else {
                    self.config.ui.show_help_bar = true;
                    self.mode = Mode::Preview;
                }
            }
            Action::ToggleRunCommands => {
                if self.mode == Mode::Run {
                    self.config.ui.show_help_bar = false;
                    self.mode = Mode::Channel;
                } else {
                    self.config.ui.show_help_bar = true;
                    self.mode = Mode::Run;
                }
            }
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
            | Action::Error(_)
            | Action::NoOp => (),
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
            self.config.ui.show_preview_panel && !self.channel.preview_command.is_empty(),
            self.config.ui.input_bar_position,
        );

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

            draw_results(
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
            draw_input(
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
        if let Some(preview_area) = layout.preview {
            self.preview_pane_height = layout.preview.map_or(0, |preview| preview.height);

            let preview = self.previewer.preview(&selected_entry, &self.channel);

            self.current_preview_total_lines = preview.total_lines();

            // initialize preview scroll
            self.maybe_init_preview_scroll(
                selected_entry
                    .line_number
                    .map(|l| u16::try_from(l).unwrap_or(0)),
                preview_area.height,
            );

            draw_preview(
                f,
                preview_area,
                &selected_entry,
                &preview,
                &self.rendered_preview_cache,
                self.channel.current_preview_command(),
                self.preview_scroll.unwrap_or(0),
                self.config.ui.use_nerd_font_icons,
                &self.colorscheme,
            )?;
        }

        // Draw Help
        if let Some(help_area) = &layout.help {
            draw_help(
                f,
                help_area,
                self.current_channel(),
                &self.config.keybindings,
                self.mode,
                &self.app_metadata,
                &self.colorscheme,
            );
        }

        // Draw Logger
        if let Some(logs_area) = layout.logs {
            draw_logs(f, logs_area, &self.colorscheme, &mut self.log_scroll);
        }

        // Draw Remote Control
        if let Some(remote_control_area) = layout.remote_control {
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
                remote_control_area,
                &entries,
                self.config.ui.use_nerd_font_icons,
                &mut self.rc_picker.state,
                &mut self.rc_picker.input,
                &mut self.icon_color_cache,
                &self.mode,
                &self.colorscheme,
            )?;
        }
        println!("foo");

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
            Mode::RemoteControl | Mode::Transition => {
                self.remote_control.find(pattern);
            }
            Mode::Channel | Mode::Run | Mode::Preview => {
                self.channel.find(pattern);
            }
        }
    }

    #[must_use]
    pub fn get_selected_entry(&mut self, mode: Option<Mode>) -> Option<Entry> {
        match mode.unwrap_or(self.mode) {
            Mode::Channel | Mode::Run | Mode::Preview | Mode::Transition => {
                if let Some(i) = self.results_picker.selected() {
                    return self.channel.get_result(i.try_into().unwrap());
                }
                None
            }
            Mode::RemoteControl => {
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
            Mode::Channel | Mode::Run | Mode::Preview | Mode::Transition => {
                (self.channel.result_count(), &mut self.results_picker)
            }
            Mode::RemoteControl => (self.remote_control.total_count(), &mut self.rc_picker),
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
            Mode::Channel | Mode::Run | Mode::Preview | Mode::Transition => {
                (self.channel.result_count(), &mut self.results_picker)
            }
            Mode::RemoteControl => (self.remote_control.total_count(), &mut self.rc_picker),
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
            Mode::Channel | Mode::Run | Mode::Preview | Mode::Transition => {
                self.results_picker.reset_selection();
            }
            Mode::RemoteControl => {
                self.rc_picker.reset_selection();
            }
        }
    }

    fn reset_picker_input(&mut self) {
        match self.mode {
            Mode::Channel | Mode::Run | Mode::Preview | Mode::Transition => {
                self.results_picker.reset_input();
            }
            Mode::RemoteControl => {
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
