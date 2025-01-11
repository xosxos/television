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

use crate::utils::strings::{proportion_of_printable_ascii_characters, PRINTABLE_ASCII_THRESHOLD};

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

pub mod cache {

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

pub mod input {
    /// Input requests are used to change the input state.
    ///
    /// Different backends can be used to convert events into requests.
    #[allow(clippy::module_name_repetitions)]
    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
    pub enum InputRequest {
        SetCursor(usize),
        InsertChar(char),
        GoToPrevChar,
        GoToNextChar,
        GoToPrevWord,
        GoToNextWord,
        GoToStart,
        GoToEnd,
        DeletePrevChar,
        DeleteNextChar,
        DeletePrevWord,
        DeleteNextWord,
        DeleteLine,
        DeleteTillEnd,
    }

    #[derive(Debug, PartialOrd, PartialEq, Eq, Clone, Copy, Hash)]
    pub struct StateChanged {
        pub value: bool,
        pub cursor: bool,
    }

    #[allow(clippy::module_name_repetitions)]
    pub type InputResponse = Option<StateChanged>;

    /// An input buffer with cursor support.
    #[derive(Default, Debug, Clone)]
    pub struct Input {
        value: String,
        cursor: usize,
    }

    impl Input {
        /// Initialize a new instance with a given value
        /// Cursor will be set to the given value's length.
        pub fn new(value: String) -> Self {
            let len = value.chars().count();
            Self { value, cursor: len }
        }

        /// Set the value manually.
        /// Cursor will be set to the given value's length.
        pub fn with_value(mut self, value: String) -> Self {
            self.cursor = value.chars().count();
            self.value = value;
            self
        }

        /// Set the cursor manually.
        /// If the input is larger than the value length, it'll be auto adjusted.
        pub fn with_cursor(mut self, cursor: usize) -> Self {
            self.cursor = cursor.min(self.value.chars().count());
            self
        }

        // Reset the cursor and value to default
        pub fn reset(&mut self) {
            self.cursor = Default::default();
            self.value = String::default();
        }

