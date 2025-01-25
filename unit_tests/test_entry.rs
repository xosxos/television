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
