# radicle-tui

*Radicle terminal user interfaces*

## Installation

**Requirements**

* *Linux* or *Unix* based operating system.
* Git 2.34 or later
* OpenSSH 9.1 or later with `ssh-agent`

### 📦 From source

> Requires the Rust toolchain.

You can install the binaries from source, by running the following
commands from inside this repository:

    cargo install --path . --force --locked

Or directly from our seed node:

    cargo install --force --locked --git https://seed.radicle.xyz/z39mP9rQAaGmERfUMPULfPUi473tY.git

This will install the following binaries:

- `rad-issue-tui`
- `rad-patch-tui`

## License

Radicle is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
