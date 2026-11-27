//! Frame budget: times every page and counts the work of a set of
//! interactions (`--bench FILE`), and compares two such files
//! (`--compare BASE HEAD`). CI runs both on the commit before a push and on
//! the push itself, on the same machine, and fails when the push is more than
//! 15 percent slower.
//!
//! A file has one value per line: kind, name and value, separated by tabs.
//! The counts (passes, captured pixels, frames until rest) are the same on
//! every run and fail at 15 percent. Times change from run to run: the sum of
//! all pages fails at 15 percent, a single page only at twice that, and the
//! time of an interaction is shown but never fails.

use std::collections::BTreeMap;

use rust_glass::input::{Event, Key};
use rust_glass::prelude::*;

use crate::capture::{AURORA, DT, DUNES, PETALS, RIBBONS, Rig, wallpaper_for};
use crate::pages::Page;

/// How much worse a value may get before the comparison fails.
const ALLOWED: f64 = 0.15;

/// Frames timed per round, and rounds per page. A page keeps the median of
/// each round and the fastest of the rounds, which is steady even on a busy
/// machine.
const FRAMES: usize = 15;
const ROUNDS: usize = 3;

/// The longest an interaction may take to come to rest, in seconds.
const MAX_SECONDS: f64 = 6.0;

/// Time of the last frame in milliseconds: on the GPU where it can be
/// measured, else from start to finish on the CPU.
fn frame_ms(rig: &Rig, wall: f64) -> f64 {
    rig.host.info.gpu.gpu_ms_last.map_or(wall, |ms| ms as f64)
}

