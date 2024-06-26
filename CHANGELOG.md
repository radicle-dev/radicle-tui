# Changelog

## Unreleased

### Added

**Library features:**

- Widgets can be mutated in their render function
- Scrollable widgets calculate their state by using a stored render height
- Per-column visibility for tables depending on their render width
- Vertically split container
- Predefined layouts for section groups
- `TextView`: Scrollable text viewer widget
- `TextArea`: Non-editable text area widget

### Changed

**Library features:**

- Use container focus for table highlighting
- Default keybindings for switching sections

### Removed

**Library features:**

- Widgets are not immutable anymore in their render function
- Ability to send messages through widgets
- All Radicle-dependent code (moved to `bin/`)
- Page size attribute from scrollable widgets
- Cutoff attributes from table properties

## [0.3.1] - 2024-06-11

### Added

- Changelog

### Changed

- Clarify binary usage in README

### Fixed

- Broken relative links to licenses in README