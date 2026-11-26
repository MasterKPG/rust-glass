# Changelog

All notable changes are listed here. Format: Keep a Changelog.

## [Unreleased]

### Added
- `Context::remove_image` frees an image.
- API docs for every public item. Pushes to dev fail when the docs have warnings.

### Changed
- Breaking: each widget has its own module, such as `widgets::menu`, `widgets::dialog` and `widgets::toggle`. The old `bars`, `controls`, `overlay`, `gel` and `dnd` modules are gone. The prelude still exports every widget.
- Breaking: `Checkbox`, `Radio`, `ProgressBar`, `Gauge`, `Spinner`, `Image`, `Collapsing`, `ColorPicker`, `Stepper`, `SplitButton`, `TabBar` and `ComboBox` are created with `new` and builder methods instead of struct literals. `Row` is now `SettingsRow`.
- Breaking: renamed modules `anim` to `animation`, `geom` to `geometry`, `access` to `accessibility` and `icons` to `icon`.
- Breaking: renamed `Spring::x` and `Spring::v` to `value` and `velocity`, `Spring::at` to `Spring::new`, `Spring2` to `Spring2d`, `Context::animate2` to `animate_vec2` and `Context::tune` to `tuning`.
- Breaking: renamed `Context::ppp` to `pixels_per_point`, `Context::host` to `host_commands`, `HostCmd` to `HostCommand`, `Context::copy` to `copy_text`, `Context::adapt` to `backdrop` and its result `Adapted` to `Backdrop`.
- Breaking: renamed `Ui::max` to `max_rect`, `Ui::alloc` to `allocate`, `Ui::alloc_interact` to `allocate_response`, `Ui::text_with` to `styled_text` and `Response::tip` to `on_hover_text`.
- Breaking: renamed `Theme::frost` to `glass_tint`, `Material::frosts` to `glass_tint_response`, `Material::chroma` to `dispersion`, `Material::bleed` to `light_spill`, `Glass::lit` to `brightness`, `Corner::Round` to `Corner::Circular` and `Tuning::GEL` to `Tuning::PRESS`. The `knob` and `track` color setters of sliders and segmented controls are now `thumb_color` and `track_color`.
- Breaking: the colors of `Surface` end in `_color` (`text_color`, `secondary_color`, `fill_color`, `strong_fill_color` and so on), and `Surface::dark` is `darkness`. `Theme::text` and `Theme::background` are `text_color` and `background_color`.
- Breaking: renamed `Theme::body`, `caption` and `title` to `body_size`, `caption_size` and `title_size`, and `Theme::high_contrast` to `increase_contrast`.
- Breaking: renamed `Context::clear` to `clear_color`, `Context::ime` to `ime_area`, `Input::secondary` to `secondary_clicked`, `Input::double` to `double_clicked`, `Cursor::ResizeH` to `ResizeHorizontal` and `PressEffect::press` to `amount`.
- Breaking: `FrameStats::gpu_ms` is an `Option`. It is `None` where the GPU cannot measure time, such as the virtual machines of the macOS runners.
- Parameters of public functions have full names, such as `rect` and `color` instead of `r` and `c`.
- Shorter README and roadmap. The guide uses the new names and says which screen readers have been tried. Its examples compile again after the renames.
- Breaking: `Panel::flips` is now `Panel::adaptive`.
- `section` draws its title inside the card, so it stays readable over any wallpaper. Cards are a little more opaque.
- The showcase puts every group of widgets on a card with a title and a one line explanation, labels every control, and draws page titles on a soft shade. Debug timings moved from the sidebar to the Material Lab, and the Reference page left the showcase (the `reference` binary still replays the reference gestures).
- Breaking: the renderer, draw lists and glyph atlas are no longer public. `Context::center` is removed, use `ctx.screen.center()`.

## [0.2.0] - 2026-11-26