        /// Handle request and emit response.
        #[allow(clippy::too_many_lines)]
        pub fn handle(&mut self, req: InputRequest) -> InputResponse {
            use InputRequest::{
                DeleteLine, DeleteNextChar, DeleteNextWord, DeletePrevChar, DeletePrevWord,
                DeleteTillEnd, GoToEnd, GoToNextChar, GoToNextWord, GoToPrevChar, GoToPrevWord,
                GoToStart, InsertChar, SetCursor,
            };
            match req {
                SetCursor(pos) => {
                    let pos = pos.min(self.value.chars().count());
                    if self.cursor == pos {
                        None
                    } else {
                        self.cursor = pos;
                        Some(StateChanged {
                            value: false,
                            cursor: true,
                        })
                    }
                }
                InsertChar(c) => {
                    if self.cursor == self.value.chars().count() {
                        self.value.push(c);
                    } else {
                        self.value = self
                            .value
                            .chars()
                            .take(self.cursor)
                            .chain(std::iter::once(c).chain(self.value.chars().skip(self.cursor)))
                            .collect();
                    }
                    self.cursor += 1;
                    Some(StateChanged {
                        value: true,
                        cursor: true,
                    })
                }

                DeletePrevChar => {
                    if self.cursor == 0 {
                        None
                    } else {
                        self.cursor -= 1;
                        self.value = self
                            .value
                            .chars()
                            .enumerate()
                            .filter(|(i, _)| i != &self.cursor)
                            .map(|(_, c)| c)
                            .collect();

                        Some(StateChanged {
                            value: true,
                            cursor: true,
                        })
                    }
                }

                DeleteNextChar => {
                    if self.cursor == self.value.chars().count() {
                        None
                    } else {
                        self.value = self
                            .value
                            .chars()
                            .enumerate()
                            .filter(|(i, _)| i != &self.cursor)
                            .map(|(_, c)| c)
                            .collect();
                        Some(StateChanged {
                            value: true,
                            cursor: false,
                        })
                    }
                }

                GoToPrevChar => {
                    if self.cursor == 0 {
                        None
                    } else {
                        self.cursor -= 1;
                        Some(StateChanged {
                            value: false,
                            cursor: true,
                        })
                    }
                }

                GoToPrevWord => {
                    if self.cursor == 0 {
                        None
                    } else {
                        self.cursor = self
                            .value
                            .chars()
                            .rev()
                            .skip(self.value.chars().count().max(self.cursor) - self.cursor)
                            .skip_while(|c| !c.is_alphanumeric())
                            .skip_while(|c| c.is_alphanumeric())
                            .count();
                        Some(StateChanged {
                            value: false,
                            cursor: true,
                        })
                    }
                }

                GoToNextChar => {
                    if self.cursor == self.value.chars().count() {
                        None
                    } else {
                        self.cursor += 1;
                        Some(StateChanged {
                            value: false,
                            cursor: true,
                        })
                    }
                }

                GoToNextWord => {
                    if self.cursor == self.value.chars().count() {
                        None
                    } else {
                        self.cursor = self
                            .value
                            .chars()
                            .enumerate()
                            .skip(self.cursor)
                            .skip_while(|(_, c)| c.is_alphanumeric())
                            .find(|(_, c)| c.is_alphanumeric())
                            .map(|(i, _)| i)
                            .unwrap_or_else(|| self.value.chars().count());

                        Some(StateChanged {
                            value: false,
                            cursor: true,
                        })
                    }
                }

                DeleteLine => {
                    if self.value.is_empty() {
                        None
                    } else {
                        let cursor = self.cursor;
                        self.value = String::new();
                        self.cursor = 0;
                        Some(StateChanged {
                            value: true,
                            cursor: self.cursor == cursor,
                        })
                    }
                }

                DeletePrevWord => {
                    if self.cursor == 0 {
                        None
                    } else {
                        let remaining = self.value.chars().skip(self.cursor);
                        let rev = self
                            .value
                            .chars()
                            .rev()
                            .skip(self.value.chars().count().max(self.cursor) - self.cursor)
                            .skip_while(|c| !c.is_alphanumeric())
                            .skip_while(|c| c.is_alphanumeric())
                            .collect::<Vec<char>>();
                        let rev_len = rev.len();
                        self.value = rev.into_iter().rev().chain(remaining).collect();
                        self.cursor = rev_len;
                        Some(StateChanged {
                            value: true,
                            cursor: true,
                        })
                    }
                }

                DeleteNextWord => {
                    if self.cursor == self.value.chars().count() {
                        None
                    } else {
                        self.value = self
                            .value
                            .chars()
                            .take(self.cursor)
                            .chain(
                                self.value
                                    .chars()
                                    .skip(self.cursor)
                                    .skip_while(|c| c.is_alphanumeric())
                                    .skip_while(|c| !c.is_alphanumeric()),
                            )
                            .collect();

                        Some(StateChanged {
                            value: true,
                            cursor: false,
                        })
                    }
                }

                GoToStart => {
                    if self.cursor == 0 {
                        None
                    } else {
                        self.cursor = 0;
                        Some(StateChanged {
                            value: false,
                            cursor: true,
                        })
                    }
                }

                GoToEnd => {
                    let count = self.value.chars().count();
                    if self.cursor == count {
                        None
                    } else {
                        self.cursor = count;
                        Some(StateChanged {
                            value: false,
                            cursor: true,
                        })
                    }
                }

                DeleteTillEnd => {
                    self.value = self.value.chars().take(self.cursor).collect();
                    Some(StateChanged {
                        value: true,
                        cursor: false,
                    })
                }
            }
        }

        /// Get a reference to the current value.
        pub fn value(&self) -> &str {
            self.value.as_str()
        }

        /// Get the correct cursor placement.
        pub fn cursor(&self) -> usize {
            self.cursor
        }

        /// Get the current cursor position with account for multi space characters.
        pub fn visual_cursor(&self) -> usize {
            if self.cursor == 0 {
                return 0;
            }

            // Safe, because the end index will always be within bounds
            unicode_width::UnicodeWidthStr::width(unsafe {
                self.value.get_unchecked(
                    0..self
                        .value
                        .char_indices()
                        .nth(self.cursor)
                        .map_or_else(|| self.value.len(), |(index, _)| index),
                )
            })
        }

        /// Get the scroll position with account for multi space characters.
        pub fn visual_scroll(&self, width: usize) -> usize {
            let scroll = self.visual_cursor().max(width) - width;
            let mut uscroll = 0;
            let mut chars = self.value().chars();

            while uscroll < scroll {
                match chars.next() {
                    Some(c) => {
                        uscroll += unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
                    }
                    None => break,
                }
            }
            uscroll
        }
    }

