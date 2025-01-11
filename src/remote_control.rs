use rustc_hash::{FxBuildHasher, FxHashSet as HashSet};

use color_eyre::Result;
use devicons::FileIcon;

use crate::channel::{Channel, ChannelConfig, ChannelConfigs, OnAir};
use crate::entry::Entry;
use crate::fuzzy::{Config, Matcher};

const NUM_THREADS: usize = 1;

const TV_ICON: FileIcon = FileIcon {
    icon: '📺',
    color: "#000000",
};

const CABLE_ICON: FileIcon = FileIcon {
    icon: '🍿',
    color: "#000000",
};

pub struct RemoteControl {
    matcher: Matcher<ChannelConfig>,
    channels: ChannelConfigs,
    selected_entries: HashSet<Entry>,
}

impl RemoteControl {
    pub fn new(channels: ChannelConfigs) -> Self {
        let matcher = Matcher::new(Config::default().n_threads(NUM_THREADS));
        let injector = matcher.injector();

        for channel in channels.values() {
            let () = injector.push(channel.clone(), |e, cols| {
                cols[0] = e.to_string().clone().into();
            });
        }

        RemoteControl {
            matcher,
            channels,
            selected_entries: HashSet::with_hasher(FxBuildHasher),
        }
    }

    pub fn zap(&self, channel_name: &str) -> Result<Channel> {
        match self.channels.get(channel_name) {
            Some(prototype) => Ok(Channel::from(prototype.clone())),
            None => Err(color_eyre::eyre::eyre!(
                "No channel or cable channel prototype found for {}",
                channel_name
            )),
        }
    }
}
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

    fn selected_entries(&self) -> &HashSet<Entry> {
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
