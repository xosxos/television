use std::fmt::Debug;
use std::fs::File;
use std::io::Read;
use std::num::NonZeroUsize;
use std::path::Path;
use std::process::Command;
use std::sync::LazyLock;

use color_eyre::Result;
use rustc_hash::FxHashSet as HashSet;
use tracing::{debug, warn};

use crate::strings::{proportion_of_printable_ascii_characters, PRINTABLE_ASCII_THRESHOLD};

pub struct AppMetadata {
    pub version: String,
    pub current_directory: String,
}

impl AppMetadata {
    pub fn new(version: String, current_directory: String) -> Self {
        Self {
            version,
            current_directory,
        }
    }
}

#[cfg(not(windows))]
pub fn shell_command() -> Command {
    let mut cmd = Command::new("sh");

    cmd.arg("-c");

    cmd
}

#[cfg(windows)]
pub fn shell_command() -> Command {
    let mut cmd = Command::new("cmd");

    cmd.arg("/c");

    cmd
}

#[derive(Debug, Clone, Copy, PartialEq, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
}

const COMPLETION_ZSH: &str = include_str!("../shell/completion.zsh");
const COMPLETION_BASH: &str = include_str!("../shell/completion.bash");
const COMPLETION_FISH: &str = include_str!("../shell/completion.fish");

pub fn completion_script(shell: Shell) -> Result<&'static str> {
    match shell {
        Shell::Bash => Ok(COMPLETION_BASH),
        Shell::Zsh => Ok(COMPLETION_ZSH),
        Shell::Fish => Ok(COMPLETION_FISH),
        _ => color_eyre::eyre::bail!("This shell is not yet supported: {:?}", shell),
    }
}

pub fn default_num_threads() -> NonZeroUsize {
    // default to 1 thread if we can't determine the number of available threads
    let default = NonZeroUsize::MIN;
    // never use more than 32 threads to avoid startup overhead
    let limit = NonZeroUsize::new(32).unwrap();

    std::thread::available_parallelism()
        .unwrap_or(default)
        .min(limit)
}