    impl From<Input> for String {
        fn from(input: Input) -> Self {
            input.value
        }
    }

    impl From<String> for Input {
        fn from(value: String) -> Self {
            Self::new(value)
        }
    }

    impl From<&str> for Input {
        fn from(value: &str) -> Self {
            Self::new(value.into())
        }
    }

    impl std::fmt::Display for Input {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            self.value.fmt(f)
        }
    }

    #[cfg(test)]
    mod tests {
        const TEXT: &str = "first second, third.";

        use super::*;

        #[test]
        fn format() {
            let input: Input = TEXT.into();
            println!("{}", input);
            println!("{}", input);
        }

        #[test]
        fn set_cursor() {
            let mut input: Input = TEXT.into();

            let req = InputRequest::SetCursor(3);
            let resp = input.handle(req);

            assert_eq!(
                resp,
                Some(StateChanged {
                    value: false,
                    cursor: true,
                })
            );

            assert_eq!(input.value(), "first second, third.");
            assert_eq!(input.cursor(), 3);

            let req = InputRequest::SetCursor(30);
            let resp = input.handle(req);

            assert_eq!(input.cursor(), TEXT.chars().count());
            assert_eq!(
                resp,
                Some(StateChanged {
                    value: false,
                    cursor: true,
                })
            );

            let req = InputRequest::SetCursor(TEXT.chars().count());
            let resp = input.handle(req);

            assert_eq!(input.cursor(), TEXT.chars().count());
            assert_eq!(resp, None);
        }

        #[test]
        fn insert_char() {
            let mut input: Input = TEXT.into();

            let req = InputRequest::InsertChar('x');
            let resp = input.handle(req);

            assert_eq!(
                resp,
                Some(StateChanged {
                    value: true,
                    cursor: true,
                })
            );

            assert_eq!(input.value(), "first second, third.x");
            assert_eq!(input.cursor(), TEXT.chars().count() + 1);
            input.handle(req);
            assert_eq!(input.value(), "first second, third.xx");
            assert_eq!(input.cursor(), TEXT.chars().count() + 2);

            let mut input = input.with_cursor(3);
            input.handle(req);
            assert_eq!(input.value(), "firxst second, third.xx");
            assert_eq!(input.cursor(), 4);

            input.handle(req);
            assert_eq!(input.value(), "firxxst second, third.xx");
            assert_eq!(input.cursor(), 5);
        }

        #[test]
        fn go_to_prev_char() {
            let mut input: Input = TEXT.into();

            let req = InputRequest::GoToPrevChar;
            let resp = input.handle(req);

            assert_eq!(
                resp,
                Some(StateChanged {
                    value: false,
                    cursor: true,
                })
            );

            assert_eq!(input.value(), "first second, third.");
            assert_eq!(input.cursor(), TEXT.chars().count() - 1);

            let mut input = input.with_cursor(3);
            input.handle(req);
            assert_eq!(input.value(), "first second, third.");
            assert_eq!(input.cursor(), 2);

            input.handle(req);
            assert_eq!(input.value(), "first second, third.");
            assert_eq!(input.cursor(), 1);
        }

        #[test]
        fn remove_unicode_chars() {
            let mut input: Input = "¡test¡".into();

            let req = InputRequest::DeletePrevChar;
            let resp = input.handle(req);

            assert_eq!(
                resp,
                Some(StateChanged {
                    value: true,
                    cursor: true,
                })
            );

            assert_eq!(input.value(), "¡test");
            assert_eq!(input.cursor(), 5);

            input.handle(InputRequest::GoToStart);

            let req = InputRequest::DeleteNextChar;
            let resp = input.handle(req);

            assert_eq!(
                resp,
                Some(StateChanged {
                    value: true,
                    cursor: false,
                })
            );

            assert_eq!(input.value(), "test");
            assert_eq!(input.cursor(), 0);
        }

