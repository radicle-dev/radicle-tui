use anyhow::anyhow;

use tuirealm::tui::widgets::{ListState, TableState};
use tuirealm::{State, StateValue};

/// State that holds the index of a selected tab item and the count of all tab items.
/// The index can be increased and will start at 0, if length was reached.
#[derive(Clone, Default)]
pub struct TabState {
    pub selected: u16,
    pub len: u16,
}

impl TabState {
    pub fn incr_tab_index(&mut self, rewind: bool) {
        if self.selected + 1 < self.len {
            self.selected += 1;
        } else if rewind {
            self.selected = 0;
        }
    }
}

#[derive(Clone)]
pub struct ItemState {
    selected: Option<usize>,
    len: usize,
}

impl ItemState {
    pub fn new(selected: Option<usize>, len: usize) -> Self {
        Self { selected, len }
    }

    pub fn selected(&self) -> Option<usize> {
        if !self.is_empty() {
            self.selected
        } else {
            None
        }
    }

    pub fn select_previous(&mut self) -> Option<usize> {
        let old_index = self.selected();
        let new_index = match old_index {
            Some(0) | None => Some(0),
            Some(selected) => Some(selected.saturating_sub(1)),
        };

        if old_index != new_index {
            self.selected = new_index;
            self.selected()
        } else {
            None
        }
    }

    pub fn select_next(&mut self) -> Option<usize> {
        let old_index = self.selected();
        let new_index = match old_index {
            Some(selected) if selected >= self.len.saturating_sub(1) => {
                Some(self.len.saturating_sub(1))
            }
            Some(selected) => Some(selected.saturating_add(1)),
            None => Some(0),
        };

        if old_index != new_index {
            self.selected = new_index;
            self.selected()
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl TryFrom<State> for ItemState {
    type Error = anyhow::Error;

    fn try_from(state: State) -> Result<Self, Self::Error> {
        match state {
            State::Tup2((StateValue::Usize(selected), StateValue::Usize(len))) => Ok(Self {
                selected: Some(selected),
                len,
            }),
            _ => Err(anyhow!(format!(
                "Cannot convert into item state: {:?}",
                state
            ))),
        }
    }
}

impl From<&ItemState> for TableState {
    fn from(value: &ItemState) -> Self {
        let mut state = TableState::default();
        state.select(value.selected);
        state
    }
}

impl From<&ItemState> for ListState {
    fn from(value: &ItemState) -> Self {
        let mut state = ListState::default();
        state.select(value.selected);
        state
    }
}

#[derive(Clone)]
pub struct FormState {
    focus: Option<usize>,
    len: usize,
}

impl FormState {
    pub fn new(focus: Option<usize>, len: usize) -> Self {
        Self { focus, len }
    }

    pub fn focus(&self) -> Option<usize> {
        self.focus
    }

    pub fn focus_previous(&mut self) -> Option<usize> {
        let old_index = self.focus();
        let new_index = match old_index {
            Some(0) | None => Some(0),
            Some(focus) => Some(focus.saturating_sub(1)),
        };

        if old_index != new_index {
            self.focus = new_index;
            self.focus()
        } else {
            None
        }
    }

    pub fn focus_next(&mut self) -> Option<usize> {
        let old_index = self.focus();
        let new_index = match old_index {
            Some(focus) if focus >= self.len.saturating_sub(1) => Some(self.len.saturating_sub(1)),
            Some(focus) => Some(focus.saturating_add(1)),
            None => Some(0),
        };

        if old_index != new_index {
            self.focus = new_index;
            self.focus()
        } else {
            None
        }
    }
}
