use std::sync::{Arc, LazyLock};

use devicons::FileIcon;

// previewer types
use std::sync::atomic::{AtomicU8, Ordering};
use rustc_hash::FxHashSet as HashSet;

use parking_lot::Mutex;
use regex::Regex;
use tracing::debug;

use crate::channel::{Channel, PreviewCommand};
use crate::entry::Entry;

use crate::utils::shell_command;
use crate::previewer::cache::PreviewCache;

#[derive(Clone, Debug)]
pub enum PreviewContent {
    Empty,
    FileTooLarge,
    Loading,
    NotSupported,
    AnsiText(String),
}

pub static COMMAND_PLACEHOLDER_REGEX: LazyLock<Regex> = LazyLock::new(||
        Regex::new(r"\{(\d+)\}").unwrap()
);

pub const PREVIEW_NOT_SUPPORTED_MSG: &str = "Preview for this file type is not supported";
pub const FILE_TOO_LARGE_MSG: &str = "File too large";

#[derive(Clone, Debug)]
pub struct Preview {
    pub title: String,
    pub content: PreviewContent,
    pub icon: Option<FileIcon>,
    pub stale: bool,
}

impl Default for Preview {
    fn default() -> Self {
        Preview {
            title: String::new(),
            content: PreviewContent::Empty,
            icon: None,
            stale: false,
        }
    }
}

impl Preview {
    pub fn new(
        title: String,
        content: PreviewContent,
        icon: Option<FileIcon>,
        stale: bool,
    ) -> Self {
        Preview {
            title,
            content,
            icon,
            stale,
        }
    }

    pub fn stale(&self) -> Self {
        Preview {
            stale: true,
            ..self.clone()
        }
    }

    pub fn total_lines(&self) -> u16 {
        match &self.content {
            PreviewContent::AnsiText(text) => text.lines().count().try_into().unwrap_or(u16::MAX),
            _ => 0,
        }
    }
}

#[derive(Debug, Default)]
pub struct Previewer {
    cache: Arc<Mutex<PreviewCache>>,
    concurrent_preview_tasks: Arc<AtomicU8>,
    last_previewed: Arc<Mutex<Arc<Preview>>>,
    in_flight_previews: Arc<Mutex<HashSet<String>>>,
}

const MAX_CONCURRENT_PREVIEW_TASKS: u8 = 3;

impl Previewer {
    pub fn new() -> Self {
        Previewer {
            cache: Arc::new(Mutex::new(PreviewCache::default())),
            concurrent_preview_tasks: Arc::new(AtomicU8::new(0)),
            last_previewed: Arc::new(Mutex::new(Arc::new(
                Preview::default().stale(),
            ))),
            in_flight_previews: Arc::new(Mutex::new(HashSet::default())),
        }
    }

    pub fn preview(
        &mut self,
        entry: &Entry,
        channel: &Channel,
    ) -> Arc<Preview> {
        let command = channel.current_preview_command();
        let delimiter = &channel.delimiter;
        // do we have a preview in cache for that entry?
        let cache_key = format!("{}{}", entry.name, command.command);

        if let Some(preview) = self.cache.lock().get(&cache_key) {
            return preview;
        }
        debug!("Preview cache miss for {:?}", entry.name);

        // are we already computing a preview in the background for that entry?
        if self.in_flight_previews.lock().contains(&entry.name) {
            debug!("Preview already in flight for {:?}", entry.name);
            return self.last_previewed.lock().clone();
        }

        if self.concurrent_preview_tasks.load(Ordering::Relaxed)
            < MAX_CONCURRENT_PREVIEW_TASKS
        {
            self.concurrent_preview_tasks
                .fetch_add(1, Ordering::Relaxed);
            let cache = self.cache.clone();
            let entry_c = entry.clone();
            let concurrent_tasks = self.concurrent_preview_tasks.clone();
            let command = command.clone();
            let delimiter = delimiter.clone();
            let last_previewed = self.last_previewed.clone();

            tokio::spawn(async move {
                try_preview(
                    &command,
                    &delimiter,
                    &entry_c,
                    &cache,
                    &concurrent_tasks,
                    &last_previewed,
                );
            });
        } else {
            debug!("Too many concurrent preview tasks running");
        }

        self.last_previewed.lock().clone()
    }
}

/// Format the command with the entry name and provided placeholders
pub fn format_command(command: &String, delimiter: &String, entry: &Entry) -> Option<String> {
    let parts = entry.name.split(delimiter).collect::<Vec<&str>>();

    if entry.name.trim().is_empty() {
        return None;
    }

    debug!("Parts: {:?}", parts);

    let mut formatted_command = command.replace("{}", &entry.name);

    formatted_command = COMMAND_PLACEHOLDER_REGEX
        .replace_all(&formatted_command, |caps: &regex::Captures| {
            let index =
                caps.get(1).unwrap().as_str().parse::<usize>().unwrap();

            if let Some(part) = parts.get(index) { part } else {
                let count = index + 1;
                panic!("The entry: {:?} did not have {count} parts\nbut the preview command: {:?}\nrequires {count} parts",
                    entry.name, command
                );
            }
        })
        .to_string();

    Some(formatted_command)
}

