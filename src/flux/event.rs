#[derive(Clone, Copy)]
pub enum Event {
    Key(termion::event::Key),
    Resize,
}