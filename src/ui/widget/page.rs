use tokio::sync::mpsc::UnboundedSender;

use termion::event::Key;

use super::{BaseView, BoxedWidget, Properties, RenderProps, Widget, WidgetState};

#[derive(Clone)]
pub struct SectionGroupState {
    /// Index of currently focused section.
    focus: Option<usize>,
}

impl WidgetState for SectionGroupState {}

#[derive(Clone, Default)]
pub struct SectionGroupProps {
    /// If this pages' keys should be handled.
    handle_keys: bool,
}

impl SectionGroupProps {
    pub fn handle_keys(mut self, handle_keys: bool) -> Self {
        self.handle_keys = handle_keys;
        self
    }
}

impl Properties for SectionGroupProps {}

pub struct SectionGroup<S, A> {
    /// Internal base
    base: BaseView<S, A>,
    /// Internal table properties
    props: SectionGroupProps,
    /// All sections
    sections: Vec<BoxedWidget<S, A>>,
    /// Internal selection and offset state
    state: SectionGroupState,
}

impl<S, A> SectionGroup<S, A> {
    pub fn section(mut self, section: BoxedWidget<S, A>) -> Self {
        self.sections.push(section);
        self
    }

    fn prev(&mut self) -> Option<usize> {
        let focus = self.state.focus.map(|current| current.saturating_sub(1));
        self.state.focus = focus;
        focus
    }

    fn next(&mut self, len: usize) -> Option<usize> {
        let focus = self.state.focus.map(|current| {
            if current < len.saturating_sub(1) {
                current.saturating_add(1)
            } else {
                current
            }
        });
        self.state.focus = focus;
        focus
    }
}

impl<S, A> Widget for SectionGroup<S, A> {
    type State = S;
    type Action = A;

    fn new(_state: &S, action_tx: UnboundedSender<A>) -> Self {
        Self {
            base: BaseView {
                action_tx: action_tx.clone(),
                on_update: None,
                on_event: None,
            },
            props: SectionGroupProps::default(),
            sections: vec![],
            state: SectionGroupState { focus: Some(0) },
        }
    }

    fn handle_event(&mut self, key: Key) {
        if let Some(section) = self
            .state
            .focus
            .and_then(|focus| self.sections.get_mut(focus))
        {
            section.handle_event(key);
        }

        if self.props.handle_keys {
            match key {
                Key::Left => {
                    self.prev();
                }
                Key::Right => {
                    self.next(self.sections.len());
                }
                _ => {}
            }
        }

        if let Some(on_event) = self.base.on_event {
            (on_event)(
                self.state.clone().to_boxed_any(),
                self.base.action_tx.clone(),
            );
        }
    }

    fn update(&mut self, state: &S) {
        self.props = SectionGroupProps::from_callback(self.base.on_update, state)
            .unwrap_or(self.props.clone());

        for section in &mut self.sections {
            section.update(state);
        }
    }

    fn render(&self, frame: &mut ratatui::Frame, props: RenderProps) {
        let areas = props.layout.split(props.area);

        for (index, area) in areas.iter().enumerate() {
            if let Some(section) = self.sections.get(index) {
                let focus = self
                    .state
                    .focus
                    .map(|focus_index| index == focus_index)
                    .unwrap_or_default();

                section.render(frame, RenderProps::from(*area).focus(focus));
            }
        }
    }

    fn base_mut(&mut self) -> &mut BaseView<S, A> {
        &mut self.base
    }
}
