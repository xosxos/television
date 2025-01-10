use std::collections::HashSet;

use color_eyre::Result;
use devicons::FileIcon;
use rustc_hash::{FxBuildHasher, FxHashSet};

use crate::channels::cable::{CableChannelPrototype, CableChannels};
use crate::channels::{OnAir, TelevisionChannel, UnitChannel};
use crate::entry::Entry;
use crate::fuzzy::{Config, Matcher};

use super::cable;

pub struct RemoteControl {
    matcher: Matcher<CableChannelPrototype>,
    cable_channels: CableChannels,
    selected_entries: FxHashSet<Entry>,
}

const NUM_THREADS: usize = 1;

impl RemoteControl {
    pub fn new(cable_channels: CableChannels) -> Self {
        let matcher = Matcher::new(Config::default().n_threads(NUM_THREADS));
        let injector = matcher.injector();

        for channel in cable_channels
            .iter()
            .map(|(_, prototype)| prototype.clone())
        {
            let () = injector.push(channel.clone(), |e, cols| {
                cols[0] = e.to_string().clone().into();
            });
        }

        RemoteControl {
            matcher,
            cable_channels,
            selected_entries: HashSet::with_hasher(FxBuildHasher),
        }
    }

    pub fn zap(&self, channel_name: &str) -> Result<TelevisionChannel> {
        if let Ok(channel) = UnitChannel::try_from(channel_name) {
            Ok(channel.into())
        } else {
            let maybe_prototype = self.cable_channels.get(channel_name);
            match maybe_prototype {
                Some(prototype) => Ok(TelevisionChannel::Cable(cable::Channel::from(
                    prototype.clone(),
                ))),
                None => Err(color_eyre::eyre::eyre!(
                    "No channel or cable channel prototype found for {}",
                    channel_name
                )),
            }
        }
    }
}

const TV_ICON: FileIcon = FileIcon {
    icon: '📺',
    color: "#000000",
};

const CABLE_ICON: FileIcon = FileIcon {
    icon: '🍿',
    color: "#000000",
};

impl OnAir for RemoteControl {
    fn find(&mut self, pattern: &str) {
        self.matcher.find(pattern);
    }

    fn results(&mut self, num_entries: u32, offset: u32) -> Vec<Entry> {
        self.matcher.tick();
        self.matcher
            .results(num_entries, offset)
            .into_iter()
            .map(|item| {
                let path = item.matched_string;
                Entry::new(path)
                    .with_name_match_ranges(&item.match_indices)
                    .with_icon(CABLE_ICON)
            })
            .collect()
    }

    fn selected_entries(&self) -> &FxHashSet<Entry> {
        &self.selected_entries
    }

    fn toggle_selection(&mut self, _entry: &Entry) {}

    fn get_result(&self, index: u32) -> Option<Entry> {
        self.matcher.get_result(index).map(|item| {
            let path = item.matched_string;
            Entry::new(path).with_icon(TV_ICON)
        })
    }

    fn result_count(&self) -> u32 {
        self.matcher.matched_item_count
    }

    fn total_count(&self) -> u32 {
        self.matcher.total_item_count
    }

    fn running(&self) -> bool {
        self.matcher.status.running
    }

    fn shutdown(&self) {}
}
