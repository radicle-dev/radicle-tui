# radicle-tui

![alt text](demo.gif "Demo")

`radicle-tui` provides various terminal user interfaces for interacting with the [Radicle](https://radicle.xyz) code forge and exposes the application framework they were built with.

# Table of Contents
1. [Getting Started](#getting-started)
    - [Installation](#installation)
    - [Usage](#usage)
2. [Application framework](#framework)
    - [Example](#example)
3. [Roadmap](#roadmap)
4. [Acknowledgments](#acknowledgments)
5. [License](#license)

## Getting started

This crate provides a single binary called `rad-tui` which contains all user interfaces. Specific interfaces can be run by the appropriate command, e.g. `rad-tui patch select` shows a patch selector.

The interfaces are designed to be modular and to integrate well with the existing Radicle CLI. Right now, they are meant to be called from other programs that will collect and process their output.

### Installation

**Requirements**

- _Linux_ or _Unix_ based operating system.
- Git 2.34 or later
- OpenSSH 9.1 or later with `ssh-agent`

#### From source

> **Note**: Requires the Rust toolchain.

You can install the binary from source, by running the following
commands from inside this repository:

```
cargo install --path . --force --locked
```

Or directly from our seed node:

```
cargo install --force --locked --git https://seed.radicle.xyz/z39mP9rQAaGmERfUMPULfPUi473tY.git
```

This will install `rad-tui`. All available commands can be shown by running `rad-tui --help`.

### Usage

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

All interfaces return a JSON object that reflects the choices made by the user, e.g.: 

```
{ "operation": "show", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }
```

## Application framework

The library portion of this crate is a framework that is the foundation for all `radicle-tui` binaries. The framework is built on top of [ratatui](https://ratatui.rs) and mostly follows the Flux application pattern. It took some ideas from [tui-realm](https://github.com/veeso/tui-realm) and [cursive](https://github.com/gyscos/cursive). The concurrency model was mostly inspired by [rust-chat-server](https://github.com/Yengas/rust-chat-server).

> **Note**: Core functionalities are considered to be stable, but the API may still change at any point. New features like configurations, used-defined keybindings, themes etc. will be added soon though.

The framework comes with widget library that provides low-level widgets such as lists, text fields etc. as well as higher-level application widgets such as windows, pages and shortcuts.

> **Note:** The widget library is under heavy development and still missing most low-level widgets. These will be added as they are needed by the `radicle-tui` binaries.

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

/// Centralized application state
#[derive(Clone, Debug)]
struct State {
    hello: String,
}

/// All messages known by the application
enum Message {
    Quit,
}

/// Implementation of the app-state trait. It's updated whenever a message was received.
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

## License

`radicle-tui` is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
