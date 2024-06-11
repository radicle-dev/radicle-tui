# radicle-tui

![Screenshot](https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z39mP9rQAaGmERfUMPULfPUi473tY/tree/demo.png "A screenshot of a terminal running rad-tui")

`radicle-tui` provides various terminal user interfaces for interacting with the [Radicle](https://radicle.xyz) code forge. It also exposes the application framework they were built with.

# Table of Contents
1. [Getting Started](#getting-started)
    - [Installation](#installation)
    - [Usage](#usage)
2. [Application framework](#application-framework)
    - [Design](#design)
    - [Example](#example)
3. [Roadmap](#roadmap)
4. [Contributing](#contributing)
5. [Contact](#contact)
6. [License](#license)

## Getting started

This crate provides a binary called `rad-tui` which contains all user interfaces. Specific interfaces can be run by the appropriate command, e.g. `rad-tui patch select` shows a patch selector.

The interfaces are designed to be modular and to integrate well with the existing Radicle CLI. Right now, the binary is meant to be called from `rad`, which will collect and process its output, e.g.

```
rad patch show
```

will show a patch selector and pass on the id of the selected patch.

> **Note:** The integration into the Radicle CLI is not fully done, yet. Please refer to the [Usage](#usage) section for information on how to use `rad-tui` already.

### Installation

**Requirements**

- _Linux_ or _Unix_ based operating system.
- Git 2.34 or later
- OpenSSH 9.1 or later with `ssh-agent`

#### From source

> **Note**: Requires the Rust toolchain.

You can install the binary from source, by running the following commands from inside this repository:

```
cargo install --path . --force --locked
```

Or directly from our seed node:

```
cargo install --force --locked --git https://seed.radicle.xyz/z39mP9rQAaGmERfUMPULfPUi473tY.git
```

This will install `rad-tui`. All available commands can be shown by running `rad-tui --help`.

### Usage

Soon, `rad-tui` will be integrated into [`heartwood`](https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z3gqcJUoA1n9HaHKufZs5FCSGazv5). Until then, you can use the `rad` proxy script that is provided. It's considered to be a drop-in replacement for `rad` and can be used for testing and prototyping purposes. It should reflect the current behavior, as if `rad-tui` would have been integrated, e.g.

```sh
# show an interface that let's you select a patch
./scripts/rad.sh patch show
```
```sh
# show an interface that let's you select a patch and an operation
./scripts/rad.sh patch --tui
```

Both commands will call into `rad-tui`, process its output and call `rad` accordingly.

#### Interfaces

Select a patch, an issue or a notification and an operation:

```
$ rad-tui <patch | issue | inbox> select
```
Same as above:

```
$ rad-tui <patch | issue | inbox> select --mode operation
```

Select a patch, an issue or a notification only and return its id:

```
$ rad-tui <patch | issue | inbox> select --mode id
```

#### Output

All interfaces return a common JSON object that reflects the choices made by the user, e.g.: 

```
{ "operation": "show", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }
```

## Application framework

The library portion of this crate is a framework that is the foundation for all `radicle-tui` binaries. The framework is built on top of [ratatui](https://ratatui.rs) and mostly follows the Flux application pattern. It took some ideas from [tui-realm](https://github.com/veeso/tui-realm) and [cursive](https://github.com/gyscos/cursive). The concurrency model was mostly inspired by [rust-chat-server](https://github.com/Yengas/rust-chat-server).

> **Note**: Existing core functionalities are considered to be stable, but the API may still change at any point. New features like configurations, used-defined keybindings, themes etc. will be added soon though.

The framework comes with a widget library that provides low-level widgets such as lists, text fields etc. as well as higher-level application widgets such as windows, pages and various other containers.

> **Note:** The widget library is under heavy development and still missing most low-level widgets. These will be added where needed by the `radicle-tui` binaries.

### Design

The framework was built with a few design goals in mind:

- **async**: state updates and IO should be asynchronous and not block the UI
- **declarative**: developers should rather think about the *What* then the *How*
- **widget library**: custom widgets should be easy to build; ready-made widgets should come with defaults for user interactions and rendering

The central pieces of the framework are the `Store`, the `Frontend` and a message passing system that let both communicate with each other. The `Store` handles the centralized application state and sends updates to the `Frontend`, whereas the `Frontend` handles user-interactions and sends messages to the `Store`, which updates the state accordingly.

On top of this, an extensible widget library was built. A widget is defined by an implementation of the `View` trait and a `Widget` it is wrapped in. A `View` handles user-interactions, updates itself whenever the application state changed and renders itself frequently. A `Widget` adds additional support for properties and event, update and render callbacks. Properties define the data, configuration etc. of a widget. They are updated by the framework taking the properties built by the `on_update` callback. The `on_event` callback is used to emit application messages whenever a widget receives an event.

The main idea is to build widgets that handle their specific events already, and that are updated with the properties built by the `on_update` callback. Custom logic is added by setting the `on_event` callback. E.g. the `Table` widget handles item selection already; items are set via the `on_update` callback and application messages are emitted via the `on_event` callback.

### Example

```rust
use anyhow::Result;

use termion::event::Key;

use ratatui::text::Text;

use radicle_tui as tui;

use tui::store;
use tui::ui::widget::text::{TextArea, TextAreaProps};
use tui::ui::widget::ToWidget;
use tui::{BoxedAny, Channel, Exit};

/// Centralized application state.
#[derive(Clone, Debug)]
struct State {
    hello: String,
}

/// All messages known by the application.
enum Message {
    Quit,
}

/// Implementation of the app-state trait. It's updated whenever a message was received.
/// Applications quit whenever an `Exit` is returned by the `update` function. The `Exit`
/// type also provides and optional return value. 
impl store::State<()> for State {
    type Message = Message;

    fn update(&mut self, message: Self::Message) -> Option<tui::Exit<()>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
        }
    }
}

/// 1. Initializes the communication channel between frontend and state store
/// 2. Initializes the application state
/// 3. Builds a textarea widget which renders a welcome message and quits the 
///    application when (q) is pressed
/// 4. Runs the TUI application
#[tokio::main]
pub async fn main() -> Result<()> {
    let channel = Channel::default();
    let sender = channel.tx.clone();
    let state = State {
        hello: "Hey there, press (q) to quit...".to_string(),
    };

    let textarea = TextArea::default()
        .to_widget(sender.clone())
        .on_event(|key, _, _| match key {
            Key::Char('q') => Some(Message::Quit),
            _ => None,
        })
        .on_update(|state: &State| {
            TextAreaProps::default()
                .text(&Text::raw(state.hello.clone()))
                .to_boxed_any()
                .into()
        });

    tui::run(channel, state, textarea).await?;

    Ok(())
}
```

## ROADMAP

The project roadmap is largely defined by the requirements of the [Radicle](https://radicle.xyz) team. If you're missing something or have any suggestions that would make this better, please feel free to [get in touch](#contact).

### Now

- [ ] Patch and issue preview in selection interfaces
- [ ] Basic `radicle-cli` integration

### Next

- [ ] Support for multiple selected list and tree items
- [ ] Read configuration from file
- [ ] Support user-defined keybindings
- [ ] Patch review

### Later   

- [ ] Streamline CLI integration w/ config and flags for `rad` commands (e.g. `rad patch edit --tui`)`
- [ ] Read COBs from JSON input
- [ ] Add support for custom themes

## Contributing

Contributions are what make the open source community such an amazing place to learn, inspire, and create. Any contributions you make are greatly appreciated.

If you have any suggestions that would make this better, please clone the repo and open a patch. You can also simply open an issue with the label "enhancement".

## Contact

Please get in touch on [Zulip](https://radicle.zulipchat.com).

## License

`radicle-tui` is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z39mP9rQAaGmERfUMPULfPUi473tY/tree/LICENSE-APACHE) and [LICENSE-MIT](https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z39mP9rQAaGmERfUMPULfPUi473tY/tree/LICENSE-MIT) for details.
