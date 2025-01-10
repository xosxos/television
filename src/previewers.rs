use std::sync::Arc;

use devicons::FileIcon;

// previewer types
pub use command::CommandPreviewer;
pub use command::CommandPreviewerConfig;
use syntect::highlighting::Style;

#[derive(Clone, Debug)]
pub enum PreviewContent {
    Empty,
    FileTooLarge,
    SyntectHighlightedText(Vec<Vec<(Style, String)>>),
    Loading,
    NotSupported,
    PlainText(Vec<String>),
    PlainTextWrapped(String),
    AnsiText(String),
}

pub const PREVIEW_NOT_SUPPORTED_MSG: &str = "Preview for this file type is not supported";
pub const FILE_TOO_LARGE_MSG: &str = "File too large";

/// A preview of an entry.
///
/// # Fields
/// - `title`: The title of the preview.
/// - `content`: The content of the preview.
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
            PreviewContent::SyntectHighlightedText(lines) => {
                lines.len().try_into().unwrap_or(u16::MAX)
            }
            PreviewContent::PlainText(lines) => lines.len().try_into().unwrap_or(u16::MAX),
            PreviewContent::AnsiText(text) => text.lines().count().try_into().unwrap_or(u16::MAX),
            _ => 0,
        }
    }
}


pub fn not_supported(title: &str) -> Arc<Preview> {
    Arc::new(Preview::new(
        title.to_string(),
        PreviewContent::NotSupported,
        None,
        false,
    ))
}

pub fn file_too_large(title: &str) -> Arc<Preview> {
    Arc::new(Preview::new(
        title.to_string(),
        PreviewContent::FileTooLarge,
        None,
        false,
    ))
}

#[allow(dead_code)]
pub fn loading(title: &str) -> Arc<Preview> {
    Arc::new(Preview::new(
        title.to_string(),
        PreviewContent::Loading,
        None,
        false,
    ))
}

pub mod command {
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::Arc;
    use rustc_hash::FxHashSet;
    
    use lazy_static::lazy_static;
    use parking_lot::Mutex;
    use regex::Regex;
    use tracing::debug;

    use crate::entry::{Entry, PreviewCommand};

    use crate::utils::shell_command;
    use crate::previewers::cache::PreviewCache;
    use crate::previewers::{Preview, PreviewContent};

    #[allow(dead_code)]
    #[derive(Debug, Default)]
    pub struct CommandPreviewer {
        cache: Arc<Mutex<PreviewCache>>,
        config: CommandPreviewerConfig,
        concurrent_preview_tasks: Arc<AtomicU8>,
        last_previewed: Arc<Mutex<Arc<Preview>>>,
        in_flight_previews: Arc<Mutex<FxHashSet<String>>>,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    pub struct CommandPreviewerConfig {
        delimiter: String,
    }

    const DEFAULT_DELIMITER: &str = ":";

    impl Default for CommandPreviewerConfig {
        fn default() -> Self {
            CommandPreviewerConfig {
                delimiter: String::from(DEFAULT_DELIMITER),
            }
        }
    }

    impl CommandPreviewerConfig {
        pub fn new(delimiter: &str) -> Self {
            CommandPreviewerConfig {
                delimiter: String::from(delimiter),
            }
        }
    }

    const MAX_CONCURRENT_PREVIEW_TASKS: u8 = 3;

    impl CommandPreviewer {
        pub fn new(config: Option<CommandPreviewerConfig>) -> Self {
            let config = config.unwrap_or_default();
            CommandPreviewer {
                cache: Arc::new(Mutex::new(PreviewCache::default())),
                config,
                concurrent_preview_tasks: Arc::new(AtomicU8::new(0)),
                last_previewed: Arc::new(Mutex::new(Arc::new(
                    Preview::default().stale(),
                ))),
                in_flight_previews: Arc::new(Mutex::new(FxHashSet::default())),
            }
        }

