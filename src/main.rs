use std::env;
use std::io::{stdout, BufWriter, IsTerminal, StdoutLock, Write};
use std::path::Path;
use std::process::exit;
use rustc_hash::FxHashMap as HashMap;

use clap::{Parser, Subcommand};
use color_eyre::{Result, eyre::eyre};

use tracing::{debug, error, info};
use utils::shell_command;

use channel::PreviewCommand;
use entry::Entry;
use previewer::COMMAND_PLACEHOLDER_REGEX;
use rayon::prelude::*;

use crate::app::App;
use crate::config::Config;
use crate::channel::{ChannelConfig, RunCommand, TransitionCommand};
use crate::utils::Shell;
use crate::utils::{completion_script, is_readable_stdin};
use crate::channel::Channel;

pub mod app;
pub mod config;
pub mod init_eyre;
pub mod init_logging;
pub mod event;
pub mod tui;
pub mod ansi;
pub mod view;
pub mod strings;
pub mod utils;
pub mod fuzzy;
pub mod model;
pub mod action;
pub mod colors;

pub use crate::model::channel;
pub use crate::model::entry;
pub use crate::model::previewer;
pub use crate::model::picker;
pub use crate::model::television;
pub use crate::model::input;
pub use crate::model::remote_control;
pub use crate::model::logger;


#[allow(clippy::unnecessary_wraps)]
fn delimiter_parser(s: &str) -> Result<String, String> {
    Ok(match s {
        "" => ":".to_string(),
        "\\t" => "\t".to_string(),
        _ => s.to_string(),
    })
}

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    /// Which channel shall we watch?
    #[arg(value_enum, default_value = "files", index = 1)]
    pub channel: String,

    /// Use a custom preview command (currently only supported by the stdin channel)
    #[arg(short, long, value_name = "STRING")]
    pub preview: Option<String>,

    /// Use a custom run command (currently only supported by the stdin channel)
    #[arg(short, long = "run", value_name = "STRING")]
    pub run_command: Option<String>,

    /// Use a custom run command (currently only supported by the stdin channel)
    #[arg(long = "transition_command", value_name = "STRING")]
    pub transition_command: Option<String>,

    /// Use a custom run command (currently only supported by the stdin channel)
    #[arg(long = "transition_channel", value_name = "STRING")]
    pub transition_channel: Option<String>,

    /// The delimiter used to extract fields from the entry to provide to the preview command
    /// (defaults to ":")
    #[arg(long, value_name = "STRING", default_value = " ", value_parser = delimiter_parser)]
    pub delimiter: String,

    /// Tick rate, i.e. number of ticks per second
    #[arg(short, long, value_name = "FLOAT")]
    pub tick_rate: Option<f64>,

    /// Frame rate, i.e. number of frames per second
    #[arg(short, long, value_name = "FLOAT")]
    pub frame_rate: Option<f64>,

    /// Disable the preview pane
    #[arg(long)]
    pub no_preview: bool,

    /// Disable the preview pane
    #[arg(long)]
    pub hide_defaults: bool,

    /// Passthrough keybindings (comma separated, e.g. "q,ctrl-w,ctrl-t") These keybindings will
    /// trigger selection of the current entry and be passed through to stdout along with the entry
    /// to be handled by the parent process.
    #[arg(long, value_name = "STRING")]
    pub passthrough_keybindings: Option<String>,

    /// Input text to pass to the channel to prefill the prompt
    #[arg(short, long, value_name = "STRING")]
    pub input: Option<String>,

    /// The working directory to start in
    #[arg(value_name = "PATH", index = 2)]
    pub working_directory: Option<String>,

    /// Try to guess the channel from the provided input prompt
    #[arg(long, value_name = "STRING")]
    pub autocomplete_prompt: Option<String>,

    #[command(subcommand)]
    pub command: Option<SubCommand>,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum SubCommand {
    /// Lists available channels
    ListChannels,
    /// Initializes shell completion ("tv init zsh")
    #[clap(name = "init")]
    InitShell {
        /// The shell for which to generate the autocompletion script
        #[arg(value_enum)]
        shell: Shell,
    },
}


