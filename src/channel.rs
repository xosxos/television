use indexmap::IndexMap;
use rustc_hash::FxHashSet as HashSet;
use rustc_hash::{FxBuildHasher, FxHashSet};
use std::fmt::{self, Display, Formatter};
use std::{
    io::{BufRead, BufReader},
    process::Stdio,
    sync::LazyLock,
};

use regex::Regex;
use tracing::debug;

use crate::entry::Entry;
use crate::fuzzy::{Config, Injector, Matcher};
use crate::utils::shell_command;

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

#[derive(Debug, Clone, Eq, PartialEq, Hash, Default)]
pub struct PreviewCommand {
    pub command: String,
    pub delimiter: String,
}

impl PreviewCommand {
    pub fn new(command: &str, delimiter: &str) -> Self {
        Self {
            command: command.to_string(),
            delimiter: delimiter.to_string(),
        }
    }
}

impl Display for PreviewCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
pub struct ChannelConfig {
    pub name: String,
    pub source_command: String,
    pub preview_command: Option<String>,
    #[serde(default = "default_delimiter")]
    pub preview_delimiter: Option<String>,
}

pub const DEFAULT_DELIMITER: &str = " ";

#[allow(clippy::unnecessary_wraps)]
fn default_delimiter() -> Option<String> {
    Some(DEFAULT_DELIMITER.to_string())
}

impl Display for ChannelConfig {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub type ChannelConfigs = IndexMap<String, ChannelConfig>;

pub struct Channel {
    pub name: String,
    matcher: Matcher<String>,
    pub preview_command: PreviewCommand,
    selected_entries: FxHashSet<Entry>,
}

impl Default for Channel {
    fn default() -> Self {
        Self::new(
            "Files".to_string(),
            Some("find . -type f".to_string()),
            PreviewCommand::new("bat -n --color=always {}", ":"),
        )
    }
}

impl From<ChannelConfig> for Channel {
    fn from(config: ChannelConfig) -> Self {
        let command = match &config.preview_command {
            Some(command) => command,
            None => &String::new(),
        };

        Self::new(
            config.name,
            Some(config.source_command),
            PreviewCommand::new(
                command,
                &config
                    .preview_delimiter
                    .unwrap_or(DEFAULT_DELIMITER.to_string()),
            ),
        )
    }
}

static BUILTIN_PREVIEW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^:(\w+):$").unwrap());

// fn parse_preview_kind(command: &PreviewCommand) -> Result<PreviewKind> {
//     debug!("Parsing preview kind for command: {:?}", command);
//     if let Some(captures) = BUILTIN_PREVIEW_RE.captures(&command.command) {
//         let preview_type = PreviewType::try_from(&captures[1])?;
//         Ok(PreviewKind::Builtin(preview_type))
//     } else {
//         Ok(PreviewKind::Command(command.clone()))
//     }
// }

impl Channel {
    pub fn new(
        name: String,
        entries_command: Option<String>,
        preview_command: PreviewCommand,
    ) -> Self {
        let matcher = Matcher::new(Config::default());
        let injector = matcher.injector();

        match entries_command {
            Some(cmd) => {
                std::thread::spawn(move || load_candidates(cmd, &injector));
            }
            None => {
                std::thread::spawn(move || stream_from_stdin(&injector));
            }
        }

        Self {
            name,
            matcher,
            preview_command,
            selected_entries: HashSet::with_hasher(FxBuildHasher),
        }
    }
}

fn load_candidates(command: String, injector: &Injector<String>) {
    debug!("Loading candidates from command: {:?}", command);

    let mut child = shell_command()
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to execute process");

    if let Some(out) = child.stdout.take() {
        let reader = BufReader::new(out);

        for line in reader.lines() {
            let line = line.unwrap();
            if !line.trim().is_empty() {
                let () = injector.push(line, |e, cols| {
                    cols[0] = e.clone().into();
                });
            }
        }
    }
}

const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn stream_from_stdin(injector: &Injector<String>) {
    let mut stdin = std::io::stdin().lock();
    let mut buffer = String::new();

    let instant = std::time::Instant::now();
    loop {
        match stdin.read_line(&mut buffer) {
            Ok(c) if c > 0 => {
                if !buffer.trim().is_empty() {
                    injector.push(buffer.trim().to_string(), |e, cols| {
                        cols[0] = e.clone().into();
                    });
                }
                buffer.clear();
            }
            Ok(0) => {
                debug!("EOF");
                break;
            }
            _ => {
                debug!("Error reading from stdin");
                if instant.elapsed() > TIMEOUT {
                    break;
                }
            }
        }
    }
}

impl OnAir for Channel {
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
                Entry::new(path.clone()).with_name_match_ranges(&item.match_indices)
            })
            .collect()
    }

    fn get_result(&self, index: u32) -> Option<Entry> {
        self.matcher.get_result(index).map(|item| {
            let path = item.matched_string;
            Entry::new(path.clone())
        })
    }

    fn selected_entries(&self) -> &FxHashSet<Entry> {
        &self.selected_entries
    }

    fn toggle_selection(&mut self, entry: &Entry) {
        if self.selected_entries.contains(entry) {
            self.selected_entries.remove(entry);
        } else {
            self.selected_entries.insert(entry.clone());
        }
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
