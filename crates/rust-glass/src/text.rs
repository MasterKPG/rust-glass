//! Text layout. Text uses the interface font of the platform, shaped by
//! cosmic-text. Layouts are cached, so laying out the same text every frame
//! is cheap.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use cosmic_text::{
    Align as CtAlign, Attrs, Buffer, CacheKey, CacheKeyFlags, Family, FontSystem, Metrics, Shaping,
    SwashCache, SwashContent, Weight as CtWeight, Wrap, fontdb,
};

use crate::geometry::{Vec2, vec2};
use crate::icon::Icon;

pub(crate) const ATLAS_SIZE: u32 = 2048;
/// The glyph atlas grows to this size before it is ever cleared.
pub(crate) const MAX_ATLAS_SIZE: u32 = 4096;
pub(crate) const SPRITE_ATLAS_SIZE: u32 = 1024;

/// Font weight.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Weight {
    /// 400.
    #[default]
    Regular,
    /// 500.
    Medium,
    /// 600.
    Semibold,
    /// 700.
    Bold,
}

impl Weight {
    fn value(self) -> u16 {
        match self {
            Weight::Regular => 400,
            Weight::Medium => 500,
            Weight::Semibold => 600,
            Weight::Bold => 700,
        }
    }
}

/// Horizontal alignment of wrapped lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum TextAlign {
    /// Lines start at the left edge.
    #[default]
    Left,
    /// Lines are centered.
    Center,
    /// Lines end at the right edge.
    Right,
}

/// How text is set: size, weight, font and wrapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    /// Font size in points.
    pub size: f32,
    /// Font weight.
    pub weight: Weight,
    /// Use the monospace font.
    pub mono: bool,
    /// Wrap lines at this width in points. `None` keeps one line.
    pub wrap: Option<f32>,
    /// Alignment of wrapped lines.
    pub align: TextAlign,
}

impl TextStyle {
    /// Creates a regular, unwrapped style of `size` points.
    pub fn new(size: f32) -> Self {
        Self { size, weight: Weight::Regular, mono: false, wrap: None, align: TextAlign::Left }
    }
    /// Sets the weight.
    pub fn weight(mut self, weight: Weight) -> Self {
        self.weight = weight;
        self
    }
    /// Uses the monospace font.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }
    /// Wraps lines at `width` points.
    pub fn wrap(mut self, width: f32) -> Self {
        self.wrap = Some(width);
        self
    }
    /// Sets the alignment of wrapped lines.
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Glyph {
    /// Pen position in points, relative to the layout origin. `y` is the baseline.
    pub pos: Vec2,
    font: fontdb::ID,
    glyph: u16,
    weight: CtWeight,
    flags: CacheKeyFlags,
}

#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub top: f32,
    pub height: f32,
    /// Byte range in the source text.
    pub start: usize,
    pub end: usize,
    /// (byte offset, x) for every cursor stop on the line, in order.
    pub stops: Vec<(usize, f32)>,
}

/// Text that has been shaped and laid out. Paint it with
/// [`Painter::text`](crate::Painter::text).
#[derive(Clone, Debug)]
pub struct TextLayout {
    /// Size of the laid out text in points.
    pub size: Vec2,
    /// Font size it was laid out at.
    pub font_size: f32,
    pub(crate) glyphs: Vec<Glyph>,
    pub(crate) lines: Vec<Line>,
}

impl TextLayout {
    /// Returns the caret before byte `index` as `(x, top, height)`.
    pub fn caret(&self, index: usize) -> (f32, f32, f32) {
        for (i, l) in self.lines.iter().enumerate() {
            let last = i + 1 == self.lines.len();
            if index <= l.end || last {
                let next_starts_here = !last && self.lines[i + 1].start == index && index == l.end;
                if next_starts_here {
                    continue;
                }
                let x = l.stops.iter().rev().find(|s| s.0 <= index).map_or(0.0, |s| s.1);
                return (x, l.top, l.height);
            }
        }
        (0.0, 0.0, self.font_size * 1.3)
    }

    /// Returns the byte index closest to `point`.
    pub fn hit(&self, point: Vec2) -> usize {
        let p = point;
        let Some(line) = self.lines.iter().find(|l| p.y < l.top + l.height).or(self.lines.last()) else {
            return 0;
        };
        let mut best = (f32::MAX, line.start);
        for &(i, x) in &line.stops {
            let d = (x - p.x).abs();
            if d < best.0 {
                best = (d, i);
            }
        }
        best.1
    }
}

