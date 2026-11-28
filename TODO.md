# TODO

Short-term work. The long-term plan is in [ROADMAP.md](ROADMAP.md).

## In progress

## Next
- [ ] Try the accessibility tree with VoiceOver on macOS (needs a Mac)
- [ ] Try the accessibility tree with Orca on Linux (needs a Linux desktop)
- [ ] Touch gestures with momentum
- [ ] The first frames of a full page blur cost about 24 ms on Intel UHD while the GPU clocks up from idle (album view). The sharp copy is gone and the inside is drawn at half resolution; what is left is the clock
- [ ] At 4K on Intel UHD a render pass over the scene costs about 7 ms even when it draws nothing (Vulkan and DirectX 12 alike), likely the driver resolving a compressed target at the low clock of the bench. Try fewer scene passes, or a scene texture without compression
- [ ] Partial frames differ from full redraws by 1 or 2 steps of 8 bit color on a few thousand pixels of some pages (Buttons, Segments, Text Fields, Menus, Feedback, Windows); it was already so before v0.4. Find the pass that reads differently
- [ ] `Glass::shapes` is a `Vec`, so every glass element allocates once per frame; an inline list for one or two shapes would avoid it (breaking change)
- [ ] Check after a few pushes whether the frame budget on the macOS runner is steady enough at 15 percent, or needs a third run per side

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
- [ ] The Lists & Drag page allocates about 960 times per frame against about 330 elsewhere: find what its rows allocate

## Backlog
- [ ] Drag and drop between windows and to other apps
- [ ] Nested submenus
- [ ] Image tests against stored reference renders
- [ ] egui integration (the approach is in the roadmap)

## Done (recent)
- [x] v0.4 speed work (2026-11-28): the inside of large blurred glass drawn at half resolution (4 percent faster pages at 4K, album and sheet transitions 5 to 11 percent, same picture within one step of 8 bit color); `Quality` with an automatic step that drops glass shadows first (12 to 16 percent at 4K, a fifth of a menu or sheet opening); renderer lists and widget state reused from frame to frame (about 450 allocations per frame down to about 330, the rest inside wgpu), with the count in the frame budget
- [x] Fixes (2026-11-28): caret after a new line in multi-line fields, segmented selection centered at every scale, dark glass edges without a light band, a tighter tab bar blob checked against the iOS 27 clips
- [x] Speed pass (2026-11-27): frame budget per page and per interaction, checked in CI against the commit before; blurred glass skips the sharp copy (captures down about 70 percent); springs stop once their motion no longer shows (a switch rests after 61 frames instead of 153). Shader variants without the lens were measured and dropped: the branches are the same across a draw, so they cost nothing to skip
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