pub fn try_preview(
    prev_command: &PreviewCommand,
    delimiter: &String,
    entry: &Entry,
    cache: &Arc<Mutex<PreviewCache>>,
    concurrent_tasks: &Arc<AtomicU8>,
    last_previewed: &Arc<Mutex<Arc<Preview>>>,
) {
    debug!("Computing preview for {:?}", entry.name);

    if let Some(command) = format_command(&prev_command.command, delimiter, entry) {
        debug!("Formatted preview command: {:?}", command);

        let output = shell_command()
            .arg(&command)
            .output()
            .expect("failed to execute process");

        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout);
            let preview = Arc::new(Preview::new(
                entry.name.clone(),
                PreviewContent::AnsiText(content.to_string()),
                None,
                false,
            ));

            let cache_key = format!("{}{}", entry.name, prev_command.command);
            cache.lock().insert(cache_key, &preview);
            let mut tp = last_previewed.lock();
            *tp = preview.stale().into();
        } else {
            let content = String::from_utf8_lossy(&output.stderr);
            let error = format!("error running command: {}\n{}", command, content);

            let preview = Arc::new(Preview::new(
                entry.name.clone(),
                PreviewContent::AnsiText(error.to_string()),
                None,
                false,
            ));

            let cache_key = format!("{}{}", entry.name, prev_command.command);
            cache.lock().insert(cache_key, &preview);
        }
    }

    concurrent_tasks.fetch_sub(1, Ordering::Relaxed);
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry::Entry;

    #[test]
    fn test_format_command() {
        let delimiter = ":".to_string();
        let command = PreviewCommand {
            command: "something {} {2} {0}".to_string(),
        };
        let entry = Entry::new("an:entry:to:preview".to_string());
        let formatted_command = format_command(&command.command, &delimiter, &entry).unwrap();

        assert_eq!(formatted_command, "something an:entry:to:preview to an");
    }

    #[test]
    fn test_format_command_no_placeholders() {
        let delimiter = ":".to_string();
        let command = PreviewCommand {
            command: "something".to_string(),
        };
        let entry = Entry::new(
            "an:entry:to:preview".to_string(),
        );
        let formatted_command = format_command(&command.command, &delimiter, &entry).unwrap();

        assert_eq!(formatted_command, "something");
    }

    #[test]
    fn test_format_command_with_global_placeholder_only() {
        let delimiter = ":".to_string();
        let command = PreviewCommand {
            command: "something {}".to_string(),
        };
        let entry = Entry::new(
            "an:entry:to:preview".to_string(),
        );
        let formatted_command = format_command(&command.command, &delimiter, &entry).unwrap();

        assert_eq!(formatted_command, "something an:entry:to:preview");
    }

    #[test]
    fn test_format_command_with_positional_placeholders_only() {
        let delimiter = ":".to_string();
        let command = PreviewCommand {
            command: "something {0} -t {2}".to_string(),
        };
        let entry = Entry::new(
            "an:entry:to:preview".to_string(),
        );
        let formatted_command = format_command(&command.command, &delimiter, &entry).unwrap();

        assert_eq!(formatted_command, "something an -t to");
    }
}

pub mod rendered_cache {
    use rustc_hash::FxHashMap as HashMap;
    use std::sync::Arc;

    use ratatui::widgets::Paragraph;

    use crate::previewer::cache::ring_set::RingSet;

    const DEFAULT_RENDERED_PREVIEW_CACHE_SIZE: usize = 25;

    #[derive(Debug)]
    pub struct RenderedPreviewCache<'a> {
        previews: HashMap<String, Arc<Paragraph<'a>>>,
        ring_set: RingSet<String>,
    }

    impl<'a> RenderedPreviewCache<'a> {
        pub fn new(capacity: usize) -> Self {
            RenderedPreviewCache {
                previews: HashMap::default(),
                ring_set: RingSet::with_capacity(capacity),
            }
        }

        pub fn get(&self, key: &str) -> Option<Arc<Paragraph<'a>>> {
            self.previews.get(key).cloned()
        }

        pub fn insert(&mut self, key: String, preview: &Arc<Paragraph<'a>>) {
            self.previews.insert(key.clone(), preview.clone());

            if let Some(oldest_key) = self.ring_set.push(key) {
                self.previews.remove(&oldest_key);
            }
        }
    }

    impl Default for RenderedPreviewCache<'_> {
        fn default() -> Self {
            RenderedPreviewCache::new(DEFAULT_RENDERED_PREVIEW_CACHE_SIZE)
        }
    }
}

pub mod cache {
    use rustc_hash::FxHashMap as HashMap;
    use std::sync::Arc;

    use tracing::debug;

    use crate::previewer::Preview;

    /// Default size of the preview cache: 100 entries.
    ///
    /// This does seem kind of arbitrary for now, will need to play around with it.
    /// At the moment, files over 4 MB are not previewed, so the cache size
    /// should never exceed 400 MB.
    const DEFAULT_PREVIEW_CACHE_SIZE: usize = 100;