/// Where a rasterized glyph lives in its atlas.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AtlasEntry {
    pub uv: [f32; 4],
    pub offset: Vec2,
    pub size: Vec2,
    pub color: bool,
}

pub(crate) struct AtlasUpload {
    pub color: bool,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub data: Vec<u8>,
}

struct Shelf {
    size: u32,
    x: u32,
    y: u32,
    row: u32,
}

impl Shelf {
    fn new(size: u32) -> Self {
        Self { size, x: 1, y: 1, row: 0 }
    }
    fn allocate(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if self.x + w + 1 > self.size {
            self.x = 1;
            self.y += self.row + 1;
            self.row = 0;
        }
        if self.y + h + 1 > self.size || w + 2 > self.size {
            return None;
        }
        let at = (self.x, self.y);
        self.x += w + 1;
        self.row = self.row.max(h);
        Some(at)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Key {
    Glyph(CacheKey),
    Icon(Icon, u32),
}

/// Lays out text and keeps the fonts. Reach it through
/// [`Context::text`](crate::Context::text).
pub struct TextEngine {
    pub(crate) fonts: FontSystem,
    swash: SwashCache,
    buffer: Buffer,
    /// Layouts by key, with the frame and time they were last used.
    layouts: HashMap<u64, (Rc<TextLayout>, u64, f64)>,
    retention: f64,
    time: f64,
    atlas: HashMap<Key, Option<AtlasEntry>>,
    shelf: Shelf,
    sprite_shelf: Shelf,
    pub(crate) uploads: Vec<AtlasUpload>,
    /// Family name of the interface font. Change it after
    /// [`TextEngine::add_font`] to use your own font.
    pub sans: String,
    /// Family name of the monospace font.
    pub mono: String,
    frame: u64,
    full: bool,
    ppp: f32,
    /// Width and height of the glyph atlas in pixels.
    pub(crate) atlas_size: u32,
    /// The atlases were emptied since the renderer last uploaded, so what it
    /// shows from the last frame is no longer valid.
    pub(crate) atlas_reset: bool,
    grown: bool,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine {
    /// Loads the system fonts and picks the interface font of the platform.
    pub fn new() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        let has =
            |db: &fontdb::Database, name: &str| db.faces().any(|f| f.families.iter().any(|x| x.0 == name));
        let pick =
            |db: &fontdb::Database, names: &[&str]| names.iter().find(|n| has(db, n)).map(|n| n.to_string());
        // The platform's own interface font, so text matches other native apps.
        let sans = pick(
            &db,
            &[
                "SF Pro Text",
                ".SF NS",
                "Inter",
                "Segoe UI Variable",
                "Segoe UI",
                "Cantarell",
                "Noto Sans",
                "DejaVu Sans",
                "Arial",
            ],
        )
        .unwrap_or_else(|| "sans-serif".into());
        let mono = pick(
            &db,
            &["SF Mono", "Cascadia Mono", "Consolas", "JetBrains Mono", "Menlo", "DejaVu Sans Mono"],
        )
        .unwrap_or_else(|| "monospace".into());
        let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
        let buffer = Buffer::new(&mut fonts, Metrics::new(16.0, 20.0));
        Self {
            fonts,
            swash: SwashCache::new(),
            buffer,
            layouts: HashMap::new(),
            atlas: HashMap::new(),
            shelf: Shelf::new(ATLAS_SIZE),
            sprite_shelf: Shelf::new(SPRITE_ATLAS_SIZE),
            uploads: Vec::new(),
            sans,
            mono,
            frame: 0,
            full: false,
            ppp: 1.0,
            retention: 30.0,
            time: 0.0,
            atlas_size: ATLAS_SIZE,
            atlas_reset: false,
            grown: false,
        }
    }

    /// Adds a font file. Its family becomes available by name.
    pub fn add_font(&mut self, bytes: Vec<u8>) {
        self.fonts.db_mut().load_font_data(bytes);
        self.layouts.clear();
    }

    /// Keeps layouts that are not drawn for at least `keep`. A layout is
    /// dropped only when both this time and 240 frames have passed since it
    /// was last used. The default is 30 seconds.
    pub fn set_retention(&mut self, keep: std::time::Duration) {
        self.retention = keep.as_secs_f64();
    }

    /// Number of laid out texts in the cache.
    pub fn cached_layouts(&self) -> usize {
        self.layouts.len()
    }

    pub(crate) fn begin_frame(&mut self, ppp: f32, time: f64) {
        self.frame += 1;
        self.time = time;
        if ppp != self.ppp {
            self.ppp = ppp;
            self.layouts.clear();
            self.full = true;
        }
        if self.full {
            // Start over with an empty atlas; visible glyphs are rasterized again
            // during this frame.
            self.full = false;
            self.atlas.clear();
            self.shelf = Shelf::new(self.atlas_size);
            self.sprite_shelf = Shelf::new(SPRITE_ATLAS_SIZE);
            self.uploads.clear();
            self.atlas_reset = true;
        }
        if self.frame.is_multiple_of(240) {
            let (f, now, keep) = (self.frame, self.time, self.retention);
            // A budget keeps text that changes all the time, such as a clock,
            // from filling memory while the time limit has not passed.
            let keep = if self.layouts.len() > 20_000 { 0.0 } else { keep };
            self.layouts.retain(|_, v| f - v.1 < 240 || now - v.2 < keep);
        }
    }

    /// Lays out `text` in `style`. Results are cached.
    pub fn layout(&mut self, text: &str, style: &TextStyle) -> Rc<TextLayout> {
        let st = style;
        let mut h = std::hash::DefaultHasher::new();
        text.hash(&mut h);
        st.size.to_bits().hash(&mut h);
        st.weight.hash(&mut h);
        st.mono.hash(&mut h);
        st.wrap.map(f32::to_bits).hash(&mut h);
        st.align.hash(&mut h);
        let key = h.finish();
        if let Some((l, used, at)) = self.layouts.get_mut(&key) {
            (*used, *at) = (self.frame, self.time);
            return l.clone();
        }
        let l = Rc::new(self.shape(text, st));
        self.layouts.insert(key, (l.clone(), self.frame, self.time));
        l
    }

    fn shape(&mut self, text: &str, st: &TextStyle) -> TextLayout {
        let ppp = self.ppp;
        let px = st.size * ppp;
        let line_h = (px * 1.28).round();
        let family = if st.mono { self.mono.clone() } else { self.sans.clone() };
        let attrs = Attrs::new().family(Family::Name(&family)).weight(CtWeight(st.weight.value()));
        let align = match st.align {
            TextAlign::Left => None,
            TextAlign::Center => Some(CtAlign::Center),
            TextAlign::Right => Some(CtAlign::Right),
        };
        let b = &mut self.buffer;
        b.set_metrics(Metrics::new(px, line_h));
        b.set_wrap(if st.wrap.is_some() { Wrap::WordOrGlyph } else { Wrap::None });
        b.set_size(st.wrap.map(|w| w * ppp), None);
        b.set_text(text, &attrs, Shaping::Advanced, align);
        b.shape_until_scroll(&mut self.fonts, false);

        // Byte offset of each source line in the full text.
        let mut line_start = vec![0usize];
        for (i, c) in text.char_indices() {
            if c == '\n' {
                line_start.push(i + 1);
            }
        }
        let mut glyphs = Vec::new();
        let mut lines: Vec<Line> = Vec::new();
        let mut width = 0f32;
        for run in b.layout_runs() {
            let base = line_start.get(run.line_i).copied().unwrap_or(0);
            let mut stops: Vec<(usize, f32)> = Vec::with_capacity(run.glyphs.len() + 1);
            let (mut start, mut end) = (usize::MAX, 0);
            for g in run.glyphs {
                glyphs.push(Glyph {
                    pos: vec2(
                        (g.x + g.font_size * g.x_offset) / ppp,
                        (run.line_y + g.y - g.font_size * g.y_offset) / ppp,
                    ),
                    font: g.font_id,
                    glyph: g.glyph_id,
                    weight: g.font_weight,
                    flags: g.cache_key_flags,
                });
                stops.push((base + g.start, g.x / ppp));
                start = start.min(base + g.start);
                end = end.max(base + g.end);
            }
            let last_x = run.glyphs.last().map_or(0.0, |g| (g.x + g.w) / ppp);
            if start == usize::MAX {
                start = base;
                end = base;
            }
            stops.push((end, last_x));
            width = width.max(last_x - run.glyphs.first().map_or(0.0, |g| g.x / ppp).min(0.0));
            lines.push(Line { top: run.line_top / ppp, height: run.line_height / ppp, start, end, stops });
        }
        if text.ends_with('\n') {
            let top = lines.last().map_or(0.0, |l| l.top + l.height);
            lines.push(Line {
                top,
                height: line_h / ppp,
                start: text.len(),
                end: text.len(),
                stops: vec![(text.len(), 0.0)],
            });
        }
        if lines.is_empty() {
            lines.push(Line { top: 0.0, height: line_h / ppp, start: 0, end: 0, stops: vec![(0, 0.0)] });
        }
        if let (Some(w), true) = (st.wrap, st.align != TextAlign::Left) {
            width = w;
        }
        let height = lines.last().map_or(line_h / ppp, |l| l.top + l.height);
        TextLayout { size: vec2(width, height), font_size: st.size, glyphs, lines }
    }

    /// Rasterize (or fetch) a glyph drawn at pixel position `pos`. Returns the
    /// entry plus the integer pixel origin.
    pub(crate) fn glyph(&mut self, g: &Glyph, px: f32, pos: Vec2) -> Option<(AtlasEntry, Vec2)> {
        let (key, x, y) = CacheKey::new(g.font, g.glyph, px, (pos.x, pos.y), g.weight, g.flags);
        let k = Key::Glyph(key);
        let origin = vec2(x as f32, y as f32);
        if let Some(e) = self.atlas.get(&k) {
            return e.map(|e| (e, origin));
        }
        let entry = self.swash.get_image_uncached(&mut self.fonts, key).and_then(|img| {
            let p = img.placement;
            if p.width == 0 || p.height == 0 {
                return None;
            }
            let color = matches!(img.content, SwashContent::Color);
            let data = match img.content {
                SwashContent::SubpixelMask => img.data.as_chunks::<4>().0.iter().map(|c| c[1]).collect(),
                _ => img.data,
            };
            self.insert(color, p.width, p.height, data, vec2(p.left as f32, -p.top as f32))
        });
        self.atlas.insert(k, entry);
        entry.map(|e| (e, origin))
    }

    /// Icon rasterized at `px` by `px` pixels.
    pub(crate) fn icon(&mut self, icon: Icon, px: u32) -> Option<AtlasEntry> {
        let k = Key::Icon(icon, px);
        if let Some(e) = self.atlas.get(&k) {
            return *e;
        }
        let (data, w, h) = icon.rasterize(px);
        let entry = self.insert(false, w, h, data, Vec2::ZERO);
        self.atlas.insert(k, entry);
        entry
    }

    fn insert(&mut self, color: bool, w: u32, h: u32, data: Vec<u8>, offset: Vec2) -> Option<AtlasEntry> {
        let mut place = if color { self.sprite_shelf.allocate(w, h) } else { self.shelf.allocate(w, h) };
        if place.is_none() && !color && self.atlas_size < MAX_ATLAS_SIZE {
            self.grow_atlas();
            place = self.shelf.allocate(w, h);
        }
        let size = if color { SPRITE_ATLAS_SIZE } else { self.atlas_size } as f32;
        let Some((x, y)) = place else {
            self.full = true;
            return None;
        };
        self.uploads.push(AtlasUpload { color, x, y, w, h, data });
        Some(AtlasEntry {
            uv: [x as f32 / size, y as f32 / size, (x + w) as f32 / size, (y + h) as f32 / size],
            offset,
            size: vec2(w as f32, h as f32),
            color,
        })
    }

    /// Doubles the glyph atlas instead of emptying it. Glyphs keep their
    /// pixels in the top left quarter; the renderer copies them over.
    fn grow_atlas(&mut self) {
        self.grown = true;
        self.atlas_size *= 2;
        self.shelf.size = self.atlas_size;
        for e in self.atlas.values_mut().flatten() {
            if !e.color {
                e.uv = e.uv.map(|v| v * 0.5);
            }
        }
    }

    /// The atlas grew during this frame.
    pub(crate) fn take_grown(&mut self) -> bool {
        std::mem::take(&mut self.grown)
    }

    /// The atlas overflowed during this frame; it is rebuilt on the next one.
    pub(crate) fn needs_rebuild(&self) -> bool {
        self.full
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_and_hit_agree() {
        let mut t = TextEngine::new();
        let l = t.layout("Hello glass", &TextStyle::new(16.0));
        for i in 0..=11 {
            let (x, top, h) = l.caret(i);
            assert_eq!(l.hit(vec2(x, top + h * 0.5)), i);
        }
    }

    #[test]
    fn multi_line_offsets() {
        let mut t = TextEngine::new();
        let l = t.layout("ab\ncd", &TextStyle::new(16.0));
        assert_eq!(l.lines.len(), 2);
        assert_eq!(l.caret(3).1, l.lines[1].top);
        assert_eq!(l.hit(vec2(0.0, l.lines[1].top + 1.0)), 3);
    }

    #[test]
    fn layout_is_cached() {
        let mut t = TextEngine::new();
        let a = t.layout("same", &TextStyle::new(14.0));
        let b = t.layout("same", &TextStyle::new(14.0));
        assert!(Rc::ptr_eq(&a, &b));
    }
}
