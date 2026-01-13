## The application framework

The library portion of this crate is a framework that is the foundation for all `radicle-tui` binaries. It supports building concurrent applications with an immediate mode UI. It comes with a widget library that provides low-level widgets such as lists, text fields etc. as well as higher-level application widgets such as windows, pages and various other containers.

> **Note:** The framework is under heavy development and still missing some required concepts / components as well as some common low-level widgets. These will be added where needed by the `radicle-tui` binaries.

### Design

The framework was built first and foremost with developer experience in mind:

- **easy-to-use**: building new or changing existing applications should be as easy as possible; ready-made widgets should come with defaults for user interactions and rendering
- **extensibility**: extending existing and building new widgets should be straight-forward; custom application logic components should be easy to implement
- **flexibility**: widgets and application logic should be easy to change and compose; it should be all about changing and composing functions and not about writing boilerplate code

#### Components

The central pieces of the framework are the `Store`, the `Frontend` and a message passing system that let both communicate with each other. The `Store` handles the centralized application state and sends updates to the `Frontend`, whereas the `Frontend` handles user-interactions and sends messages to the `Store`, which updates the state accordingly.

The `Frontend` drives an _immediate mode_ `Ui`. In _immediate mode_, widgets are rendered the moment they're created and events are handled right before the actual drawing happens (in _retained mode_, you'd create stateful widgets once and later modify their properties or handle their events).

> **Note:** The first versions of the framework provided a retained mode UI (rmUI) which was then replaced in favor of an immediate mode UI (imUI). The Retained mode UI is still supported, but it's recommended to use the new immediate mode UI.
