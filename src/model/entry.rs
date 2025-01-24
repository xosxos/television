use std::{
    hash::{Hash, Hasher},
    path::PathBuf,
};

use devicons::FileIcon;

#[derive(Clone, Debug, Eq)]
pub struct Entry {
    /// The name of the entry.
    pub name: String,
    /// The optional ranges for matching characters in the name.
    pub name_match_ranges: Option<Vec<(u32, u32)>>,
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
            
            acc
        })
}

impl Entry {
    /// Create a new entry with the given name and preview type.
    pub fn new(name: String) -> Self {
        Self {
            name,
            name_match_ranges: None,
            icon: None,
            line_number: None,
        }
    }

    pub fn with_name_match_ranges(mut self, name_match_ranges: &[(u32, u32)]) -> Self {
        self.name_match_ranges = Some(merge_ranges(name_match_ranges));
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
    name_match_ranges: None,
    icon: None,
    line_number: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let ranges: Vec<(u32, u32)> = vec![];
        assert_eq!(merge_ranges(&ranges), Vec::<(u32, u32)>::new());
    }

    #[test]
    fn test_single_range() {
        let ranges = vec![(1, 3)];
        assert_eq!(merge_ranges(&ranges), vec![(1, 3)]);
    }

    #[test]
    fn test_contiguous_ranges() {
        let ranges = vec![(1, 2), (2, 3), (3, 4), (4, 5)];
        assert_eq!(merge_ranges(&ranges), vec![(1, 5)]);
    }

    #[test]
    fn test_non_contiguous_ranges() {
        let ranges = vec![(1, 2), (3, 4), (5, 6)];
        assert_eq!(merge_ranges(&ranges), vec![(1, 2), (3, 4), (5, 6)]);
    }
}
