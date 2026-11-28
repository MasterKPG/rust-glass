# Changelog

All notable changes are listed here. Format: Keep a Changelog.

## [Unreleased]

## [0.4.0] - 2026-11-28

### Added
- `Quality` and `Context::quality` set how far glass may be simplified to keep up. Windows start at `Quality::Auto`: after three large frames over 14 ms of GPU time the glass steps down, and it steps back up after 120 frames with plenty of room. `Reduced` drops glass shadows, so glass covers only its body; at 4K on Intel UHD Graphics that makes a page 12 percent faster and opening a menu or a sheet about a fifth faster. `Low` also drops rainbow edges, light spilled onto edges and the bicubic blur filter. `FrameStats::quality` is the step a frame was drawn with. `Headless` draws at `Quality::Full`.
- `Headless::set_half_resolution` and `RUST_GLASS_NO_HALF=1` draw large blurred glass at full size everywhere, for comparison.
- The showcase takes `--quality full|reduced|low` for its headless renders, its Material Lab shows the quality step when it is not full, and the frame budget counts the heap allocations of a frame drawn in full.
- Only the part of the window that changed is drawn again. Each frame is compared with the last on a grid of tiles, and glass keeps the backdrop it captured while nothing under it changed. A still page now costs about 0.4 ms of GPU time instead of 3.5 to 5 ms at 1770 x 1170, and the Feedback page with its spinner 1.5 ms instead of 3.8. `RUST_GLASS_FULL_REDRAW=1` turns it off. Partial frames match full redraws pixel for pixel in every page and gesture of the capture run.
- `Context::request_repaint_after` asks for a frame later and lets the window sleep until then. `Context::waker` returns a `Waker` that wakes the window from another thread.
- `Context::hidden` and `Ui::hidden` run UI code without showing it: text is shaped and state kept, but nothing is painted, reacts, takes focus or asks for frames. Use it to prepare a page before it is shown.
- `Sense::Press` clicks when the button goes down. `nav_list` selects with it, like a sidebar on macOS, and runs the frame again so content drawn before the list shows the new page at once.
- `FrameInfo::last` and `FrameInfo::peak` give the last frame unsmoothed and the slowest of the last two seconds. `FrameStats` has `gpu_ms_last` and `redrawn_px`.
- `RUST_GLASS_TRACE=file` writes every frame with its timings and the time from input to the frame on screen.
- `ScrollArea::content_width` centers a narrower column in a vertical area and keeps the bar at the edge. `ScrollArea::bar_inset` moves the bar.
- `Ui::with_max_width` centers a column of at most a given width. `Ui::scroll_wheel` registers a scroll area cut to the clip.
- `Context::set_retention` and `TextEngine::set_retention` set how long unused state and text stay cached.
- `Material::liquidity` sets how liquid glass feels in motion: how far thumbs and the tab bar blob stretch and how long they wobble. The Material Lab has a slider for it.
- Pressed glass lights up where the pointer touches it, spreading through the body and catching the rim nearby, as on iOS 27: `Glass::glow`, set by `PressEffect::apply` for buttons, toolbars, menus, steppers, checkboxes, color wells and the tab bar.
- `Ui::drag_source_id` takes an id as it is, so an item keeps its state when it moves from one list to another.
- `--motion DIR` in the showcase renders drops, presses and the album view frame by frame, and the `reference` tool takes `store` to replay a tab bar at the scale of the iOS 27 clips.
- Window options: `Options::visible`, `Options::icon`, `Options::title_bar_follows_theme`, `Options::title_bar_colors`. `Context::hide_window` and `Context::show_window`. `App::close_requested` and `App::window_created`, and `winit` is re-exported.
- `Spring::step_within`, `Spring2d::step_within` and `SpringRect::step_within` stop a spring once the rest of its motion stays within a precision in the units of the value. `Spring::PRECISION` is what `step` uses.
- `Headless::set_full_redraw` draws every frame in full, to time the whole window.
- A frame budget for the showcase: `--bench FILE` times every page drawn in full and counts the passes, captured pixels and frames until rest of nine interactions, and `--compare BASE HEAD` fails when HEAD is more than 15 percent worse. Each push to dev is compared with the commit before it on the macOS runner.

