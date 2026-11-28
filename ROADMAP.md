# Roadmap

Where the project is going. Short-term tasks are in [TODO.md](TODO.md).

## Goal

A Rust UI framework whose controls look and move like Liquid Glass on every desktop platform, fast enough for integrated GPUs.

## Milestones

### v0.1: Material and core widgets (released 2026-11-25)
- Glass renderer with adaptive materials
- Core widgets and the showcase
- Guide, API docs and the first release on crates.io

### v0.2: Real apps (released 2026-11-26)
- Screen reader support through AccessKit
- Horizontal scrolling, long lists, drag and drop
- Text editing with undo and input methods
- Tests and renders on Windows, macOS and Linux

### v0.3: Clarity and motion (released 2026-11-26)
- Clear names, one module per widget, API docs for every item
- A showcase that is easy to read
- Elements that morph from one view into another
- Content blur during transitions
- Polish of every transition: menus, overlays, collapsing sections, presses and drags

### v0.4: Speed (released 2026-11-28)
Goal: a steady frame rate on integrated GPUs at 4K, with no loss in how glass looks.
- A frame budget per showcase page, checked in CI against the commit before each push: GPU time, passes, captured pixels, allocations and frames until rest. A page drawn in full takes 3.2 to 5 ms of GPU time at 1770 x 1170 on Intel UHD Graphics.
- Damage tracking for the whole window: a still frame costs about 0.4 ms
- Blurred glass read from a lower level of the blur chain (captures down about 70 percent), and the inside of large blurred glass drawn at half resolution
- An automatic quality step for GPUs that cannot keep up. Measured at 4K on Intel UHD Graphics, the time goes to the pixels glass covers, not its shading, so the first step drops glass shadows
- Renderer and widget state reuse their memory from frame to frame. What still allocates per frame is mostly inside wgpu, and the count is part of the frame budget
- Springs that stop when the change is no longer visible, so the window goes idle sooner

### v0.5: Components
Goal: the controls an app needs, matching the Liquid Glass set of iOS 26 and 27.
- Bars: toolbar with grouped items, navigation bar with a large title and the scroll edge effect, search in toolbars and tab bars, a tab bar that minimizes on scroll with a bottom accessory
- Presentations: popovers that flow out of their control, confirmation dialogs, sheet detents
- Navigation: sidebar, split view and inspector
- Pickers: date, time, wheel and page control
- Lists: swipe actions, edit mode, context menus with a preview
- Glass containers whose shapes merge, split and morph by id, and concentric corners
- The transparency setting of iOS 27, from ultra clear to fully tinted, already exists as `Theme::glass_tint`

### v0.6: Glass Studio
Goal: tune the look in a window and take it into code.
- The Material Lab grows into Glass Studio: a slider for every material and theme setting, with a live preview on real controls
- Export as a Rust file with `material()` and `theme()` functions, or copy it to the clipboard
- Load an exported file back to keep tuning

### v0.7: Reach
Goal: usable outside its own window.
- Renderer usable inside an existing wgpu app
- egui integration with the same look (below)
- Web build through WebGPU
- Touch gestures with momentum, and tilt from device sensors
- Screen readers checked with VoiceOver and Orca

## egui integration
egui draws everything in one render pass. Glass has to sample what is already drawn below it, so glass widgets cannot simply be dropped into that pass. The plan keeps egui for layout, input and its own widgets, and lets rust-glass render the frame:

1. A crate `rust-glass-egui` with a renderer that takes egui's tessellated output (meshes and paint callbacks, in order) in place of `egui_wgpu::Renderer`.
2. egui meshes are drawn into the rust-glass scene texture through a mesh pipeline. A glass widget adds a paint callback that carries its `Glass` description.
3. When the renderer reaches a glass callback, it plans passes the way it does today: what was drawn so far becomes the backdrop, the glass is drawn, and egui content continues on top. Glass sees the egui widgets below it, and egui widgets draw over glass.
4. Glass widgets with egui's API, such as `ui.add(GlassButton::new("Share"))`, plus a theme that sets egui's visuals to match: text colors, fills and corner radii.
5. Springs run on egui's animation time, and adaptive light and dark uses the same backdrop measurement as now.

A first spike checks two risks. egui_wgpu pins its own wgpu version, so the renderer may have to follow egui's wgpu releases. And egui expects a single pass, so the custom renderer has to be a drop-in replacement for the wgpu backend of eframe.

## Not planned
- A software renderer.
