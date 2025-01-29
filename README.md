# radicle-tui

![Screenshot](https://seed.radicle.xyz/raw/rad:z39mP9rQAaGmERfUMPULfPUi473tY/4e1233bcf87f3bf2d98f2d5c3c996dff3be390fe/demo.png "Screenshot of radicle-tui")

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

This crate provides a binary called `rad-tui` which can be used as a drop-in replacement for `rad`. It maps known commands and operations to internal ones, running the corresponding interface, e.g.

```
rad-tui patch
```

runs the patch list interface and calls `rad` with the operation and id selected. Commands or operations not known to `rad-tui` will be forwarded to `rad`, e.g. the following just calls `rad node`:

```
rad-tui node
```

The default forwarding behaviour can be overridden with a flag, e.g.

```
rad-tui help --no-forward
```
runs the internal help command instead of forwarding to `rad help`.

### Using a shell alias

In order to make the CLI integration opaque, a shell alias can be used:

```
alias rad="rad-tui"
```

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

#### Nix

There is a `flake.nix` present in the repository. This means that for
development, it should be as simple as using [`direnv`](https://direnv.net/) and
having the following `.envrc` file:

```
# .envrc
use flake
```

For using the binary in a NixOS, in your `flake.nix` you can add one of the
following to the `inputs` set:

```nix
inputs = {
    # Replace <Tag> with the specific tag to build
    radicle-tui = {
        url = "git+https://seed.radicle.xyz/z39mP9rQAaGmERfUMPULfPUi473tY.git?tag=<Tag>";
    }
}
```

```nix
inputs = {
    # Replace <Commit SHA> with the specific commit to build
    rad-tui = {
        url = "git+https://seed.radicle.xyz/z39mP9rQAaGmERfUMPULfPUi473tY.git?rev=<Commit SHA>";
    }
}
```

Then in your `home.nix` you can add:

```
home.packages = [
  inputs.radicle-tui.packages.${system}.default
];
```

### Usage

#### List

Select a patch, an issue or a notification and an operation:

```
rad-tui <patch | issue | inbox>
rad-tui <patch | issue | inbox> list
```

#### CLI integration via JSON

The interfaces are designed to be modular and could also be integrated with existing CLI tooling. The binary is can be called and its output collected and processed, e.g.

```
rad-tui patch list --json
```

runs the patch list interface and return a JSON object specifying the operation and id selected:

```
{ "operation": "show", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }
```

## Application framework

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

### Example

```rust
use anyhow::Result;

use termion::event::Key;

use ratatui::{Frame, Viewport};

use radicle_tui as tui;

use tui::store;
use tui::ui::im::widget::Window;
use tui::ui::im::Show;
use tui::ui::im::{Borders, Context};
use tui::{Channel, Exit};

#[derive(Clone, Debug)]
struct App {
    hello: String,
}

#[derive(Clone, Debug)]
enum Message {
    Quit,
}

impl store::Update<Message> for App {
    type Return = ();

    fn update(&mut self, message: Message) -> Option<tui::Exit<()>> {
        match message {
            Message::Quit => Some(Exit { value: None }),
        }
    }
}

impl Show<Message> for App {
    fn show(&self, ctx: &Context<Message>, frame: &mut Frame) -> Result<()> {
        Window::default().show(ctx, |ui| {
            ui.text_view(frame, self.hello.clone(), &mut (0, 0), Some(Borders::None));

            if ui.input_global(|key| key == Key::Char('q')) {
                ui.send_message(Message::Quit);
            }
        });

        Ok(())
    }
}

#[tokio::main]
pub async fn main() -> Result<()> {
    let app = App {
        hello: "Hello World!".to_string(),
    };

    tui::im(app, Viewport::default(), Channel::default()).await?;

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

## License

`radicle-tui` is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z39mP9rQAaGmERfUMPULfPUi473tY/tree/LICENSE-APACHE) and [LICENSE-MIT](https://app.radicle.xyz/nodes/seed.radicle.xyz/rad:z39mP9rQAaGmERfUMPULfPUi473tY/tree/LICENSE-MIT) for details.

## Contact

Please get in touch on [Zulip](https://radicle.zulipchat.com).

## Acknowledgments

Parts of this project rely on or were heavily inspired by some great open source projects. So we'd like to thank:

- [ratatui](https://ratatui.rs)
- [egui](https://github.com/egui)
- [tui-realm](https://github.com/veeso/tui-realm)
- [tui-textarea](https://github.com/rhysd/tui-textarea)
- [tui-rs-tree-widget](https://github.com/EdJoPaTo/tui-rs-tree-widget)
- [rust-chat-server](https://github.com/Yengas/rust-chat-server)