/// This is used to determine if we should use the stdin channel.
pub fn is_readable_stdin() -> bool {
    use std::io::IsTerminal;

    #[cfg(unix)]
    fn imp() -> bool {
        use std::{
            fs::File,
            os::{fd::AsFd, unix::fs::FileTypeExt},
        };

        let stdin = std::io::stdin();
        let Ok(fd) = stdin.as_fd().try_clone_to_owned() else {
            return false;
        };
        let file = File::from(fd);
        let Ok(md) = file.metadata() else {
            return false;
        };
        let ft = md.file_type();
        let is_file = ft.is_file();
        let is_fifo = ft.is_fifo();
        let is_socket = ft.is_socket();
        is_file || is_fifo || is_socket
    }

    #[cfg(windows)]
    fn imp() -> bool {
        let stdin = winapi_util::HandleRef::stdin();
        let typ = match winapi_util::file::typ(stdin) {
            Ok(typ) => typ,
            Err(err) => {
                debug!(
                    "for heuristic stdin detection on Windows, \
                     could not get file type of stdin \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let is_disk = typ.is_disk();
        let is_pipe = typ.is_pipe();
        let is_readable = is_disk || is_pipe;
        debug!(
            "for heuristic stdin detection on Windows, \
             found that is_disk={is_disk} and is_pipe={is_pipe}, \
             and thus concluded that is_stdin_readable={is_readable}",
        );
        is_readable
    }

    #[cfg(not(any(unix, windows)))]
    fn imp() -> bool {
        debug!("on non-{{Unix,Windows}}, assuming stdin is not readable");
        false
    }

    !std::io::stdin().is_terminal() && imp()
}

pub fn sep_name_and_value_indices(
    indices: &mut Vec<u32>,
    name_len: u32,
) -> (Vec<u32>, Vec<u32>, bool, bool) {
    let mut name_indices = Vec::new();
    let mut value_indices = Vec::new();
    let mut should_add_name_indices = false;
    let mut should_add_value_indices = false;

    for i in indices.drain(..) {
        if i < name_len {
            name_indices.push(i);
            should_add_name_indices = true;
        } else {
            value_indices.push(i - name_len);
            should_add_value_indices = true;
        }
    }

    name_indices.sort_unstable();
    name_indices.dedup();
    value_indices.sort_unstable();
    value_indices.dedup();

    (
        name_indices,
        value_indices,
        should_add_name_indices,
        should_add_value_indices,
    )
}

pub fn get_file_size(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}

#[derive(Debug)]
pub enum FileType {
    Text,
    Other,
    Unknown,
}

impl<P> From<P> for FileType
where
    P: AsRef<Path> + Debug,
{
    fn from(path: P) -> Self {
        debug!("Getting file type for {:?}", path);
        let p = path.as_ref();
        if is_known_text_extension(p) {
            return FileType::Text;
        }
        if let Ok(mut f) = File::open(p) {
            let mut buffer = [0u8; 256];
            if let Ok(bytes_read) = f.read(&mut buffer) {
                if bytes_read > 0
                    && proportion_of_printable_ascii_characters(&buffer[..bytes_read])
                        > PRINTABLE_ASCII_THRESHOLD
                {
                    return FileType::Text;
                }
            }
        } else {
            warn!("Error opening file: {:?}", path);
        }
        FileType::Other
    }
}

pub fn is_known_text_extension<P>(path: P) -> bool
where
    P: AsRef<Path>,
{
    path.as_ref()
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| KNOWN_TEXT_FILE_EXTENSIONS.contains(ext))
}

static KNOWN_TEXT_FILE_EXTENSIONS: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    [
        "ada",
        "adb",
        "ads",
        "applescript",
        "as",
        "asc",
        "ascii",
        "ascx",
        "asm",
        "asmx",
        "asp",
        "aspx",
        "atom",
        "au3",
        "awk",
        "bas",
        "bash",
        "bashrc",
        "bat",
        "bbcolors",
        "bcp",
        "bdsgroup",
        "bdsproj",
        "bib",
        "bowerrc",
        "c",
        "cbl",
        "cc",
        "cfc",
        "cfg",
        "cfm",
        "cfml",
        "cgi",
        "cjs",
        "clj",
        "cljs",
        "cls",
        "cmake",
        "cmd",
        "cnf",
        "cob",
        "code-snippets",
        "coffee",
        "coffeekup",
        "conf",
        "cp",
        "cpp",
        "cpt",
        "cpy",
        "crt",
        "cs",
        "csh",
        "cson",
        "csproj",
        "csr",
        "css",
        "csslintrc",
        "csv",
        "ctl",
        "curlrc",
        "cxx",
        "d",
        "dart",
        "dfm",
        "diff",
        "dof",
        "dpk",
        "dpr",
        "dproj",
        "dtd",
        "eco",
        "editorconfig",
        "ejs",
        "el",
        "elm",
        "emacs",
        "eml",
        "ent",
        "erb",
        "erl",
        "eslintignore",
        "eslintrc",
        "ex",
        "exs",
        "f",
        "f03",
        "f77",
        "f90",
        "f95",
        "fish",
        "for",
        "fpp",
        "frm",
        "fs",
        "fsproj",
        "fsx",
        "ftn",
        "gemrc",
        "gemspec",
        "gitattributes",
        "gitconfig",
        "gitignore",
        "gitkeep",
        "gitmodules",
        "go",
        "gpp",
        "gradle",
        "graphql",
        "groovy",
        "groupproj",
        "grunit",
        "gtmpl",
        "gvimrc",
        "h",
        "haml",
        "hbs",
        "hgignore",
        "hh",
        "hpp",
        "hrl",
        "hs",
        "hta",
        "htaccess",
        "htc",
        "htm",
        "html",
        "htpasswd",
        "hxx",
        "iced",
        "iml",
        "inc",
        "inf",
        "info",
        "ini",
        "ino",
        "int",
        "irbrc",
        "itcl",
        "itermcolors",
        "itk",
        "jade",
        "java",
        "jhtm",
        "jhtml",
        "js",
        "jscsrc",
        "jshintignore",
        "jshintrc",
        "json",
        "json5",
        "jsonld",
        "jsp",
        "jspx",
        "jsx",
        "ksh",
        "less",
        "lhs",
        "lisp",
        "log",
        "ls",
        "lsp",
        "lua",
        "m",
        "m4",
        "mak",
        "map",
        "markdown",
        "master",
        "md",
        "mdown",
        "mdwn",
        "mdx",
        "metadata",
        "mht",
        "mhtml",
        "mjs",
        "mk",
        "mkd",
        "mkdn",
        "mkdown",
        "ml",
        "mli",
        "mm",
        "mxml",
        "nfm",
        "nfo",
        "noon",
        "npmignore",
        "npmrc",
        "nuspec",
        "nvmrc",
        "ops",
        "pas",
        "pasm",
        "patch",
        "pbxproj",
        "pch",
        "pem",
        "pg",
        "php",
        "php3",
        "php4",
        "php5",
        "phpt",
        "phtml",
        "pir",
        "pl",
        "pm",
        "pmc",
        "pod",
        "pot",
        "prettierrc",
        "properties",
        "props",
        "pt",
        "pug",
        "purs",
        "py",
        "pyx",
        "r",
        "rake",
        "rb",
        "rbw",
        "rc",
        "rdoc",
        "rdoc_options",
        "resx",
        "rexx",
        "rhtml",
        "rjs",
        "rlib",
        "ron",
        "rs",
        "rss",
        "rst",
        "rtf",
        "rvmrc",
        "rxml",
        "s",
        "sass",
        "scala",
        "scm",
        "scss",
        "seestyle",
        "sh",
        "shtml",
        "sln",
        "sls",
        "spec",
        "sql",
        "sqlite",
        "sqlproj",
        "srt",
        "ss",
        "sss",
        "st",
        "strings",
        "sty",
        "styl",
        "stylus",
        "sub",
        "sublime-build",
        "sublime-commands",
        "sublime-completions",
        "sublime-keymap",
        "sublime-macro",
        "sublime-menu",
        "sublime-project",
        "sublime-settings",
        "sublime-workspace",
        "sv",
        "svc",
        "svg",
        "swift",
        "t",
        "tcl",
        "tcsh",
        "terminal",
        "tex",
        "text",
        "textile",
        "tg",
        "tk",
        "tmLanguage",
        "tmpl",
        "tmTheme",
        "toml",
        "tpl",
        "ts",
        "tsv",
        "tsx",
        "tt",
        "tt2",
        "ttml",
        "twig",
        "txt",
        "v",
        "vb",
        "vbproj",
        "vbs",
        "vcproj",
        "vcxproj",
        "vh",
        "vhd",
        "vhdl",
        "vim",
        "viminfo",
        "vimrc",
        "vm",
        "vue",
        "webapp",
        "webmanifest",
        "wsc",
        "x-php",
        "xaml",
        "xht",
        "xhtml",
        "xml",
        "xs",
        "xsd",
        "xsl",
        "xslt",
        "y",
        "yaml",
        "yml",
        "zsh",
        "zshrc",
    ]
    .iter()
    .copied()
    .collect()
});