#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    init_eyre::init()?;
    // color_eyre::install()?;
    // 
    init_logging::init()?;

    let args = Cli::parse();

    if let Some(command) = args.command {
        match command {
            SubCommand::ListChannels => {
                let channels = channel::load_channels(args.hide_defaults).unwrap();

                for channel in channels.values() {
                    println!("{}", channel.name);
                    
                }

                return Ok(())
            }
            SubCommand::InitShell { shell } => {
                let script = completion_script(shell)?;
                println!("{script}");

                return Ok(())
            }
        }
    }

    let channels = channel::load_channels(args.hide_defaults)?;

    let preview_command = args.preview.map(|preview| PreviewCommand::new(&preview)).unwrap_or(PreviewCommand::new("echo {}"));

    let passthrough_keybindings: Vec<String> = args
            .passthrough_keybindings
            .unwrap_or_default()
            .split(',')
            .map(std::string::ToString::to_string)
            .collect();


    // Initiate config
    let mut config = Config::new()?;

    config.ui.tick_rate =
        args.tick_rate.unwrap_or(config.ui.tick_rate);

    config.ui.frame_rate =
        args.frame_rate.unwrap_or(config.ui.frame_rate);

    if args.no_preview {
        config.ui.show_preview_panel = false;
    }

    if let Some(working_directory) = args.working_directory {
        let path = Path::new(&working_directory);

        if !path.exists() {
            error!( "Working directory \"{working_directory}\" does not exist" );
            println!( "Error: Working directory \"{working_directory}\" does not exist", );
            exit(1);
        }

        env::set_current_dir(path)?;
    }

    let channel: Channel = {
        if is_readable_stdin() {
            debug!("Using stdin channel");

            let run_command = args.run_command.map(|command|
                    vec![ RunCommand { command, exit: false, remove: vec![] }]
                ).unwrap_or(vec![]);

            let transition_command = match (args.transition_command, args.transition_channel) {
                (Some(command), Some(channel)) => vec![ TransitionCommand { command, channel } ],
                _ => vec![],
            };

            Channel::new(String::from("stdin"), None, vec![preview_command], run_command, transition_command, args.delimiter, None, false)
        } else if let Some(prompt) = args.autocomplete_prompt {
            guess_channel_from_prompt(
                &prompt,
                &config.shell_integration.commands,
                args.hide_defaults,
            )
            .inspect(|ch| debug!("Using guessed channel: {}", ch.name))?
            .into()
        } else {
            channels
                .values()
                .find(|ch| ch.name.to_lowercase() == args.channel)
                .inspect(|ch| debug!("Using {} channel", ch.name))
                .unwrap_or_else(|| panic!("Channel not found: {}", args.channel))
                .clone()
                .into()
        }
    };

    let mut app = App::new(
        channel,
        config,
        &passthrough_keybindings,
        args.input,
        channels,
    )?;

    stdout().flush()?;

    // Run the main loop waiting for the final output
    let exit_action = app.run(stdout().is_terminal()).await?;

    info!("{:?}", exit_action);

    // Write to stdout
    let stdout_handle = stdout().lock();

    let mut bufwriter = BufWriter::new(stdout_handle);

    match exit_action {
        app::ExitAction::Entries(entries) => {
            for entry in &entries {
                writeln!(bufwriter, "{}", entry.stdout_repr())?;
            }
        },
        app::ExitAction::Input(input) => {
            writeln!(bufwriter, "{input}")?;
        },
        app::ExitAction::Passthrough(entries, key) => {
            writeln!(bufwriter, "{key}")?;

            for entry in &entries {
                writeln!(bufwriter, "{}", entry.stdout_repr())?;
            }
        },
        app::ExitAction::Command(entries, cmd, delimiter) => {
            run_command(entries, &cmd, &delimiter, &bufwriter);
        },
        app::ExitAction::None => (), 
    }

    bufwriter.flush()?;

    return Ok(())

}