        #[test]
        fn insert_unicode_chars() {
            let mut input = Input::from("¡test¡").with_cursor(5);

            let req = InputRequest::InsertChar('☆');
            let resp = input.handle(req);

            assert_eq!(
                resp,
                Some(StateChanged {
                    value: true,
                    cursor: true,
                })
            );

            assert_eq!(input.value(), "¡test☆¡");
            assert_eq!(input.cursor(), 6);

            input.handle(InputRequest::GoToStart);
            input.handle(InputRequest::GoToNextChar);

            let req = InputRequest::InsertChar('☆');
            let resp = input.handle(req);

            assert_eq!(
                resp,
                Some(StateChanged {
                    value: true,
                    cursor: true,
                })
            );

            assert_eq!(input.value(), "¡☆test☆¡");
            assert_eq!(input.cursor(), 2);
        }

        #[test]
        fn multispace_characters() {
            let input: Input = "Ｈｅｌｌｏ, ｗｏｒｌｄ!".into();
            assert_eq!(input.cursor(), 13);
            assert_eq!(input.visual_cursor(), 23);
            assert_eq!(input.visual_scroll(6), 18);
        }
    }
}

pub mod strings {
    use std::sync::LazyLock;

    /// Returns the index of the next character boundary in the given string.
    ///
    /// If the given index is already a character boundary, it is returned as is.
    /// If the given index is out of bounds, the length of the string is returned.
    ///
    pub fn next_char_boundary(s: &str, start: usize) -> usize {
        let mut i = start;
        let len = s.len();
        if i >= len {
            return len;
        }
        while !s.is_char_boundary(i) && i < len {
            i += 1;
        }
        i
    }

    /// Returns the index of the previous character boundary in the given string.
    ///
    /// If the given index is already a character boundary, it is returned as is.
    /// If the given index is out of bounds, 0 is returned.
    ///
    pub fn prev_char_boundary(s: &str, start: usize) -> usize {
        let mut i = start;
        while !s.is_char_boundary(i) && i > 0 {
            i -= 1;
        }
        i
    }

    /// Returns a slice of the given string that starts and ends at character boundaries.
    ///
    /// If the given start index is greater than the end index, or if either index is out of bounds,
    /// an empty string is returned.
    ///
    pub fn slice_at_char_boundaries(
        s: &str,
        start_byte_index: usize,
        end_byte_index: usize,
    ) -> &str {
        if start_byte_index > end_byte_index
            || start_byte_index > s.len()
            || end_byte_index > s.len()
        {
            return EMPTY_STRING;
        }
        &s[prev_char_boundary(s, start_byte_index)..next_char_boundary(s, end_byte_index)]
    }

    /// Returns a slice of the given string that starts at the beginning and ends at a character
    /// boundary.
    ///
    /// If the given index is out of bounds, the whole string is returned.
    /// If the given index is already a character boundary, the string up to that index is returned.
    ///
    pub fn slice_up_to_char_boundary(s: &str, byte_index: usize) -> &str {
        &s[..next_char_boundary(s, byte_index)]
    }

    /// Attempts to parse a UTF-8 character from the given byte slice.
    ///
    /// The function returns the parsed character and the number of bytes consumed.
    ///
    pub fn try_parse_utf8_char(input: &[u8]) -> Option<(char, usize)> {
        let str_from_utf8 = |seq| std::str::from_utf8(seq).ok();

        let decoded = input
            .get(0..1)
            .and_then(str_from_utf8)
            .map(|c| (c, 1))
            .or_else(|| input.get(0..2).and_then(str_from_utf8).map(|c| (c, 2)))
            .or_else(|| input.get(0..3).and_then(str_from_utf8).map(|c| (c, 3)))
            .or_else(|| input.get(0..4).and_then(str_from_utf8).map(|c| (c, 4)));

        decoded.map(|(seq, n)| (seq.chars().next().unwrap(), n))
    }

    static NULL_SYMBOL: LazyLock<char> = LazyLock::new(|| char::from_u32(0x2400).unwrap());

    pub const EMPTY_STRING: &str = "";
    pub const TAB_WIDTH: usize = 4;

    const TAB_CHARACTER: char = '\t';
    const LINE_FEED_CHARACTER: char = '\x0A';
    const DELETE_CHARACTER: char = '\x7F';
    const BOM_CHARACTER: char = '\u{FEFF}';
    const NULL_CHARACTER: char = '\x00';
    const UNIT_SEPARATOR_CHARACTER: char = '\u{001F}';
    const APPLICATION_PROGRAM_COMMAND_CHARACTER: char = '\u{009F}';

