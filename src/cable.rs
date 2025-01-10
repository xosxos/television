use rustc_hash::FxHashMap;

use color_eyre::Result;
use tracing::{debug, error};

use crate::channels::cable::{CableChannelPrototype, CableChannels};
use crate::config::get_config_dir;

const CABLE_FILE_NAME_SUFFIX: &str = "channels";
const CABLE_FILE_FORMAT: &str = "toml";

const DEFAULT_CABLE_CHANNELS: &str = include_str!("../config/channels.toml");

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
pub fn load_cable_channels() -> Result<CableChannels> {
    /// Just a proxy struct to deserialize prototypes
    #[derive(Debug, serde::Deserialize, Default)]
    struct ChannelPrototypes {
        #[serde(rename = "cable_channel")]
        prototypes: Vec<CableChannelPrototype>,
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
            let r: Result<ChannelPrototypes, _> = toml::from_str(
                &std::fs::read_to_string(path).expect("Unable to read configuration file"),
            );

            // Output the error
            match &r {
                Err(e) => error!("failed to read config: {e:?}"),
                Ok(_) => debug!("found able channel files: {:?}", r),
            }

            r.unwrap_or_default().prototypes
        })
        .map(|prototype| (prototype.name.clone(), prototype))
        .collect::<FxHashMap<_, _>>();

    // Load defaults
    for channel in toml::from_str::<ChannelPrototypes>(DEFAULT_CABLE_CHANNELS)?.prototypes {
        channels.insert(channel.name.clone(), channel);
    }

    Ok(CableChannels(channels))
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
