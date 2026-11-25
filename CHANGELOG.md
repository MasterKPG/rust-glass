# Changelog

All notable changes are listed here. Format: Keep a Changelog.

## [Unreleased]

### Added
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
