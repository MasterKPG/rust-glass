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

### v0.3: Clarity and motion
Goal: an API that reads well, and transitions that keep glass on one continuous surface.
- Clear names, one module per widget, API docs for every item (done on dev)
- A showcase that is easy to read (done on dev)
- Elements that morph from one view into another
- Content blur during transitions
- Touch gestures with momentum, and tilt from device sensors
- Screen readers checked with VoiceOver and Orca

### v0.4: Reach
Goal: usable outside its own window.
- Renderer usable inside an existing wgpu app
- Web build through WebGPU

## Not planned
- Building on egui. It paints in one pass, so glass could not see the widgets below it.
- Bundling Apple fonts or SF Symbols. Their licenses do not allow it. Text uses the platform font, and the icons are drawn for this project.
- A software renderer.
