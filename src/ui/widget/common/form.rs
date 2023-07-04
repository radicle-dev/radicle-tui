use tui_realm_stdlib::Input;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::tui::layout::{Constraint, Direction, Rect};
use tuirealm::{AttrValue, Attribute, Frame, MockComponent, Props, State};

use crate::ui::state::FormState;
use crate::ui::theme::Theme;
use crate::ui::widget::WidgetComponent;

pub struct Form {
    // This form's fields: title, tags, assignees, description.
    inputs: Vec<Input>,
    /// State that holds the current focus etc.
    state: FormState,
}

impl Form {
    pub fn new(_theme: Theme, inputs: Vec<Input>) -> Self {
        let state = FormState::new(Some(0), inputs.len());

        Self { inputs, state }
    }
}

impl WidgetComponent for Form {
    fn view(&mut self, properties: &Props, frame: &mut Frame, area: Rect) {
        use tuirealm::props::Layout;
        // Clear and set current focus
        let focus = self.state.focus().unwrap_or(0);
        for input in &mut self.inputs {
            input.attr(Attribute::Focus, AttrValue::Flag(false));
        }
        if let Some(input) = self.inputs.get_mut(focus) {
            input.attr(Attribute::Focus, AttrValue::Flag(true));
        }

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                &self
                    .inputs
                    .iter()
                    .map(|_| Constraint::Length(3))
                    .collect::<Vec<_>>(),
            );
        let layout = properties
            .get_or(Attribute::Layout, AttrValue::Layout(layout))
            .unwrap_layout();
        let layout = layout.chunks(area);

        for (index, area) in layout.iter().enumerate().take(self.inputs.len()) {
            if let Some(input) = self.inputs.get_mut(index) {
                input.view(frame, *area);
            }
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, cmd: Cmd) -> CmdResult {
        use tuirealm::command::Direction;
        match cmd {
            Cmd::Move(Direction::Up) => {
                self.state.focus_previous();
                CmdResult::None
            }
            Cmd::Move(Direction::Down) => {
                self.state.focus_next();
                CmdResult::None
            }
            _ => {
                let focus = self.state.focus().unwrap_or(0);
                if let Some(input) = self.inputs.get_mut(focus) {
                    input.perform(cmd)
                } else {
                    CmdResult::None
                }
            }
        }
    }
}
