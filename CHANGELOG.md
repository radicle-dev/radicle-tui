# Changelog

## [0.4.0] - 2024-07-19

### Added

**Binary features:**

- Issue preview widgets in `issue select`
- Basic theming support with light and dark theme bundles
- Support for application settings

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