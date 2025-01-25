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
    let entry = Entry::new("an:entry:to:preview".to_string());
    let formatted_command = format_command(&command.command, &delimiter, &entry).unwrap();

    assert_eq!(formatted_command, "something");
}

#[test]
fn test_format_command_with_global_placeholder_only() {
    let delimiter = ":".to_string();
    let command = PreviewCommand {
        command: "something {}".to_string(),
    };
    let entry = Entry::new("an:entry:to:preview".to_string());
    let formatted_command = format_command(&command.command, &delimiter, &entry).unwrap();

    assert_eq!(formatted_command, "something an:entry:to:preview");
}

#[test]
fn test_format_command_with_positional_placeholders_only() {
    let delimiter = ":".to_string();
    let command = PreviewCommand {
        command: "something {0} -t {2}".to_string(),
    };
    let entry = Entry::new("an:entry:to:preview".to_string());
    let formatted_command = format_command(&command.command, &delimiter, &entry).unwrap();

    assert_eq!(formatted_command, "something an -t to");
}
