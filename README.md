# radicle-tui

_Radicle terminal user interfaces_

## Interfaces

The Radicle terminal interfaces are designed to be modular and to integrate well with the existing Radicle CLI. Right now, they are meant to be called from other programs that will collect and process their output.

### Installation

**Requirements**

- _Linux_ or _Unix_ based operating system.
- Git 2.34 or later
- OpenSSH 9.1 or later with `ssh-agent`

#### From source

> Requires the Rust toolchain.

You can install the binary from source, by running the following
commands from inside this repository:

```
cargo install --path . --force --locked
```

Or directly from our seed node:

```
cargo install --force --locked --git https://seed.radicle.xyz/z39mP9rQAaGmERfUMPULfPUi473tY.git
```

This will install `rad-tui`. You can execute it by running `rad-tui`. All available commands can be shown by running `rad-tui --help`.

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

### Output

All interfaces return a JSON object that reflects the choices made by the user, e.g.: 

```
{ "operation": "show", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }
```

## Library

The library portion of this crate is a framework for creating TUIs built on top of [ratatui](https://ratatui.rs). It is the base for the interfaces mentioned above.

### Design

The framework was inspired by the Flux application pattern and took some ideas from [rust-chat-server](https://github.com/Yengas/rust-chat-server)


## License

Radicle is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
