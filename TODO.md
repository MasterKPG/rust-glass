# TODO

Short-term work. Long-term plan lives in [ROADMAP.md](ROADMAP.md).

## In progress
- [x] Clear names and one module per widget
- [x] API docs for every public item
- [ ] Showcase that is easier to read
- [ ] Shorter README and guide

## Next
- [ ] Check the macOS and Linux renders from CI against the Windows ones
- [ ] Try the accessibility tree with VoiceOver and Orca
- [ ] Blur content while a menu or sheet forms, not just fade it
- [ ] Combine nested masks, so a segmented control fades at the edge of a scroll area
- [ ] Items of a reordered list make room for the dragged one, instead of a line marking the spot
- [ ] Scroll a focused widget into view, also when a screen reader moves to it

## Backlog
- [ ] Drag and drop between windows and to other apps
- [ ] Date picker
- [ ] Nested submenus
- [ ] Render only the damaged part of the window
- [ ] Image tests against stored reference renders

## Done (recent)
- [x] Accessibility tree through AccessKit, checked with Windows UI Automation (2026-11-26)
- [x] Horizontal and two axis scrolling, wheel routed per axis (2026-11-26)
- [x] Virtualized lists with `show_rows` (2026-11-26)
- [x] Drag and drop, reorderable lists (2026-11-26)
- [x] Undo and redo in text fields, input method preview (2026-11-26)
- [x] Showcase on DirectX 12: renders like Vulkan; macOS and Linux run in CI (2026-11-26)
- [x] Public repository, license, guide, v0.1.0 on crates.io and GitHub (2026-11-25)
- [x] Releases from pushes to main (2026-11-25)
- [x] Dev branch with its own tests, main is for releases (2026-11-25)
- [x] Renderer, material, springs, text, input and layout (2026-11-25)
- [x] First set of components (2026-11-25)
- [x] Showcase with live material editor (2026-11-25)
- [x] Slider and segmented control matched frame by frame to the reference clips (2026-11-25)
- [x] Tab bar blob, smoother edges, rigid windows, separating drops, no pointer light (2026-11-25)
- [x] Tab bar blob refitted to the clip: wider, squarer, tapered caps, softer squeeze (2026-11-25)
- [x] Clips of the slider and the + button in the README (2026-11-25)
- [x] Interaction tests for the widgets (2026-11-25)
- [x] Pass planning by overlap, from up to 22 passes to 3 (2026-11-25)
