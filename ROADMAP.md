# Roadmap

Where the project is going. Day-to-day tasks are in [TODO.md](TODO.md).

## Vision

A Rust UI framework whose controls look and move like Liquid Glass on every desktop platform, fast enough for integrated GPUs.

## Milestones

### v0.1: Material and core components
Goal: the glass holds up next to the reference footage and covers the components an app needs daily.
- Glass renderer with adaptive materials
- Core components and showcase
- Visual corrections after review

### v0.2: Real apps
Goal: a small app can ship with it.
- Accessibility tree through AccessKit
- Horizontal scrolling, virtualized lists, drag and drop
- Text editing with undo and IME preview
- Tested on Windows (Vulkan, DirectX 12), macOS and Linux

### v0.3: Motion
Goal: transitions between screens keep glass on one continuous plane.
- Shared element morphs between views
- Content blur during transitions
- Touch gestures with momentum, and tilt from device sensors

### v0.4: Reach
Goal: usable outside its own window host.
- Renderer usable inside an existing wgpu app
- Web build through WebGPU
- Published crate with API docs

## Not planned
- Building on egui. Its single render pass prevents glass from sampling the widgets below it.
- Bundling Apple fonts or SF Symbols. Their licenses do not allow it. Text uses the platform font, icons are drawn for this project.
- A software renderer.
