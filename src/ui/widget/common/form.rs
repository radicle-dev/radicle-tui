use std::collections::HashMap;

use tuirealm::command::{CmdResult, Cmd};
use tuirealm::tui::layout::Rect;
use tuirealm::{MockComponent, Props, Frame, State};

use crate::ui::widget::WidgetComponent;

struct Form {
    inputs: HashMap<String, Box<dyn MockComponent>>,
    
}

impl Form {

}

impl WidgetComponent for Form {
    fn view(&mut self, _properties: &Props, _frame: &mut Frame, _area: Rect) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _properties: &Props, _cmd: Cmd) -> CmdResult {
        CmdResult::None
    }
}