fn step_timed(rig: &mut Rig, events: &[Event]) -> f64 {
    let t = std::time::Instant::now();
    rig.step(events);
    let wall = t.elapsed().as_secs_f64() * 1000.0;
    frame_ms(rig, wall)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// Values of one run, by kind and name.
#[derive(Default)]
struct Sheet {
    lines: Vec<(String, String, f64)>,
}

impl Sheet {
    fn add(&mut self, kind: &str, name: &str, value: f64) {
        self.lines.push((kind.into(), name.into(), value));
    }
}

/// Runs frames until the window would sleep and returns how many frames a
/// window draws on the way, and their time. A frame is drawn when the one
/// before asked for it, or when a frame asked for later is due.
fn until_rest(rig: &mut Rig) -> (f64, f64) {
    let start = rig.host.ctx.time;
    let (mut frames, mut ms, mut quiet) = (1.0, frame_ms(rig, 0.0), 0);
    let mut wanted = (rig.host.ctx.wants_repaint(), rig.host.ctx.repaint_deadline());
    while rig.host.ctx.time - start < MAX_SECONDS && quiet < 6 {
        let now = rig.host.ctx.time + DT as f64;
        let drawn = wanted.0 || wanted.1.is_some_and(|at| at <= now + 1e-6);
        let t = step_timed(rig, &[]);
        if drawn {
            frames += 1.0;
            ms += t;
            quiet = 0;
        } else {
            quiet += 1;
        }
        wanted = (rig.host.ctx.wants_repaint(), rig.host.ctx.repaint_deadline());
    }
    (frames, ms)
}

/// Runs one frame with `events` and then until the window would sleep.
fn interaction(sheet: &mut Sheet, rig: &mut Rig, name: &str, events: &[Event]) {
    rig.step(events);
    let (frames, ms) = until_rest(rig);
    sheet.add("frames", name, frames);
    sheet.add("motion_ms", name, ms);
    println!("{name:<20} {frames:4} frames, {ms:7.1} ms");
}

/// Presses `offset` away from the middle of the marked widget and returns
/// the release, for [`interaction`].
fn click(rig: &mut Rig, mark: &str, offset: Vec2) -> [Event; 1] {
    rig.move_to(rig.mark(mark) + offset);
    rig.step(&[Event::PointerDown]);
    [Event::PointerUp]
}

/// Writes the frame budget of the showcase to `path`.
pub fn run(path: &str) -> Result<(), String> {
    let mut rig = Rig::new(1770, 1170, 1.5)?;
    let mut sheet = Sheet::default();
    println!("{} at 1770 x 1170", rig.host.describe());

    for page in Page::ALL {
        rig.open(page, wallpaper_for(page));
        let name = page.title();
        // What a page at rest draws again, frame after frame.
        rig.step(&[]);
        sheet.add("still_kpx", name, (rig.host.info.gpu.redrawn_px / 1000) as f64);
        rig.host.set_full_redraw(true);
        for _ in 0..4 {
            rig.step(&[]);
        }
        let ms = (0..ROUNDS)
            .map(|_| median((0..FRAMES).map(|_| step_timed(&mut rig, &[])).collect()))
            .fold(f64::MAX, f64::min);
        let gpu = rig.host.info.gpu;
        rig.host.set_full_redraw(false);
        sheet.add("page_ms", name, ms);
        sheet.add("passes", name, gpu.passes as f64);
        sheet.add("captured_kpx", name, (gpu.captured_px / 1000) as f64);
        sheet.add("shapes", name, gpu.shapes as f64);
        println!("{name:<20} {ms:6.2} ms, {} passes, {} kpx captured", gpu.passes, gpu.captured_px / 1000);
    }

    rig.open(Page::Buttons, PETALS);
    rig.app.page = Page::Menus;
    interaction(&mut sheet, &mut rig, "page switch", &[]);

    rig.open(Page::Buttons, PETALS);
    let up = click(&mut rig, "regular button", Vec2::ZERO);
    interaction(&mut sheet, &mut rig, "button click", &up);

    rig.open(Page::Controls, AURORA);
    let up = click(&mut rig, "wifi switch", Vec2::ZERO);
    interaction(&mut sheet, &mut rig, "switch", &up);

    rig.open(Page::Selection, RIBBONS);
    let up = click(&mut rig, "tab bar", vec2(77.0, 0.0));
    interaction(&mut sheet, &mut rig, "tab tap", &up);

    rig.open(Page::Menus, PETALS);
    let up = click(&mut rig, "options menu", Vec2::ZERO);
    interaction(&mut sheet, &mut rig, "menu open", &up);
    interaction(&mut sheet, &mut rig, "menu close", &[Event::Key(Key::Escape)]);

    rig.open(Page::Windows, AURORA);
    let up = click(&mut rig, "sheet button", Vec2::ZERO);
    interaction(&mut sheet, &mut rig, "sheet open", &up);

    rig.open(Page::Player, DUNES);
    rig.scroll(rig.mark("player content"), -1000.0);
    let up = click(&mut rig, "first album", Vec2::ZERO);
    interaction(&mut sheet, &mut rig, "album open", &up);
    let up = click(&mut rig, "album back", Vec2::ZERO);
    interaction(&mut sheet, &mut rig, "album close", &up);

    let mut out = format!("# {}\n", rig.host.describe());
    for (kind, name, value) in &sheet.lines {
        out += &format!("{kind}\t{name}\t{value}\n");
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

/// Reads one or more files written by [`run`], separated by commas. Of
/// several runs, each value keeps its best.
fn read(list: &str) -> Result<BTreeMap<(String, String), f64>, String> {
    let mut values = BTreeMap::new();
    for path in list.split(',') {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let mut parts = line.split('\t');
            let (Some(kind), Some(name), Some(value)) = (parts.next(), parts.next(), parts.next()) else {
                return Err(format!("{path}: cannot read {line:?}"));
            };
            let value: f64 = value.parse().map_err(|_| format!("{path}: cannot read {line:?}"))?;
            let key = (kind.to_string(), name.to_string());
            let best = values.get(&key).map_or(value, |old: &f64| old.min(value));
            values.insert(key, best);
        }
    }
    Ok(values)
}

/// How much a value of `kind` may grow without counting, on top of
/// [`ALLOWED`]. Small times jitter by a fraction of a millisecond, and a
/// frame more or less is no regression.
fn slack(kind: &str) -> f64 {
    match kind {
        "page_ms" => 0.15,
        "frames" => 2.0,
        "still_kpx" | "captured_kpx" => 32.0,
        "shapes" => 8.0,
        _ => 0.0,
    }
}

/// Compares the runs in `base` with those in `head`, prints every value and
/// fails when one got more than 15 percent worse.
pub fn compare(base: &str, head: &str) -> Result<(), String> {
    let (base, head) = (read(base)?, read(head)?);
    let mut worse = Vec::new();
    println!("{:<14} {:<20} {:>10} {:>10} {:>8}", "kind", "name", "base", "head", "change");
    for ((kind, name), &now) in &head {
        let Some(&before) = base.get(&(kind.clone(), name.clone())) else {
            println!("{kind:<14} {name:<20} {:>10} {now:>10.2}      new", "");
            continue;
        };
        let change = if before > 0.0 { (now / before - 1.0) * 100.0 } else { 0.0 };
        let allowed = match kind.as_str() {
            "page_ms" => 2.0 * ALLOWED,
            "motion_ms" => f64::INFINITY,
            _ => ALLOWED,
        };
        let over = now > before * (1.0 + allowed) + slack(kind);
        let mark = if over { "  <- over budget" } else { "" };
        println!("{kind:<14} {name:<20} {before:>10.2} {now:>10.2} {change:>+7.1}%{mark}");
        if over {
            worse.push(format!("{kind} of {name}: {before:.2} -> {now:.2}"));
        }
    }
    let total = |m: &BTreeMap<(String, String), f64>| {
        m.iter().filter(|((k, _), _)| k == "page_ms").map(|(_, v)| v).sum::<f64>()
    };
    let (before, now) = (total(&base), total(&head));
    if before > 0.0 {
        println!("all pages: {before:.2} ms -> {now:.2} ms ({:+.1}%)", (now / before - 1.0) * 100.0);
        if now > before * (1.0 + ALLOWED) {
            worse.push(format!("all pages: {before:.2} -> {now:.2} ms"));
        }
    }
    if worse.is_empty() {
        println!("Within the frame budget.");
        Ok(())
    } else {
        Err(format!("More than {:.0} percent worse than before:\n{}", ALLOWED * 100.0, worse.join("\n")))
    }
}