    pub struct ReplaceNonPrintableConfig {
        pub replace_tab: bool,
        pub tab_width: usize,
        pub replace_line_feed: bool,
        pub replace_control_characters: bool,
    }

    impl ReplaceNonPrintableConfig {
        pub fn tab_width(&mut self, tab_width: usize) -> &mut Self {
            self.tab_width = tab_width;
            self
        }
    }

    impl Default for ReplaceNonPrintableConfig {
        fn default() -> Self {
            Self {
                replace_tab: true,
                tab_width: TAB_WIDTH,
                replace_line_feed: true,
                replace_control_characters: true,
            }
        }
    }

    #[allow(clippy::missing_panics_doc)]
    /// Replaces non-printable characters in the given byte slice with default printable characters.
    ///
    /// The tab width is used to determine how many spaces to replace a tab character with.
    /// The default printable character for non-printable characters is the Unicode symbol for NULL.
    ///
    /// The function returns a tuple containing the processed string and a vector of offsets introduced
    /// by the transformation.
    ///
    pub fn replace_non_printable(
        input: &[u8],
        config: &ReplaceNonPrintableConfig,
    ) -> (String, Vec<i16>) {
        let mut output = String::new();
        let mut offsets = Vec::new();
        let mut cumulative_offset: i16 = 0;

        let mut idx = 0;
        let len = input.len();
        while idx < len {
            offsets.push(cumulative_offset);
            if let Some((chr, skip_ahead)) = try_parse_utf8_char(&input[idx..]) {
                idx += skip_ahead;

                match chr {
                    // tab
                    TAB_CHARACTER if config.replace_tab => {
                        output.push_str(&" ".repeat(config.tab_width));
                        cumulative_offset += i16::try_from(config.tab_width).unwrap() - 1;
                    }
                    // line feed
                    LINE_FEED_CHARACTER if config.replace_line_feed => {
                        cumulative_offset -= 1;
                    }

                    // ASCII control characters from 0x00 to 0x1F
                    // + control characters from \u{007F} to \u{009F}
                    // + BOM
                    NULL_CHARACTER..=UNIT_SEPARATOR_CHARACTER
                    | DELETE_CHARACTER..=APPLICATION_PROGRAM_COMMAND_CHARACTER
                    | BOM_CHARACTER
                        if config.replace_control_characters =>
                    {
                        output.push(*NULL_SYMBOL);
                    }
                    // CJK Unified Ideographs
                    c if ('\u{4E00}'..='\u{9FFF}').contains(&c) => {
                        output.push(c);
                    }
                    // Unicode characters above 0x0700 seem unstable with ratatui
                    c if c > '\u{0700}' => {
                        output.push(*NULL_SYMBOL);
                    }
                    // everything else
                    c => output.push(c),
                }
            } else {
                output.push(*NULL_SYMBOL);
                idx += 1;
            }
        }

        (output, offsets)
    }

    /// The threshold for considering a buffer to be printable ASCII.
    ///
    /// This is used to determine whether a file is likely to be a text file
    /// based on a sample of its contents.
    pub const PRINTABLE_ASCII_THRESHOLD: f32 = 0.7;

    /// Returns the proportion of printable ASCII characters in the given buffer.
    ///
    /// This really is a cheap way to determine if a buffer is likely to be a text file.
    ///
    pub fn proportion_of_printable_ascii_characters(buffer: &[u8]) -> f32 {
        let mut printable: usize = 0;
        for &byte in buffer {
            if (32..127).contains(&byte) {
                printable += 1;
            }
        }
        printable as f32 / buffer.len() as f32
    }

    const MAX_LINE_LENGTH: usize = 300;

    /// Preprocesses a line of text for display.
    ///
    /// This function trims the line, replaces non-printable characters, and truncates the line if it
    /// is too long.
    ///
    pub fn preprocess_line(line: &str) -> (String, Vec<i16>) {
        replace_non_printable(
            {
                if line.len() > MAX_LINE_LENGTH {
                    slice_up_to_char_boundary(line, MAX_LINE_LENGTH)
                } else {
                    line
                }
            }
            .as_bytes(),
            &ReplaceNonPrintableConfig::default(),
        )
    }

