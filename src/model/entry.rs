use std::{
    hash::{Hash, Hasher},
    path::PathBuf,
};

use devicons::FileIcon;

#[cfg(test)]
#[path = "../../unit_tests/test_entry.rs"]
mod tests;

// NOTE: having an enum for entry types would be nice since it would allow
// having a nicer implementation for transitions between channels. This would
// permit implementing `From<EntryType>` for channels which would make the
// channel convertible from any other that yields `EntryType`.
// This needs pondering since it does bring another level of abstraction and
// adds a layer of complexity.
#[derive(Clone, Debug, Eq)]
pub struct Entry {
    /// The name of the entry.
    pub name: String,
    /// An optional value associated with the entry.
    pub value: Option<String>,
    /// The optional ranges for matching characters in the name.
    pub name_match_ranges: Option<Vec<(u32, u32)>>,
    /// The optional ranges for matching characters in the value.
    pub value_match_ranges: Option<Vec<(u32, u32)>>,
    /// The optional icon associated with the entry.
    pub icon: Option<FileIcon>,
    /// The optional line number associated with the entry.
    pub line_number: Option<usize>,
}

impl Hash for Entry {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        if let Some(line_number) = self.line_number {
            line_number.hash(state);
        }
    }
}

impl PartialEq<Entry> for &Entry {
    fn eq(&self, other: &Entry) -> bool {
        self.name == other.name
            && (self.line_number.is_none() && other.line_number.is_none()
                || self.line_number == other.line_number)
    }
}

impl PartialEq<Entry> for Entry {
    fn eq(&self, other: &Entry) -> bool {
        self.name == other.name
            && (self.line_number.is_none() && other.line_number.is_none()
                || self.line_number == other.line_number)
    }
}

#[allow(clippy::needless_return)]
pub fn merge_ranges(ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    ranges
        .iter()
        .fold(Vec::new(), |mut acc: Vec<(u32, u32)>, x: &(u32, u32)| {
            if let Some(last) = acc.last_mut() {
                if last.1 == x.0 {
                    last.1 = x.1;
                } else {
                    acc.push(*x);
                }
            } else {
                acc.push(*x);
            }
            return acc;
        })
}

impl Entry {
    /// Create a new entry with the given name and preview type.
    pub fn new(name: String) -> Self {
        Self {
            name,
            value: None,
            name_match_ranges: None,
            value_match_ranges: None,
            icon: None,
            line_number: None,
        }
    }

    pub fn with_value(mut self, value: String) -> Self {
        self.value = Some(value);
        self
    }

    pub fn with_name_match_ranges(mut self, name_match_ranges: &[(u32, u32)]) -> Self {
        self.name_match_ranges = Some(merge_ranges(name_match_ranges));
        self
    }

    pub fn with_value_match_ranges(mut self, value_match_ranges: &[(u32, u32)]) -> Self {
        self.value_match_ranges = Some(merge_ranges(value_match_ranges));
        self
    }

    pub fn with_icon(mut self, icon: FileIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_line_number(mut self, line_number: usize) -> Self {
        self.line_number = Some(line_number);
        self
    }

    pub fn stdout_repr(&self) -> String {
        let mut repr = self.name.clone();

        if PathBuf::from(&repr).exists() && repr.contains(|c| char::is_ascii_whitespace(&c)) {
            repr.insert(0, '\'');
            repr.push('\'');
        }

        if let Some(line_number) = self.line_number {
            repr.push_str(&format!(":{line_number}"));
        }

        repr
    }
}

pub const ENTRY_PLACEHOLDER: Entry = Entry {
    name: String::new(),
    value: None,
    name_match_ranges: None,
    value_match_ranges: None,
    icon: None,
    line_number: None,
};
