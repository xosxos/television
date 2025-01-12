use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::time::Duration;

use color_eyre::Result;
use indexmap::IndexMap;
use rustc_hash::{FxBuildHasher, FxHashSet as HashSet};
use tracing::{debug, error};

use crate::config::get_config_dir;
use crate::entry::Entry;
use crate::fuzzy::{Config, Injector, Matcher};
use crate::television::OnAir;
use crate::utils::shell_command;

const CABLE_FILE_NAME_SUFFIX: &str = "channels";
const CABLE_FILE_FORMAT: &str = "toml";

const DEFAULT_CABLE_CHANNELS: &str = include_str!("../config/channels.toml");

pub const DEFAULT_DELIMITER: &str = " ";

const TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, serde::Deserialize, PartialEq)]
pub struct ChannelConfig {
    pub name: String,
    #[serde(rename = "source")]
    pub source_command: String,
    #[serde(rename = "preview")]
    pub preview_command: Option<String>,
    #[serde(default = "default_delimiter")]
    #[serde(rename = "delimiter")]
    pub preview_delimiter: Option<String>,
    #[serde(rename = "run")]
    pub run_command: Option<String>,
}

#[allow(clippy::unnecessary_wraps)]
fn default_delimiter() -> Option<String> {
    Some(DEFAULT_DELIMITER.to_string())
}

pub type ChannelConfigs = IndexMap<String, ChannelConfig>;

pub struct Channel {
    pub name: String,
    matcher: Matcher<String>,
    pub preview_command: PreviewCommand,
    pub run_command: Option<String>,
    selected_entries: HashSet<Entry>,
}

impl Default for Channel {
    fn default() -> Self {
        Self::new(
            "Files".to_string(),
            Some("find . -type f".to_string()),
            PreviewCommand::new("bat -n --color=always {}", ":"),
            None,
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
            config.run_command,
        )
    }
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

    pub fn defaults(name: &str) -> Self {
        let command = match name {
            "files" => "bat {0}",
            _ => "echo {}",
        };

        Self::new(command, DEFAULT_DELIMITER)
    }
}

impl Channel {
    pub fn new(
        name: String,
        entries_command: Option<String>,
        preview_command: PreviewCommand,
        run_command: Option<String>,
    ) -> Self {
        let matcher = Matcher::new(Config::default());
        let injector = matcher.injector();

        match entries_command {
            Some(command) => {
                std::thread::spawn(move || entries_from_shell_process(command, &injector));
            }
            None => {
                std::thread::spawn(move || entries_from_stdin(&injector));
            }
        }

        Self {
            name,
            matcher,
            preview_command,
            run_command,
            selected_entries: HashSet::with_hasher(FxBuildHasher),
        }
    }
}

fn entries_from_shell_process(command: String, injector: &Injector<String>) {
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

fn entries_from_stdin(injector: &Injector<String>) {
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

/// Load the cable configuration from the config directory.
///
/// Cable is loaded by compiling all files that match the following
/// pattern in the config directory: `*channels.toml`.
///
/// # Example:
/// ```
///   config_folder/
///   ├── cable_channels.toml
///   ├── my_channels.toml
///   └── windows_channels.toml
/// ```
pub fn load_channels(hide_defaults: bool) -> Result<ChannelConfigs> {
    /// Just a proxy struct to deserialize prototypes
    #[derive(Debug, serde::Deserialize, Default)]
    struct ChannelConfigs {
        #[serde(rename = "channel")]
        channels: Vec<ChannelConfig>,
    }

    //
    // Read Config directory
    let mut channels = std::fs::read_dir(get_config_dir())?
        //
        // Get all files
        .filter_map(|f| f.ok().map(|f| f.path()))
        //
        // Check file format
        .filter(|p| is_cable_file_format(p) && p.is_file())
        //
        // Read file to toml
        .flat_map(|path| {
            let r: Result<ChannelConfigs, _> = toml::from_str(
                &std::fs::read_to_string(path).expect("Unable to read configuration file"),
            );

            // Output the error
            match &r {
                Err(e) => error!("failed to read config: {e:?}"),
                Ok(_) => debug!("found able channel files: {:?}", r),
            }

            r.unwrap_or_default().channels
        })
        .map(|prototype| (prototype.name.clone(), prototype))
        .collect::<IndexMap<_, _>>();

    if !hide_defaults {
        // Load defaults
        for channel in toml::from_str::<ChannelConfigs>(DEFAULT_CABLE_CHANNELS)?.channels {
            if !channels.contains_key(&channel.name) {
                channels.insert(channel.name.clone(), channel);
            }
        }
    }

    Ok(channels)
}

fn is_cable_file_format<P>(p: P) -> bool
where
    P: AsRef<std::path::Path>,
{
    let p = p.as_ref();
    p.file_stem()
        .and_then(|s| s.to_str())
        .map_or(false, |s| s.ends_with(CABLE_FILE_NAME_SUFFIX))
        && p.extension()
            .and_then(|e| e.to_str())
            .map_or(false, |e| e.to_lowercase() == CABLE_FILE_FORMAT)
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

    fn selected_entries(&self) -> &HashSet<Entry> {
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