        pub fn preview(
            &mut self,
            entry: &Entry,
            command: &PreviewCommand,
        ) -> Arc<Preview> {
            // do we have a preview in cache for that entry?
            if let Some(preview) = self.cache.lock().get(&entry.name) {
                return preview.clone();
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
                let last_previewed = self.last_previewed.clone();
                tokio::spawn(async move {
                    try_preview(
                        &command,
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

    lazy_static! {
        static ref COMMAND_PLACEHOLDER_REGEX: Regex =
            Regex::new(r"\{(\d+)\}").unwrap();
    }

    /// Format the command with the entry name and provided placeholders
    pub fn format_command(command: &PreviewCommand, entry: &Entry) -> String {
        let parts = entry.name.split(&command.delimiter).collect::<Vec<&str>>();
        debug!("Parts: {:?}", parts);

        let mut formatted_command = command.command.replace("{}", &entry.name);

        formatted_command = COMMAND_PLACEHOLDER_REGEX
            .replace_all(&formatted_command, |caps: &regex::Captures| {
                let index =
                    caps.get(1).unwrap().as_str().parse::<usize>().unwrap();
                parts[index].to_string()
            })
            .to_string();

        formatted_command
    }

    pub fn try_preview(
        command: &PreviewCommand,
        entry: &Entry,
        cache: &Arc<Mutex<PreviewCache>>,
        concurrent_tasks: &Arc<AtomicU8>,
        last_previewed: &Arc<Mutex<Arc<Preview>>>,
    ) {
        debug!("Computing preview for {:?}", entry.name);
        let command = format_command(command, entry);
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

            cache.lock().insert(entry.name.clone(), &preview);
            let mut tp = last_previewed.lock();
            *tp = preview.stale().into();
        } else {
            let content = String::from_utf8_lossy(&output.stderr);
            let preview = Arc::new(Preview::new(
                entry.name.clone(),
                PreviewContent::AnsiText(content.to_string()),
                None,
                false,
            ));
            cache.lock().insert(entry.name.clone(), &preview);
        }

        concurrent_tasks.fetch_sub(1, Ordering::Relaxed);
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::entry::Entry;

        #[test]
        fn test_format_command() {
            let command = PreviewCommand {
                command: "something {} {2} {0}".to_string(),
                delimiter: ":".to_string(),
            };
            let entry = Entry::new("an:entry:to:preview".to_string());
            let formatted_command = format_command(&command, &entry);

            assert_eq!(formatted_command, "something an:entry:to:preview to an");
        }

        #[test]
        fn test_format_command_no_placeholders() {
            let command = PreviewCommand {
                command: "something".to_string(),
                delimiter: ":".to_string(),
            };
            let entry = Entry::new(
                "an:entry:to:preview".to_string(),
            );
            let formatted_command = format_command(&command, &entry);

            assert_eq!(formatted_command, "something");
        }

        #[test]
        fn test_format_command_with_global_placeholder_only() {
            let command = PreviewCommand {
                command: "something {}".to_string(),
                delimiter: ":".to_string(),
            };
            let entry = Entry::new(
                "an:entry:to:preview".to_string(),
            );
            let formatted_command = format_command(&command, &entry);

            assert_eq!(formatted_command, "something an:entry:to:preview");
        }

        #[test]
        fn test_format_command_with_positional_placeholders_only() {
            let command = PreviewCommand {
                command: "something {0} -t {2}".to_string(),
                delimiter: ":".to_string(),
            };
            let entry = Entry::new(
                "an:entry:to:preview".to_string(),
            );
            let formatted_command = format_command(&command, &entry);

            assert_eq!(formatted_command, "something an -t to");
        }
    }
}

pub mod cache {

    use rustc_hash::FxHashMap;
    use std::sync::Arc;

    use tracing::debug;

    use crate::previewers::Preview;
    use crate::utils::cache::RingSet;

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
        entries: FxHashMap<String, Arc<Preview>>,
        ring_set: RingSet<String>,
    }

    impl PreviewCache {
        /// Create a new preview cache with the given capacity.
        pub fn new(capacity: usize) -> Self {
            PreviewCache {
                entries: FxHashMap::default(),
                ring_set: RingSet::with_capacity(capacity),
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

        /// Get the preview for the given key, or insert a new preview if it doesn't exist.
        #[allow(dead_code)]
        pub fn get_or_insert<F>(&mut self, key: String, f: F) -> Arc<Preview>
        where
            F: FnOnce() -> Preview,
        {
            if let Some(preview) = self.get(&key) {
                preview
            } else {
                let preview = Arc::new(f());
                self.insert(key, &preview);
                preview
            }
        }
    }

    impl Default for PreviewCache {
        fn default() -> Self {
            PreviewCache::new(DEFAULT_PREVIEW_CACHE_SIZE)
        }
    }
}