// If a single command, return it to the shell
// If many commands, run them as subprocesses
fn run_command(entries: Vec<Entry>, run_command: &RunCommand, delimiter: &str, _bufwriter: &BufWriter<StdoutLock<'_>> ) {
    if run_command.exit && entries.len() == 1 {
        let parts = entries[0].name.split(&delimiter).collect::<Vec<&str>>();

        let command = run_command.command.clone();
        let mut command = command.replace("{}", &entries[0].name);

        command = COMMAND_PLACEHOLDER_REGEX
            .replace_all(&command, |caps: &regex::Captures| {
                let index =
                    caps.get(1).unwrap().as_str().parse::<usize>().unwrap();
                parts[index].to_string()
            })
            .to_string();

        println!("{command}");
        return
    }

    run_commands(entries, run_command, delimiter);

}
// If a single command, return it to the shell
// If many commands, run them as subprocesses
fn run_commands(entries: Vec<Entry>, run_command: &RunCommand, delimiter: &str) {
    entries.into_par_iter().for_each(|entry| {
    // for entry in entries {
        let parts = entry.name.split(&delimiter).collect::<Vec<&str>>();

        let command = run_command.command.clone();
        let mut command = command.replace("{}", &entry.name);

        command = COMMAND_PLACEHOLDER_REGEX
            .replace_all(&command, |caps: &regex::Captures| {
                let index =
                    caps.get(1).unwrap().as_str().parse::<usize>().unwrap();
                parts[index].to_string()
            })
            .to_string();

        // let command = format!("{command}", command);

        debug!("running {command}");

        let output = shell_command()
            .arg(&command)
            .output()
            .expect("failed to execute process");


        if output.status.success() {
            let content = String::from_utf8_lossy(&output.stdout);

            // println!("{content}");
            debug!("output: {content}");
        } else {
            error!("error");
        }
        // }
    });
}


pub fn parse_channel(channel: &str, hide_defaults: bool) -> Result<ChannelConfig> {
    channel::load_channels(hide_defaults)
        .unwrap_or_default()
        .iter()
        .find(|(k, _)| k.to_lowercase() == channel)
        .map_or_else(
            || Err(eyre!("Unknown channel: {}", channel)),
            |(_, v)| Ok(v.clone()),
        )
}

/// Backtrack from the end of the prompt and try to match each word to a known command
/// if a match is found, return the corresponding channel
/// if no match is found, throw an error
///
pub fn guess_channel_from_prompt(
    prompt: &str,
    command_mapping: &HashMap<String, String>,
    hide_defaults: bool,
) -> Result<ChannelConfig> {
    debug!("Guessing channel from prompt: {}", prompt);

    // git checkout -qf
    // --- -------- --- <---------
    if prompt.trim().is_empty() {
        match command_mapping.get("") {
            Some(channel) => return parse_channel(channel, hide_defaults),
            None => return Err(eyre!("No channel found for prompt: {}", prompt)),
        }
    }

    let rev_prompt_words = prompt.split_whitespace().rev();

    let mut stack = Vec::new();

    // for each patern
    for (pattern, channel) in command_mapping {
        if pattern.trim().is_empty() {
            continue;
        }

        // push every word of the pattern onto the stack
        stack.extend(pattern.split_whitespace());

        for word in rev_prompt_words.clone() {
            // if the stack is empty, we have a match
            if stack.is_empty() {
                return parse_channel(channel, hide_defaults);
            }
            // if the word matches the top of the stack, pop it
            if stack.last() == Some(&word) {
                stack.pop();
            }
        }

        // if the stack is empty, we have a match
        if stack.is_empty() {
            return parse_channel(channel, hide_defaults);
        }
        // reset the stack
        stack.clear();
    }
    Err(eyre!("No channel found for prompt: {}", prompt))
}
