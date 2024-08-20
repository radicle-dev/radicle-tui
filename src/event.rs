#[derive(Clone, Copy, Debug)]
pub enum Event {
    Key(termion::event::Key),
    Resize,
}
