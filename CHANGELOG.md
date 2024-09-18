# Changelog

## [0.5.1] - 2024-09-18

### Fixes

**Library features**

- Removes leftover code from basic example

## [0.5.0] - 2024-09-18

### Added

**Binary features**

- immediate mode UI implementation of `patch select`

**Library features**

- Support for immediate mode UIs

### Changed

**Binary features**

- `patch select` to run the immediate mode UI by default

**Library features**

- structure of UI modules to support both retained and immediate mode

## [0.4.0] - 2024-08-15

### Added

**Binary features:**

- Issue preview widgets in `issue select`
- Basic theming support with light and dark theme bundles
- Support for application settings
- Provide dynamically linked CI build

**Library features:**

- Widgets can be mutated in their render function
- Scrollable widgets calculate their state by using a stored render height
- Per-column visibility for tables depending on their render width
- Tables can render a scrollbar
- Predefined layouts for section groups
- Basic theming support via widget properties
- New widgets:
- `SplitContainer`: Vertically split container
- `Tree`: Generic tree widget
- `TextView`: Scrollable text view widget
- `TextArea`: Non-editable text area widget

- Support Nix and NixOS tooling via the use of Flakes
- Apply `cargo clippy` suggestions for trait implementations and
  missing documentation

### Changed

**Binary features**

- Selection interfaces don't show their browser scroll progress anymore
- Selection interfaces show their help as unstyled markdown

**Library features:**

- Use container focus for table highlighting
- Default keybindings for switching sections

### Removed

**Library features:**

- Widgets are not immutable anymore in their render function
- Ability to send messages through widgets
- All Radicle-dependent code (moved to `bin/`)
- Page size attribute from scrollable widgets
- Cutoff and footer attributes from table properties
- Logging facilities

### Fixed

- Broken positional argument passing in `rad.sh` proxy script

## [0.3.1] - 2024-06-11

### Added

- Changelog

### Changed

- Clarify binary usage in README

### Fixed

- Broken relative links to licenses in README