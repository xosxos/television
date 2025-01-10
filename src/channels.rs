use crate::entry::Entry;
use color_eyre::Result;
use enum_dispatch::enum_dispatch;
use rustc_hash::FxHashSet;

pub mod cable;
pub mod remote_control;
pub mod stdin;
mod text;

#[enum_dispatch(TelevisionChannel)]
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
    fn selected_entries(&self) -> &FxHashSet<Entry>;

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

/// The available television channels.
///
/// Each channel is represented by a variant of the enum and should implement
/// the `OnAir` trait.
///
/// # Important
/// When adding a new channel, make sure to add a new variant to this enum and
/// implement the `OnAir` trait for it.
///
/// # Derive
/// ## `CliChannel`
/// The `CliChannel` derive macro generates the necessary glue code to
/// automatically create the corresponding `CliTvChannel` enum with unit
/// variants that can be used to select the channel from the command line.
/// It also generates the necessary glue code to automatically create a channel
/// instance from the selected CLI enum variant.
///
/// ## `Broadcast`
/// The `Broadcast` derive macro generates the necessary glue code to
/// automatically forward method calls to the corresponding channel variant.
/// This allows to use the `OnAir` trait methods directly on the `TelevisionChannel`
/// enum. In a more straightforward way, it implements the `OnAir` trait for the
/// `TelevisionChannel` enum.
///
/// ## `UnitChannel`
/// This macro generates an enum with unit variants that can be used instead
/// of carrying the actual channel instances around. It also generates the necessary
/// glue code to automatically create a channel instance from the selected enum variant.
#[allow(dead_code, clippy::module_name_repetitions)]
#[enum_dispatch]
pub enum TelevisionChannel {
    /// The text channel.
    ///
    /// This channel allows to search through the contents of text files.
    Text(text::Channel),
    /// The standard input channel.
    ///
    /// This channel allows to search through whatever is passed through stdin.
    Stdin(stdin::Channel),
    /// The remote control channel.
    ///
    /// This channel allows to switch between different channels.
    RemoteControl(remote_control::RemoteControl),
    /// A custom channel.
    ///
    /// This channel allows to search through custom data.
    Cable(cable::Channel),
}

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::default::Default;
use strum::{Display, EnumIter, EnumString};

#[rustfmt::skip]
#[derive(Debug, Clone, ValueEnum, EnumIter, EnumString, Default, Copy, PartialEq, Eq, Serialize, Deserialize, Display)]
#[strum(serialize_all = "kebab_case")]
pub enum CliTvChannel {
    #[default]
    Text,
    // Stdin,
    // RemoteControl,
    // Cable,
}

impl CliTvChannel {
    pub fn to_channel(self) -> TelevisionChannel {
        match self {
            CliTvChannel::Text => TelevisionChannel::Text(text::Channel::default()),
            // CliTvChannel::Stdin => TelevisionChannel::Stdin(stdin::Channel::default()),
            // CliTvChannel::RemoteControl => {
            // TelevisionChannel::RemoteControl(remote_control::RemoteControl::default())
            // }
            // CliTvChannel::Cable => TelevisionChannel::Cable(cable::Channel::default()),
        }
    }

    pub fn all_channels() -> Vec<String> {
        use strum::IntoEnumIterator;
        Self::iter().map(|v| v.to_string()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, EnumString)]
#[strum(serialize_all = "kebab_case")]
pub enum UnitChannel {
    Text,
    Stdin,
    // RemoteControl,
    Cable,
}

impl From<UnitChannel> for TelevisionChannel {
    fn from(val: UnitChannel) -> Self {
        match val {
            UnitChannel::Text => TelevisionChannel::Text(text::Channel::default()),
            UnitChannel::Stdin => TelevisionChannel::Stdin(stdin::Channel::default()),
            // CliTvChannel::RemoteControl => {
            // TelevisionChannel::RemoteControl(remote_control::RemoteControl::default())
            // }
            UnitChannel::Cable => TelevisionChannel::Cable(cable::Channel::default()),
        }
    }
}

impl From<&TelevisionChannel> for UnitChannel {
    fn from(channel: &TelevisionChannel) -> Self {
        match channel {
            TelevisionChannel::Text(_) => UnitChannel::Text,
            TelevisionChannel::Stdin(_) => UnitChannel::Stdin,
            TelevisionChannel::Cable(_) => UnitChannel::Cable,
            TelevisionChannel::RemoteControl(_) => {
                panic!("Cannot convert excluded variant to unit channel.")
            }
        }
    }
}

impl From<&Entry> for TelevisionChannel {
    fn from(entry: &Entry) -> Self {
        UnitChannel::try_from(entry.name.as_str()).unwrap().into()
    }
}

impl TelevisionChannel {
    pub fn zap(&self, channel_name: &str) -> Result<TelevisionChannel> {
        match self {
            TelevisionChannel::RemoteControl(remote_control) => remote_control.zap(channel_name),
            _ => unreachable!(),
        }
    }
}
