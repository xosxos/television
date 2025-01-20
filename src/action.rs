use serde::{Deserialize, Serialize};

/// The different actions that can be performed by the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    // input actions
    /// Add a character to the input buffer.
    #[serde(skip)]
    AddInputChar(char),
    /// Delete the character before the cursor from the input buffer.
    #[serde(skip)]
    DeletePrevChar,
    /// Delete the previous word from the input buffer.
    #[serde(skip)]
    DeletePrevWord,
    /// Delete the character after the cursor from the input buffer.
    #[serde(skip)]
    DeleteNextChar,
    /// Move the cursor to the character before the current cursor position.
    #[serde(skip)]
    GoToPrevChar,
    /// Move the cursor to the character after the current cursor position.
    #[serde(skip)]
    GoToNextChar,
    /// Move the cursor to the start of the input buffer.
    GoToInputStart,
    /// Move the cursor to the end of the input buffer.
    GoToInputEnd,
    // rendering actions
    /// Render the terminal user interface screen.
    #[serde(skip)]
    Render,
    /// Resize the terminal user interface screen to the given dimensions.
    #[serde(skip)]
    Resize(u16, u16),
    /// Clear the terminal user interface screen.
    #[serde(skip)]
    ClearScreen,
    // results actions
    /// Add entry under cursor to the list of selected entries and move the cursor down.
    ToggleSelectionDown,
    /// Add entry under cursor to the list of selected entries and move the cursor up.
    ToggleSelectionUp,
    /// Confirm current selection (multi select or entry under cursor).
    ConfirmSelection,
    /// Select the entry currently under the cursor and pass the key that was pressed
    /// through to be handled the parent process.
    SelectPassthrough(String),
    /// Select the entry currently under the cursor and exit the application.
    SelectAndExit,
    /// Select the next entry in the currently focused list.
    SelectNextEntry,
    /// Select the previous entry in the currently focused list.
    SelectPrevEntry,
    /// Select the next page of entries in the currently focused list.
    SelectNextPage,
    /// Select the previous page of entries in the currently focused list.
    SelectPrevPage,
    /// Select the next preview command.
    SelectNextPreview,
    /// Select the previous preview command.
    SelectPrevPreview,
    /// Select the previous preview command.
    SelectPreview(usize),
    /// Select the next run command.
    SelectNextRun,
    /// Select the previous run command.
    SelectPrevRun,
    /// Select the previous preview command.
    SelectRun(usize),
    /// Select the next preview command.
    SelectNextTransition,
    /// Select the previous preview command.
    SelectPrevTransition,
    /// Select the previous preview command.
    SelectTransition(usize),
    /// Select the next run command.
    /// Copy the currently selected entry to the clipboard.
    CopyEntryToClipboard,
    // preview actions
    /// Scroll the preview up by one line.
    ScrollPreviewUp,
    /// Scroll the preview down by one line.
    ScrollPreviewDown,
    /// Scroll the preview up by half a page.
    ScrollPreviewHalfPageUp,
    /// Scroll the preview down by half a page.
    ScrollPreviewHalfPageDown,
    /// Scroll the log up.
    ScrollLogUp,
    /// Scroll the log down.
    ScrollLogDown,
    /// Open the currently selected entry in the default application.
    #[serde(skip)]
    OpenEntry,
    // application actions
    /// Tick the application state.
    #[serde(skip)]
    Tick,
    /// Suspend the application.
    #[serde(skip)]
    Suspend,
    /// Resume the application.
    #[serde(skip)]
    Resume,
    /// Quit the application.
    Quit,
    /// Toggle the help bar.
    ToggleHelp,
    /// Toggle logs.
    ToggleLogs,
    /// Toggle the preview panel.
    TogglePreview,
    // channel actions
    /// Toggle the remote control channel.
    ToggleRemoteControl,
    /// Toggle the `transition` mode.
    ToggleTransition,
    /// Toggle the `preview commands` mode.
    TogglePreviewCommands,
    /// Toggle the `run commands` mode.
    ToggleRunCommands,
    /// Signal an error with the given message.
    #[serde(skip)]
    Error(String),
    /// No operation.
    #[serde(skip)]
    NoOp,
}
