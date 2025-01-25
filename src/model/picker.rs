use ratatui::widgets::ListState;

use crate::{input::Input, strings::EMPTY_STRING};

#[cfg(test)]
#[path = "../../unit_tests/test_picker.rs"]
mod tests;

#[derive(Debug)]
pub struct Picker {
    pub(crate) state: ListState,
    pub(crate) relative_state: ListState,
    inverted: bool,
    pub(crate) input: Input,
}

impl Default for Picker {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Picker {
    pub fn new(input: Option<String>) -> Self {
        Self {
            state: ListState::default(),
            relative_state: ListState::default(),
            inverted: false,
            input: Input::new(input.unwrap_or(EMPTY_STRING.to_string())),
        }
    }

    pub(crate) fn offset(&self) -> usize {
        self.selected()
            .unwrap_or(0)
            .saturating_sub(self.relative_selected().unwrap_or(0))
    }

    pub(crate) fn inverted(mut self) -> Self {
        self.inverted = !self.inverted;
        self
    }

    pub(crate) fn reset_selection(&mut self) {
        self.state.select(Some(0));
        self.relative_state.select(Some(0));
    }

    pub(crate) fn reset_input(&mut self) {
        self.input.reset();
    }

    pub(crate) fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub(crate) fn select(&mut self, index: Option<usize>) {
        self.state.select(index);
    }

    fn relative_selected(&self) -> Option<usize> {
        self.relative_state.selected()
    }

    pub(crate) fn relative_select(&mut self, index: Option<usize>) {
        self.relative_state.select(index);
    }

    pub(crate) fn select_next(&mut self, step: u32, total_items: usize, height: usize) {
        if self.inverted {
            for _ in 0..step {
                self._select_prev(total_items, height);
            }
        } else {
            for _ in 0..step {
                self._select_next(total_items, height);
            }
        }
    }

    pub(crate) fn select_prev(&mut self, step: u32, total_items: usize, height: usize) {
        if self.inverted {
            for _ in 0..step {
                self._select_next(total_items, height);
            }
        } else {
            for _ in 0..step {
                self._select_prev(total_items, height);
            }
        }
    }

    fn _select_next(&mut self, total_items: usize, height: usize) {
        let selected = self.selected().unwrap_or(0);
        let relative_selected = self.relative_selected().unwrap_or(0);

        self.select(Some(selected.saturating_add(1) % total_items));
        self.relative_select(Some((relative_selected + 1).min(height)));

        if self.selected().unwrap() == 0 {
            self.relative_select(Some(0));
        }
    }

    fn _select_prev(&mut self, total_items: usize, height: usize) {
        let selected = self.selected().unwrap_or(0);
        let relative_selected = self.relative_selected().unwrap_or(0);

        self.select(Some((selected + (total_items - 1)) % total_items));
        self.relative_select(Some(relative_selected.saturating_sub(1)));

        if self.selected().unwrap() == total_items - 1 {
            self.relative_select(Some(height));
        }
    }
}