### Added
- Accessibility through AccessKit. Screen readers see every built in widget with its role, name, value and state, and can press buttons, flip switches, move sliders and steppers, pick tabs and segments and set the text of fields. The tree is built only while a screen reader listens. `Ui::access` describes your own widgets and `Context::access_label` names controls without text.
- Rows of a settings list give their title to the switch, slider or stepper at their end.
- Horizontal scrolling with `ScrollArea::horizontal()` and scrolling on both axes with `ScrollArea::both()`. The wheel goes to the innermost area that scrolls on its axis, so a row of cards inside a page leaves vertical scrolling to the page. Shift turns the wheel sideways.
- `ScrollArea::show_rows` lays out only the visible rows of a long list. A hundred thousand rows cost as much as ten.
- `ScrollArea::fade_sides`, `width`, `content_width`, and `scroll_offset` and `set_scroll_offset` to save and restore a position.
- Drag and drop: `Ui::drag_source` lifts its content onto a glass plate that follows the pointer, `Ui::drop_zone` accepts payloads of one type and lights up under them, `Ui::reorder` reorders a list by dragging. Escape cancels a drag.
- Undo and redo in text fields with Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y (Command on macOS). Typing undoes a word at a time.
- Input method composition shows underlined at the caret, and the candidate window opens next to it.
- `Ui::scope` and `Ui::scope_id` lay out a group of widgets as one, `Ui::reserve` sets a minimum size.
- Lists & Drag page in the showcase.
- Tests and headless renders on macOS and Linux for every push to dev.

### Changed
- The input method is enabled only while a text field has focus.
- `Context::scroll_area` routes wheel input per axis. `Context::scroll_area_axes` takes the axes an area scrolls on.

## [0.1.1] - 2026-11-25

### Changed
- Development moved to the dev branch. Merging it into main makes a release.

## [0.1.0] - 2026-11-25

First public release.

### Added
- Guide on how to build an app, also part of the API docs. Its examples are compiled by the tests.
- License: MIT or Apache-2.0, at your option.
- Published on crates.io. Later versions are released from pushes to main.
- Glass material with refraction, dispersion, variable blur, adaptive tint, rim light, inner glow and color carrying shadow. Presets: regular, clear, thick, lens.
- Renderer on wgpu where glass refracts everything drawn below it, including other glass.
- Immediate mode layout, input routing with layers, keyboard focus and spring animation.
- Components: buttons, slider, switch, segmented control, tab bar, text fields, menus, pickers, progress, toasts, alerts, sheets, windows, scroll area and more.
- Settings for frost, light and dark, reduced transparency, reduced motion and increased contrast.
- Showcase app with a live material editor, and headless capture with timings.
- Preview folder with pictures and three clips (tab bar, slider, + button), rendered with `--preview`.
- Tab bar selection lifts into a clear blob that stands proud of the bar, with rainbow caps. It gets milkier towards the tinted end of the transparency setting.
- Windows open centered and fit the monitor.
- Image mipmaps, so scaled down pictures stay smooth.

### Changed
- The light spot under the pointer is gone and the virtual light no longer follows the mouse. A press only brightens the surface evenly and slightly.
- Edges, rims and masks use a wider anti-aliasing profile, the strongly bent zone at the rim is supersampled, and resting controls sit on whole pixels.
- Thumbs change shape with acceleration instead of speed, fitted to the reference footage. Slider, switch and segment thumbs keep their area.
- The default transparency sits between ultra clear and tinted.
- Showcase pages form with a short transition.
- The tab bar blob is wider and flatter (104 by 72.4 points), squeezes as a whole with squarer shoulders, and follows a softer spring. Size, outline and motion are fitted to the reference clip.
- The caps of the blob taper along its corners and carry rainbows only there. The labels under it bend in a pass of their own.

### Fixed
- Sliders, switches and segmented controls stay in place while their window is dragged.
- The drops of a split button separate completely once open.
- Menus stay attached to a button that scrolls or moves.
- Springs no longer jump the last fraction of a point when they settle.

[Unreleased]: https://github.com/MasterKPG/rust-glass/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/MasterKPG/rust-glass/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/MasterKPG/rust-glass/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/MasterKPG/rust-glass/releases/tag/v0.1.0
