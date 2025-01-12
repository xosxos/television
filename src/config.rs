#![allow(clippy::module_name_repetitions)]
use std::{env, path::PathBuf, sync::LazyLock};

use color_eyre::{eyre::Context, Result};
use directories::ProjectDirs;
use rustc_hash::FxHashMap as HashMap;
use serde::Deserialize;
use tracing::{debug, warn};

use crate::screen::{preview::PreviewTitlePosition, results::InputPosition};

use styles::Styles;
use themes::DEFAULT_THEME;

pub use keybindings::{parse_key, Binding, KeyBindings, KeyEvent};
pub use themes::Theme;

mod keybindings;
mod styles;
mod themes;

const DEFAULT_UI_SCALE: u16 = 100;
const CONFIG: &str = include_str!("../config/config.toml");
const CONFIG_FILE_NAME: &str = "config.toml";

static PROJECT_NAME: LazyLock<String> = LazyLock::new(|| String::from("television"));
static PROJECT_NAME_UPPER: LazyLock<String> = LazyLock::new(|| PROJECT_NAME.to_uppercase());
static DATA_FOLDER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    env::var_os(format!("{}_DATA", PROJECT_NAME_UPPER.clone()))
        .map(PathBuf::from)
        .or_else(|| {
            // otherwise, use the XDG data directory
            env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .map(|p| p.join(PROJECT_NAME.as_str()))
                .filter(|p| p.is_absolute())
        })
});

static CONFIG_FOLDER: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    // if `TELEVISION_CONFIG` is set, use that as the television config directory
    env::var_os(format!("{}_CONFIG", PROJECT_NAME_UPPER.clone()))
        .map(PathBuf::from)
        .or_else(|| {
            // otherwise, use the XDG config directory + 'television'
            env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .map(|p| p.join(PROJECT_NAME.as_str()))
                .filter(|p| p.is_absolute())
        })
});

#[allow(dead_code, clippy::module_name_repetitions)]
#[derive(Clone, Debug, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub data_dir: PathBuf,
    #[serde(default)]
    pub config_dir: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    #[allow(clippy::struct_field_names)]
    #[serde(default, flatten)]
    pub config: AppConfig,
    pub keybindings: KeyBindings,
    #[serde(default)]
    pub styles: Styles,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub shell_integration: ShellIntegrationConfig,
}

#[derive(Clone, Debug, Deserialize, Default)]
pub struct ShellIntegrationConfig {
    pub commands: HashMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UiConfig {
    pub use_nerd_font_icons: bool,
    pub ui_scale: u16,
    pub preview_title_position: Option<PreviewTitlePosition>,
    pub show_help_bar: bool,
    pub show_preview_panel: bool,

    #[serde(default)]
    pub show_logs: bool,

    #[serde(default)]
    pub show_remote_control: bool,

    #[serde(default)]
    pub input_bar_position: InputPosition,

    #[serde(default = "default_frame_rate")]
    pub frame_rate: f64,

    #[serde(default = "default_tick_rate")]
    pub tick_rate: f64,

    pub theme: String,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            use_nerd_font_icons: false,
            ui_scale: DEFAULT_UI_SCALE,
            show_help_bar: false,
            show_logs: false,
            show_preview_panel: true,
            show_remote_control: false,
            input_bar_position: InputPosition::Top,
            preview_title_position: None,
            theme: String::from(DEFAULT_THEME),
            tick_rate: default_tick_rate(),
            frame_rate: default_frame_rate(),
        }
    }
}

impl Config {
    // FIXME: default management is a bit of a mess right now
    #[allow(clippy::missing_panics_doc, clippy::missing_errors_doc)]
    pub fn new() -> Result<Self> {
        // Load the default_config values as base defaults
        let default_config: Config =
            toml::from_str(CONFIG).wrap_err("error parsing default config")?;

        // initialize the config builder
        let data_dir = get_data_dir();
        let config_dir = get_config_dir();

        std::fs::create_dir_all(&config_dir).expect("Failed creating configuration directory");
        std::fs::create_dir_all(&data_dir).expect("Failed creating data directory");

        if config_dir.join(CONFIG_FILE_NAME).is_file() {
            debug!("Found config file at {:?}", config_dir);

            let path = config_dir.join(CONFIG_FILE_NAME);
            let contents = std::fs::read_to_string(&path)?;

            let mut cfg: Config =
                toml::from_str(&contents).wrap_err(format!("error parsing config: {path:?}"))?;

            // for (mode, default_bindings) in default_config.keybindings.iter() {
            //     let user_bindings = cfg.keybindings.entry(*mode).or_default();
            //     for (command, key) in default_bindings {
            //         user_bindings
            //             .entry(command.clone())
            //             .or_insert_with(|| key.clone());
            //     }
            // }

            for (mode, default_styles) in default_config.styles.iter() {
                let user_styles = cfg.styles.entry(*mode).or_default();
                for (style_key, style) in default_styles {
                    user_styles.entry(style_key.clone()).or_insert(*style);
                }
            }

            debug!("Config: {:?}", cfg);
            Ok(cfg)
        } else {
            warn!("No config file found at {:?}, creating default configuration file at that location.", config_dir);
            // create the default configuration file in the user's config directory
            std::fs::write(config_dir.join(CONFIG_FILE_NAME), CONFIG)?;
            Ok(default_config)
        }
    }
}

pub fn get_data_dir() -> PathBuf {
    let directory = if let Some(s) = DATA_FOLDER.clone() {
        debug!("Using data directory: {:?}", s);
        s
    } else if let Some(proj_dirs) = project_directory() {
        debug!("Falling back to default data dir");
        proj_dirs.data_local_dir().to_path_buf()
    } else {
        PathBuf::from("../../../../..").join(".data")
    };
    directory
}

pub fn get_config_dir() -> PathBuf {
    let directory = if let Some(s) = CONFIG_FOLDER.clone() {
        debug!("Config directory: {:?}", s);
        s
    } else if cfg!(unix) {
        // default to ~/.config/television for unix systems
        if let Some(base_dirs) = directories::BaseDirs::new() {
            let cfg_dir = base_dirs.home_dir().join(".config").join("television");
            debug!("Config directory: {:?}", cfg_dir);
            cfg_dir
        } else {
            PathBuf::from("../../../../..").join(".config")
        }
    } else if let Some(proj_dirs) = project_directory() {
        debug!("Falling back to default config dir");
        proj_dirs.config_local_dir().to_path_buf()
    } else {
        PathBuf::from("../../../../..").join("../../../../../.config")
    };
    directory
}

fn project_directory() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "", env!("CARGO_PKG_NAME"))
}

fn default_frame_rate() -> f64 {
    60.0
}

fn default_tick_rate() -> f64 {
    50.0
}
