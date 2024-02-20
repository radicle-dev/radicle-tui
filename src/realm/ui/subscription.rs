use tuirealm::event::{Key, KeyEvent, KeyModifiers};
use tuirealm::SubEventClause;

pub fn navigation_clause<UserEvent>() -> SubEventClause<UserEvent>
where
    UserEvent: Clone + Eq + PartialEq + PartialOrd,
{
    SubEventClause::Keyboard(KeyEvent {
        code: Key::Tab,
        modifiers: KeyModifiers::NONE,
    })
}

pub fn quit_clause<UserEvent>(key: Key) -> SubEventClause<UserEvent>
where
    UserEvent: Clone + Eq + PartialEq + PartialOrd,
{
    SubEventClause::Keyboard(KeyEvent {
        code: key,
        modifiers: KeyModifiers::NONE,
    })
}
