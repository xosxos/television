use super::*;

/// - item 0 S     R *
/// - item 1 next    *
/// - item 2         * height
/// - item 3
#[test]
fn test_picker_select_next_default() {
    let mut picker = Picker::default();
    picker.select(Some(0));
    picker.relative_select(Some(0));
    picker.select_next(1, 4, 2);
    assert_eq!(picker.selected(), Some(1), "selected");
    assert_eq!(picker.relative_selected(), Some(1), "relative_selected");
}

/// - item 0         *
/// - item 1 S     R *
/// - item 2 next    * height
/// - item 3
#[test]
fn test_picker_select_next_before_relative_last() {
    let mut picker = Picker::default();
    picker.select(Some(1));
    picker.relative_select(Some(1));
    picker.select_next(1, 4, 2);
    assert_eq!(picker.selected(), Some(2), "selected");
    assert_eq!(picker.relative_selected(), Some(2), "relative_selected");
}

/// - item 0         *
/// - item 1         *
/// - item 2 S     R * height
/// - item 3 next
#[test]
fn test_picker_select_next_relative_last() {
    let mut picker = Picker::default();
    picker.select(Some(2));
    picker.relative_select(Some(2));
    picker.select_next(1, 4, 2);
    assert_eq!(picker.selected(), Some(3), "selected");
    assert_eq!(picker.relative_selected(), Some(2), "relative_selected");
}

/// - item 0 next    *
/// - item 1         *
/// - item 2       R * height
/// - item 3 S
#[test]
fn test_picker_select_next_last() {
    let mut picker = Picker::default();
    picker.select(Some(3));
    picker.relative_select(Some(2));
    picker.select_next(1, 4, 2);
    assert_eq!(picker.selected(), Some(0), "selected");
    assert_eq!(picker.relative_selected(), Some(0), "relative_selected");
}

/// - item 0 next   *
/// - item 1        *
/// - item 2 S    R *
///                 * height
#[test]
fn test_picker_select_next_less_items_than_height_last() {
    let mut picker = Picker::default();
    picker.select(Some(2));
    picker.relative_select(Some(2));
    picker.select_next(1, 3, 2);
    assert_eq!(picker.selected(), Some(0), "selected");
    assert_eq!(picker.relative_selected(), Some(0), "relative_selected");
}

/// - item 0 prev    *
/// - item 1 S     R *
/// - item 2         * height
/// - item 3
#[test]
fn test_picker_select_prev_default() {
    let mut picker = Picker::default();
    picker.select(Some(1));
    picker.relative_select(Some(1));
    picker.select_prev(1, 4, 2);
    assert_eq!(picker.selected(), Some(0), "selected");
    assert_eq!(picker.relative_selected(), Some(0), "relative_selected");
}

/// - item 0 S     R *
/// - item 1         *        *
/// - item 2         * height *
/// - item 3 prev             *
#[test]
fn test_picker_select_prev_first() {
    let mut picker = Picker::default();
    picker.select(Some(0));
    picker.relative_select(Some(0));
    picker.select_prev(1, 4, 2);
    assert_eq!(picker.selected(), Some(3), "selected");
    assert_eq!(picker.relative_selected(), Some(2), "relative_selected");
}

/// - item 0         *
/// - item 1         *
/// - item 2 prev  R * height
/// - item 3 S
#[test]
fn test_picker_select_prev_relative_trailing() {
    let mut picker = Picker::default();
    picker.select(Some(3));
    picker.relative_select(Some(2));
    picker.select_prev(1, 4, 2);
    assert_eq!(picker.selected(), Some(2), "selected");
    assert_eq!(picker.relative_selected(), Some(1), "relative_selected");
}

/// - item 0         *
/// - item 1 prev    *
/// - item 2 S     R * height
/// - item 3
#[test]
fn test_picker_select_prev_relative_sync() {
    let mut picker = Picker::default();
    picker.select(Some(2));
    picker.relative_select(Some(2));
    picker.select_prev(1, 4, 2);
    assert_eq!(picker.selected(), Some(1), "selected");
    assert_eq!(picker.relative_selected(), Some(1), "relative_selected");
}

#[test]
fn test_picker_offset_default() {
    let picker = Picker::default();
    assert_eq!(picker.offset(), 0, "offset");
}

#[test]
fn test_picker_offset_none() {
    let mut picker = Picker::default();
    picker.select(None);
    picker.relative_select(None);
    assert_eq!(picker.offset(), 0, "offset");
}

#[test]
fn test_picker_offset() {
    let mut picker = Picker::default();
    picker.select(Some(1));
    picker.relative_select(Some(2));
    assert_eq!(picker.offset(), 0, "offset");
}

#[test]
fn test_picker_inverted() {
    let mut picker = Picker::default();
    picker.select(Some(0));
    picker.relative_select(Some(0));
    picker.select_next(1, 4, 2);
    picker = picker.inverted();
    picker.select_next(1, 4, 2);
    assert!(picker.inverted, "inverted");
    assert_eq!(picker.selected(), Some(0), "selected");
    assert_eq!(picker.relative_selected(), Some(0), "relative_selected");
}