    /// Make a matched string printable while preserving match ranges in the process.
    ///
    /// This function preprocesses the matched string and returns a printable version of it along with
    /// the match ranges adjusted to the new string.
    ///
    pub fn make_matched_string_printable(
        matched_string: &str,
        match_ranges: Option<&[(u32, u32)]>,
    ) -> (String, Vec<(u32, u32)>) {
        let (printable, transformation_offsets) = preprocess_line(matched_string);
        let mut match_indices = Vec::new();

        if let Some(ranges) = match_ranges {
            for (start, end) in ranges.iter().take_while(|(start, _)| {
                *start < u32::try_from(transformation_offsets.len()).unwrap()
            }) {
                let new_start =
                    i64::from(*start) + i64::from(transformation_offsets[*start as usize]);
                let new_end = i64::from(*end)
                    + i64::from(
                        // Use the last offset if the end index is out of bounds
                        // (this will be the case when the match range includes the last character)
                        transformation_offsets
                            [(*end as usize).min(transformation_offsets.len() - 1)],
                    );
                match_indices.push((
                    u32::try_from(new_start).unwrap(),
                    u32::try_from(new_end).unwrap(),
                ));
            }
        }

        (printable, match_indices)
    }

    /// Shrink a string to a maximum length, adding an ellipsis in the middle.
    ///
    /// If the string is shorter than the maximum length, it is returned as is.
    /// If the string is longer than the maximum length, it is shortened and an ellipsis is added in
    /// the middle.
    ///
    pub fn shrink_with_ellipsis(s: &str, max_length: usize) -> String {
        if s.len() <= max_length {
            return s.to_string();
        }

        let half_max_length = (max_length / 2).saturating_sub(2);
        let first_half = slice_up_to_char_boundary(s, half_max_length);
        let second_half = slice_at_char_boundaries(s, s.len() - half_max_length, s.len());
        format!("{first_half}…{second_half}")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn test_next_char_boundary(input: &str, start: usize, expected: usize) {
            let actual = next_char_boundary(input, start);
            assert_eq!(actual, expected);
        }

        #[test]
        fn test_next_char_boundary_ascii() {
            test_next_char_boundary("Hello, World!", 0, 0);
            test_next_char_boundary("Hello, World!", 1, 1);
            test_next_char_boundary("Hello, World!", 13, 13);
            test_next_char_boundary("Hello, World!", 30, 13);
        }

        #[test]
        fn test_next_char_boundary_emoji() {
            test_next_char_boundary("👋🌍!", 0, 0);
            test_next_char_boundary("👋🌍!", 1, 4);
            test_next_char_boundary("👋🌍!", 4, 4);
            test_next_char_boundary("👋🌍!", 8, 8);
            test_next_char_boundary("👋🌍!", 7, 8);
        }

        fn test_previous_char_boundary(input: &str, start: usize, expected: usize) {
            let actual = prev_char_boundary(input, start);
            assert_eq!(actual, expected);
        }

        #[test]
        fn test_previous_char_boundary_ascii() {
            test_previous_char_boundary("Hello, World!", 0, 0);
            test_previous_char_boundary("Hello, World!", 1, 1);
            test_previous_char_boundary("Hello, World!", 5, 5);
        }

        #[test]
        fn test_previous_char_boundary_emoji() {
            test_previous_char_boundary("👋🌍!", 0, 0);
            test_previous_char_boundary("👋🌍!", 4, 4);
            test_previous_char_boundary("👋🌍!", 6, 4);
            test_previous_char_boundary("👋🌍!", 8, 8);
        }

        fn test_slice_at_char_boundaries(input: &str, start: usize, end: usize, expected: &str) {
            let actual = slice_at_char_boundaries(input, start, end);
            assert_eq!(actual, expected);
        }

        #[test]
        fn test_slice_at_char_boundaries_ascii() {
            test_slice_at_char_boundaries("Hello, World!", 0, 0, "");
            test_slice_at_char_boundaries("Hello, World!", 0, 1, "H");
            test_slice_at_char_boundaries("Hello, World!", 0, 13, "Hello, World!");
            test_slice_at_char_boundaries("Hello, World!", 0, 30, "");
        }

        #[test]
        fn test_slice_at_char_boundaries_emoji() {
            test_slice_at_char_boundaries("👋🌍!", 0, 0, "");
            test_slice_at_char_boundaries("👋🌍!", 0, 4, "👋");
            test_slice_at_char_boundaries("👋🌍!", 0, 8, "👋🌍");
            test_slice_at_char_boundaries("👋🌍!", 0, 7, "👋🌍");
            test_slice_at_char_boundaries("👋🌍!", 0, 9, "👋🌍!");
        }

        fn test_replace_non_printable(input: &str, expected: &str) {
            let (actual, _offset) = replace_non_printable(
                input.as_bytes(),
                ReplaceNonPrintableConfig::default().tab_width(2),
            );
            assert_eq!(actual, expected);
        }

        #[test]
        fn test_replace_non_printable_ascii() {
            test_replace_non_printable("Hello, World!", "Hello, World!");
        }

        #[test]
        fn test_replace_non_printable_tab() {
            test_replace_non_printable("Hello\tWorld!", "Hello  World!");
            test_replace_non_printable(
                "	-- AND
", "  -- AND",
            );
        }

        #[test]
        fn test_replace_non_printable_line_feed() {
            test_replace_non_printable("Hello\nWorld!", "HelloWorld!");
        }

        #[test]
        fn test_replace_non_printable_null() {
            test_replace_non_printable("Hello\x00World!", "Hello␀World!");
            test_replace_non_printable("Hello World!\0", "Hello World!␀");
        }

        #[test]
        fn test_replace_non_printable_delete() {
            test_replace_non_printable("Hello\x7FWorld!", "Hello␀World!");
        }

        #[test]
        fn test_replace_non_printable_bom() {
            test_replace_non_printable("Hello\u{FEFF}World!", "Hello␀World!");
        }

        #[test]
        fn test_replace_non_printable_start_txt() {
            test_replace_non_printable("Àì", "Àì␀");
        }

        #[test]
        fn test_replace_non_printable_range_tab() {
            let input = b"Hello,\tWorld!";
            let (output, offsets) =
                replace_non_printable(input, &ReplaceNonPrintableConfig::default());
            assert_eq!(output, "Hello,    World!");
            assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, 3, 3, 3, 3, 3, 3]);
        }

        #[test]
        fn test_replace_non_printable_range_line_feed() {
            let input = b"Hello,\nWorld!";
            let (output, offsets) =
                replace_non_printable(input, ReplaceNonPrintableConfig::default().tab_width(2));
            assert_eq!(output, "Hello,World!");
            assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, -1, -1, -1, -1, -1, -1]);
        }

        #[test]
        fn test_replace_non_printable_no_range_changes() {
            let input = b"Hello,\x00World!";
            let (output, offsets) =
                replace_non_printable(input, ReplaceNonPrintableConfig::default().tab_width(2));
            assert_eq!(output, "Hello,␀World!");
            assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

            let input = b"Hello,\x7FWorld!";
            let (output, offsets) =
                replace_non_printable(input, ReplaceNonPrintableConfig::default().tab_width(2));
            assert_eq!(output, "Hello,␀World!");
            assert_eq!(offsets, vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        }

        fn test_proportion_of_printable_ascii_characters(input: &str, expected: f32) {
            let actual = proportion_of_printable_ascii_characters(input.as_bytes());
            assert_eq!(actual, expected);
        }

        #[test]
        fn test_proportion_of_printable_ascii_characters_ascii() {
            test_proportion_of_printable_ascii_characters("Hello, World!", 1.0);
            test_proportion_of_printable_ascii_characters("Hello, World!\x00", 0.928_571_4);
            test_proportion_of_printable_ascii_characters(
                "\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F",
                0.0,
            );
        }

        fn test_preprocess_line(input: &str, expected: &str) {
            let (actual, _offset) = preprocess_line(input);
            assert_eq!(actual, expected, "input: {:?}", input);
        }

        #[test]
        fn test_preprocess_line_cases() {
            test_preprocess_line("Hello, World!", "Hello, World!");
            test_preprocess_line("Hello, World!\n", "Hello, World!");
            test_preprocess_line("Hello, World!\x00", "Hello, World!␀");
            test_preprocess_line("Hello, World!\x7F", "Hello, World!␀");
            test_preprocess_line("Hello, World!\u{FEFF}", "Hello, World!␀");
            test_preprocess_line(&"a".repeat(400), &"a".repeat(300));
        }
    }
}
