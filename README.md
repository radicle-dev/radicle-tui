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

This will install `rad-tui`. You can execute it by running `rad tui`. All available commands can be shown by running `rad tui --help`.

## Interfaces

The Radicle terminal interfaces are designed to be modular and to integrate well with the existing Radicle CLI. Right now, they are meant to be called from other programs that will collect and process their output.

### Usage

#### Patches

Select a patch and an operation, return both formatted as `rad` command:

```
$ rad tui patch select
rad patch show 546443226b300484a97a2b2d7c7000af6e8169ba (stderr)
```

Same as above:

```
$ rad tui patch select --operation
rad patch show 546443226b300484a97a2b2d7c7000af6e8169ba (stderr)
```

Select a patch and an operation, return both as JSON:

```
$ rad tui patch select --operation --json
{ "operation": "show", "id": "546443226b300484a97a2b2d7c7000af6e8169ba" } (stderr)
```

Select a patch only and return its id:

```
$ rad tui patch select --id
546443226b300484a97a2b2d7c7000af6e8169ba (stderr)
```

Select a patch only and return its id as JSON:

```
$ rad tui patch select --id --json
{ "operation": "null", "id": "546443226b300484a97a2b2d7c7000af6e8169ba" } (stderr)
```

## License

Radicle is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT) for details.
