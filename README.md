# radicle-tui

_Radicle terminal user interfaces_

## Installation

**Requirements**

- _Linux_ or _Unix_ based operating system.
- Git 2.34 or later
- OpenSSH 9.1 or later with `ssh-agent`

### 📦 From source

> Requires the Rust toolchain.

You can install the binary from source, by running the following
commands from inside this repository:

    cargo install --path . --force --locked

Or directly from our seed node:

    cargo install --force --locked --git https://seed.radicle.xyz/z39mP9rQAaGmERfUMPULfPUi473tY.git

This will install `rad-tui`. You can execute it by running `rad-tui`. All available commands can be shown by running `rad-tui --help`.

## Interfaces

The Radicle terminal interfaces are designed to be modular and to integrate well with the existing Radicle CLI. Right now, they are meant to be called from other programs that will collect and process their output.

### Usage

#### Patches

Select a patch and an operation:

    $ rad-tui patch select
    { "operation": "show", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }


Same as above:

    $ rad-tui patch select --mode operation
    { "operation": "show", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }

Select a patch only and return its id:

    $ rad-tui patch select --mode id
    { "operation": "null", "ids": ["546443226b300484a97a2b2d7c7000af6e8169ba"], args:[] }

#### Issues

Select an issue and an operation:

    $ rad-tui issue select
    { "operation": "show", "ids": ["12f019e3f9f52d88b470a3d7fb922452ebaca39e"], args:[] }


Same as above:

    $ rad-tui issue select --mode operation
    { "operation": "show", "ids": ["12f019e3f9f52d88b470a3d7fb922452ebaca39e"], args:[] }


Select an issue only and return its id:

    $ rad-tui issue select --mode id
    { "operation": "null", "ids": ["12f019e3f9f52d88b470a3d7fb922452ebaca39e"], args:[] }


#### Inbox

Select a notification and an operation:

    $ rad-tui inbox select
    { "operation": "show", "ids": ["1"], args:[] }


Same as above:

    $ rad-tui inbox select --mode operation
    { "operation": "show", "ids": ["1"], args:[] }


Select a notification only and return its id:

    $ rad-tui inbox select --mode id
    { "operation": "null", "ids": ["1"], args:[] }


## License

Radicle is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
