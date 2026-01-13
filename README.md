# radicle-tui

![Screenshot](https://seed.radicle.xyz/raw/rad:z39mP9rQAaGmERfUMPULfPUi473tY/4e1233bcf87f3bf2d98f2d5c3c996dff3be390fe/demo.png "Screenshot of radicle-tui")

`radicle-tui` provides various terminal user interfaces for interacting with the [Radicle](https://radicle.xyz) code forge. It also exposes the application framework they were built with.

# Table of Contents

1. [Getting Started](#Getting-started)
   - [Installation](#Installation)
   - [Usage](#Usage)
2. [Build with `radicle-tui`](#Build-with-radicle-tui)
   - [Example](#Example)
3. [Contributing](#Contributing)
4. [Contact](#Contact)
5. [License](#License)

## Getting started

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

#### CLI integration via JSON

The interfaces are designed to be modular and could also be integrated with existing CLI tooling. The binary is can be called and its output collected and processed, e.g.

```
rad-tui patch list --json
```

runs the patch list interface and return a JSON object specifying the operation and id selected:

```
{ "operation": "show", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }
```

## Build with `radicle-tui`

The library portion of this crate is a framework that is the foundation for all `radicle-tui` binaries. Although it evolved from the work on Radicle-specific applications and is far from being complete, it can serve as a general purpose framework to build applications on top already.

Find out more about the [framework](./FRAMEWORK.md).

> **Note:** The framework is under heavy development and is missing some common low-level widgets. These will be added where needed by the `radicle-tui` binaries.

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

## Contributing

Contributions are what make the open source community such an amazing place to learn, inspire, and create. Any contributions you make are greatly appreciated.

If you have any suggestions that would make this better, please clone the repo and open a patch. You can also simply open an issue with the label "enhancement".

## License

`radicle-tui` is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](./LICENSE-APACHE) and [LICENSE-MIT](./LICENSE-MIT) for details.

## Contact

Please get in touch on [Zulip](https://radicle.zulipchat.com/#narrow/channel/522964-TUI).

## Acknowledgments

Parts of this project rely on or were heavily inspired by some great open source projects. So we'd like to thank:

- [ratatui](https://ratatui.rs)
- [egui](https://github.com/egui)
- [tui-realm](https://github.com/veeso/tui-realm)
- [tui-textarea](https://github.com/rhysd/tui-textarea)
- [tui-rs-tree-widget](https://github.com/EdJoPaTo/tui-rs-tree-widget)
- [rust-chat-server](https://github.com/Yengas/rust-chat-server)