    /// A cache for previews.
    /// The cache is implemented as an LRU cache with a fixed size.
    #[derive(Debug)]
    pub struct PreviewCache {
        entries: HashMap<String, Arc<Preview>>,
        ring_set: ring_set::RingSet<String>,
    }

    impl PreviewCache {
        /// Create a new preview cache with the given capacity.
        pub fn new(capacity: usize) -> Self {
            PreviewCache {
                entries: HashMap::default(),
                ring_set: ring_set::RingSet::with_capacity(capacity),
            }
        }

        pub fn get(&self, key: &str) -> Option<Arc<Preview>> {
            self.entries.get(key).cloned()
        }

        /// Insert a new preview into the cache.
        /// If the cache is full, the oldest entry will be removed.
        /// If the key is already in the cache, the preview will be updated.
        pub fn insert(&mut self, key: String, preview: &Arc<Preview>) {
            debug!("Inserting preview into cache: {}", key);

            self.entries.insert(key.clone(), Arc::clone(preview));

            if let Some(oldest_key) = self.ring_set.push(key) {
                debug!("Cache full, removing oldest entry: {}", oldest_key);
                self.entries.remove(&oldest_key);
            }
        }
    }

    impl Default for PreviewCache {
        fn default() -> Self {
            PreviewCache::new(DEFAULT_PREVIEW_CACHE_SIZE)
        }
    }

    pub mod ring_set {
        use rustc_hash::{FxBuildHasher, FxHashSet};
        use std::collections::{HashSet, VecDeque};
        use tracing::debug;

        /// A ring buffer that also keeps track of the keys it contains to avoid duplicates.
        ///
        /// This serves as a backend for the preview cache.
        /// Basic idea:
        /// - When a new key is pushed, if it's already in the buffer, do nothing.
        /// - If the buffer is full, remove the oldest key and push the new key.
        ///
        #[derive(Debug)]
        pub struct RingSet<T> {
            ring_buffer: VecDeque<T>,
            known_keys: FxHashSet<T>,
            capacity: usize,
        }

        impl<T> RingSet<T>
        where
            T: Eq + std::hash::Hash + Clone + std::fmt::Debug,
        {
            /// Create a new `RingSet` with the given capacity.
            pub fn with_capacity(capacity: usize) -> Self {
                RingSet {
                    ring_buffer: VecDeque::with_capacity(capacity),
                    known_keys: HashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
                    capacity,
                }
            }

            /// Push a new item to the back of the buffer, removing the oldest item if the buffer is full.
            /// Returns the item that was removed, if any.
            /// If the item is already in the buffer, do nothing and return None.
            pub fn push(&mut self, item: T) -> Option<T> {
                // If the key is already in the buffer, do nothing
                if self.contains(&item) {
                    debug!("Key already in ring buffer: {:?}", item);
                    return None;
                }

                let mut popped_key = None;

                // If the buffer is full, remove the oldest key (e.g. pop from the front of the buffer)
                if self.ring_buffer.len() >= self.capacity {
                    popped_key = self.pop();
                }
                // finally, push the new key to the back of the buffer
                self.ring_buffer.push_back(item.clone());
                self.known_keys.insert(item);
                popped_key
            }

            fn pop(&mut self) -> Option<T> {
                if let Some(item) = self.ring_buffer.pop_front() {
                    debug!("Removing key from ring buffer: {:?}", item);
                    self.known_keys.remove(&item);
                    Some(item)
                } else {
                    None
                }
            }

            pub fn contains(&self, key: &T) -> bool {
                self.known_keys.contains(key)
            }
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            #[test]
            fn test_ring_set() {
                let mut ring_set = RingSet::with_capacity(3);
                // push 3 values into the ringset
                assert_eq!(ring_set.push(1), None);
                assert_eq!(ring_set.push(2), None);
                assert_eq!(ring_set.push(3), None);

                // check that the values are in the buffer
                assert!(ring_set.contains(&1));
                assert!(ring_set.contains(&2));
                assert!(ring_set.contains(&3));

                // push an existing value (should do nothing)
                assert_eq!(ring_set.push(1), None);

                // entries should still be there
                assert!(ring_set.contains(&1));
                assert!(ring_set.contains(&2));
                assert!(ring_set.contains(&3));

                // push a new value, should remove the oldest value (1)
                assert_eq!(ring_set.push(4), Some(1));

                // 1 is no longer there but 2 and 3 remain
                assert!(!ring_set.contains(&1));
                assert!(ring_set.contains(&2));
                assert!(ring_set.contains(&3));
                assert!(ring_set.contains(&4));

                // push two new values, should remove 2 and 3
                assert_eq!(ring_set.push(5), Some(2));
                assert_eq!(ring_set.push(6), Some(3));

                // 2 and 3 are no longer there but 4, 5 and 6 remain
                assert!(!ring_set.contains(&2));
                assert!(!ring_set.contains(&3));
                assert!(ring_set.contains(&4));
                assert!(ring_set.contains(&5));
                assert!(ring_set.contains(&6));
            }
        }
    }

}
