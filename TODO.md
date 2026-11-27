# TODO

Short-term work. The long-term plan is in [ROADMAP.md](ROADMAP.md).

## In progress

## Next
- [ ] Try the accessibility tree with VoiceOver on macOS (needs a Mac)
- [ ] Try the accessibility tree with Orca on Linux (needs a Linux desktop)
- [ ] Touch gestures with momentum
- [ ] Frame budget per page in CI: fail when the capture run gets more than 15 percent slower, measured with `RUST_GLASS_FULL_REDRAW=1` now that still frames cost almost nothing
- [ ] The first frames of a full page blur cost about 24 ms on Intel UHD while the GPU clocks up from idle (album view); blur large surfaces at a lower resolution

### Glass Studio: export settings as Rust
- [ ] Turn the Material Lab into Glass Studio: sliders for every `Material` field and the theme settings (glass tint, accent, dark mode, contrast, reduce transparency and motion)
- [ ] Live preview on a set of real controls (button, toggle, slider, tab bar, menu) over any wallpaper
- [ ] "Export as Rust" writes a `glass_theme.rs` with `pub fn material() -> Material` and `pub fn theme() -> Theme`, leaving out values that match the defaults, ready to drop into a project
- [ ] Copy the same code to the clipboard
- [ ] Load an exported file back by parsing the values it wrote
- [ ] Doc test that compiles an exported file

### Components missing next to Apple's set
- [ ] Toolbar: grouped items on one piece of glass, flexible and fixed spacers, overflow menu
- [ ] Navigation bar with a large title that shrinks on scroll, and the scroll edge effect under bars
- [ ] Search field in a toolbar, and a search tab that expands from the tab bar
- [ ] Tab bar that minimizes on scroll, with a bottom accessory (the mini player in the showcase is one)
- [ ] Popover: anchored glass that flows out of its control, with an arrowless and an arrowed style
- [ ] Confirmation dialog (action sheet)
- [ ] Sheet detents: medium and large, inset partial height sheets
- [ ] Sidebar and split view as widgets, with an inspector column
- [ ] Date and time pickers: compact, graphical and wheel
- [ ] Wheel picker
- [ ] Page control
- [ ] Swipe actions and edit mode in lists
- [ ] Context menu with a preview of its target
- [ ] Menu sections, inline pickers and a palette row
- [ ] Slider with tick marks and a neutral value
- [ ] Token field
- [ ] Badges on tab bar items and buttons
- [ ] Glass container: several glass shapes that merge and split as one surface, with ids that morph between states
- [ ] Concentric corners: children take the radius of their container minus the padding

### Speed
- [ ] Several damage rectangles instead of one, for two things that animate far apart
- [ ] Blur large surfaces from a lower mip level where the result looks the same
- [ ] Shader variants without dispersion and pinch for materials that turn them off
- [ ] Automatic quality step on slow GPUs: fewer blur taps and no dispersion when frames run over budget
- [ ] No allocation per frame in layout, reorder and text paths
- [ ] Settle springs by visible change, the way morphs now do, so idle windows stop drawing sooner

## Backlog
- [ ] Drag and drop between windows and to other apps
- [ ] Nested submenus
- [ ] Image tests against stored reference renders
- [ ] egui integration (the approach is in the roadmap)

## Done (recent)
- [x] Tweaks pass against iOS 27 footage (2026-11-27): tab bar blob refraction, rainbow edge, size and settle; resting selection shade; soft glass edge; press glow; `Material::liquidity` in the Material Lab; drops that land where they were let go; album view that opened only when the pointer moved
- [x] Field report from building EqualizeR (2026-11-26): damage tracking with backdrops kept per pass, timed repaints and a waker, hidden drawing for warm pages, press to select in sidebars, unsmoothed and peak frame timings with a trace file, time-based cache retention, a growing glyph atlas, scroll area column and bar options, no panic in short scroll areas, window options for icon, start hidden, title bar and close
- [x] Docs pass for the v0.3 release: guide, README and roadmap (2026-11-26)
- [x] Polish pass (2026-11-26): gauge labels fit their ring with padding, flat progress bar, collapsing sections without a stall, menus and context menus that close in one motion, overlay blur that fades with the overlay, button labels without shake, a grab animation for drag sources, and album morphs that settle as soon as they are in place
- [x] Preview pictures and clips rendered again from the new showcase (2026-11-26)
- [x] Content blur during transitions: overlays, showcase pages and the album view (2026-11-26)
- [x] Elements morph from one view into another with `Context::morph` (2026-11-26)
- [x] Focus from the keyboard or a screen reader scrolls the widget into view (2026-11-26)
- [x] Reordered lists make room for the dragged item (2026-11-26)
- [x] Menus, alerts and sheets blur their content while they form (2026-11-26)
- [x] Nested masks combine, so a segmented control fades at the edge of a scroll area (2026-11-26)
- [x] Shorter README, roadmap and guide (2026-11-26), guide examples checked by the doc tests
- [x] Showcase that is easier to read: titled cards, labeled controls (2026-11-26)
- [x] API docs for every public item (2026-11-26)
- [x] Clear names and one module per widget (2026-11-26)
- [x] Compared the macOS and Linux renders from CI with Windows: glass matches, only the fonts differ (2026-11-26)
- [x] Accessibility tree through AccessKit, checked with Windows UI Automation (2026-11-26)
- [x] Horizontal and two axis scrolling, wheel routed per axis (2026-11-26)
- [x] Lists that lay out only the visible rows (2026-11-26)
- [x] Drag and drop, reorderable lists (2026-11-26)
- [x] Undo and redo in text fields, input method preview (2026-11-26)
- [x] Showcase on DirectX 12 renders like Vulkan; macOS and Linux run in CI (2026-11-26)
- [x] Public repository, license, guide, v0.1.0 on crates.io and GitHub (2026-11-25)
- [x] Releases from pushes to main (2026-11-25)
- [x] Renderer, material, springs, text, input and layout (2026-11-25)
- [x] Slider and segmented control matched frame by frame to the reference clips (2026-11-25)
- [x] Tab bar blob refitted to the clip (2026-11-25)
- [x] Pass planning by overlap, from up to 22 passes to 3 (2026-11-25)