### Changed
- The inside of large blurred glass, such as sidebars, sheets and pages that blur as they appear, is shaded at half resolution and scaled up; the edge, where glass bends and catches the light, stays at full size. The picture is the same within one step of 8 bit color. At 4K on Intel UHD Graphics pages draw about 4 percent faster and the album and sheet transitions 5 to 11 percent.
- Breaking: `FrameStats` has a new field, `quality`, so it can only be built with `..Default::default()`.
- A frame allocates about a quarter less: the renderer keeps its lists from frame to frame, and widget state stored again for the same id is overwritten in place. A showcase page drawn in full now allocates about 330 times instead of about 450, nearly all of it inside wgpu.
- Springs stop once the rest of their motion stays within half a step of an 8 bit color, or a hundredth of a point for positions, instead of waiting for their speed to die out. Windows go idle sooner: a switch draws 61 frames after a click instead of 153, a tab bar tap 50 instead of 78, and buttons, menus, sheets, page changes and the album view 2 to 12 percent fewer.
- Glass blurred by more than about 3.3 pixels no longer copies the sharp scene before it reads it: the first level of its blur is filtered from the scene directly, since the shader never reads the sharp level for it. That covers panels, sheets and toolbars of the regular material. Captured pixels drop by about 70 percent on eight of the eleven showcase pages, and a page drawn in full takes about 5 percent less GPU time (Player 5.4 to 4.4 ms on Intel UHD Graphics at 1770 x 1170). The pictures are the same.
- The tab bar blob, refitted to iOS 27 footage in light and dark: with a wide refracting edge that pulls the content around it into a rainbow, no outline, a stretch as it lands and no wobble after. Held, it is 112 by 77 points, overhangs the bar by 7.5 points, stretches less and follows the pointer without overshooting the ends of the bar. It settles in under 0.4 s after a stop instead of about 1 s, and drops back into the selection faster after a tap.
- The selection of the tab bar at rest is a flat shade of the bar as on iOS 27: a little darker on light glass and much darker on dark glass, where it used to be lighter.
- The edge of glass reads as the surface curving away instead of a drawn line: the rim fades softly into the body, and a band along the edge turns slightly darker on light glass. On dark glass the band no longer lightens, and the soft part of the rim and the light spilled onto the edge are halved, so menus, sidebars and panels keep a fine rim without a glow along their edges.
- A dropped item flies on from where it was let go with the speed it had, stays lifted until it is over its place, then sets down. Its place stays empty until it lands, so the item is never seen twice or in place early.
- Widget state and the adaptive state of glass are dropped only after 1200 frames and five minutes without use, and shaped text after 240 frames and 30 seconds. Before, a window that drew all the time lost them within seconds, so a page you came back to paid its first frame again and lost its scroll position.
- Glass far apart is captured in separate rectangles instead of one that covers both, which halves the copied area on some pages.
- The glyph atlas grows to 4096 pixels before it is emptied, so a long animation of scaled text no longer re-rasterizes every visible glyph in one frame.
- A focused text field, a toast at rest and scroll bars waiting to fade no longer draw every frame. They ask for the frame they need.
- Text and icons wholly outside the clip are skipped before their glyphs are rasterized.
- `Headless::settle` also waits for frames asked for with `request_repaint_after`.

### Fixed
- A new line in a multi-line text field put the caret two lines down until the next character was typed: the empty last line was laid out twice.
- The selection of a segmented control at rest sat a pixel low at some display scales, leaving a hair of the track above it. Its inset is now a whole number of pixels, the same above and below.
- A click that changes what is shown without starting an animation is drawn at once. The window used to wait for the next input, so the album view of the showcase opened only when the pointer moved.
- A card dropped in the other list of the showcase lands from where it was let go instead of appearing in place.
- A scroll area shorter than its scroll bar no longer panics.
- Widgets inside hidden content no longer join the Tab order.

## [0.3.0] - 2026-11-26

### Added
- `Gauge::thickness`, `Gauge::label_size` and `Gauge::label_padding`. The label of a gauge shrinks until it fits inside the ring with the padding to spare, so a word like "Battery" no longer touches it.
- `ProgressBar::track` sets the color of the unfilled part.
- Drag sources lift onto their glass plate as soon as they are pressed, follow the pointer into the drag without a jump, and settle back into place when let go.
- `Vec2::abs`.
- Pages of the showcase come into focus as they appear, using `Painter::blur`.
- `Context::morph` moves an element from the view that showed it to the view that shows it now. In the showcase, album covers grow from the grid into a detail view and back.
- Scroll areas scroll to show a widget that gets focus from the keyboard or a screen reader.
- The items of a reorderable list move apart to make room for the dragged item, where a line used to mark the spot.
- `Painter::blur` blurs what is already painted in a rectangle, and `Material::blur` is the glass behind it.
- Menus, popups, alerts and sheets bring their content into focus as they form, instead of only fading it in.
- A mask added while another is active is combined with it. A segmented control or tab bar inside a scroll area now fades at its edge like everything else.
- `Context::remove_image` frees an image.
- API docs for every public item. Pushes to dev fail when the docs have warnings.

### Fixed
- A collapsing section no longer stalls one spacing short of closed and then jumps shut. Opening starts without a jump too.
- Menus, combo boxes and context menus fade out together with their shape. The glass no longer lingers over the button at a third of its strength and vanishes at the end, and a context menu no longer leaves a dot behind.
- The blur that brings popups, alerts and sheets into focus fades with them, instead of outlasting them and disappearing at once.
- Button labels no longer shake when a press is released. The release settles without a wobble, labels never shrink below their size at rest, and scaled text lands on the same pixel it rests on.
- An element that morphs between views follows its target when the target moves, settles as soon as it is in place, and the album cover's shadow sinks back as it lands.

### Changed
- The guide describes how drag sources lift on press and how a morph follows a target that moves. The README lists morphs and content blur, and points to the planned egui integration.
- The progress bar is a flat capsule in one color, 6 points high, without a gradient or gloss.
- The album view of the showcase opens faster, and morphs travel on slightly quicker springs.
- New preview pictures and clips of the rebuilt showcase, including the album view and the Lists page. The preview run writes the crop of each clip next to its frames.
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

[Unreleased]: https://github.com/MasterKPG/rust-glass/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/MasterKPG/rust-glass/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/MasterKPG/rust-glass/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/MasterKPG/rust-glass/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/MasterKPG/rust-glass/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/MasterKPG/rust-glass/releases/tag/v0.1.0
