//! Drives widgets with scripted input. No GPU needed.

use rust_glass::input::{Event, Key, Modifiers};
use rust_glass::prelude::*;

type Draw<'a> = &'a mut dyn FnMut(&mut Context);

/// Runs UI code frame by frame.
struct Rig {
    ctx: Context,
    t: f64,
}

impl Rig {
    fn new() -> Self {
        Self { ctx: Context::new(), t: 0.0 }
    }

    fn frame(&mut self, ev: &[Event], f: Draw) {
        self.t += 1.0 / 60.0;
        self.ctx.begin_frame(vec2(800.0, 600.0), 1.0, self.t, ev);
        f(&mut self.ctx);
        self.ctx.end_frame();
        for _ in 0..3 {
            if !self.ctx.wants_relayout() {
                break;
            }
            self.ctx.begin_frame(vec2(800.0, 600.0), 1.0, self.t, &[]);
            f(&mut self.ctx);
            self.ctx.end_frame();
        }
    }

    fn idle(&mut self, n: usize, f: Draw) {
        for _ in 0..n {
            self.frame(&[], f);
        }
    }

    fn click(&mut self, p: Vec2, f: Draw) {
        self.frame(&[Event::PointerMoved(p)], f);
        self.frame(&[Event::PointerDown], f);
        self.frame(&[Event::PointerUp], f);
        self.frame(&[], f);
    }

    fn drag(&mut self, a: Vec2, b: Vec2, f: Draw) {
        self.frame(&[Event::PointerMoved(a)], f);
        self.frame(&[Event::PointerDown], f);
        for i in 1..=20 {
            self.frame(&[Event::PointerMoved(a.lerp(b, i as f32 / 20.0))], f);
        }
        self.idle(30, f);
        self.frame(&[Event::PointerUp], f);
        self.frame(&[], f);
    }
}

fn page(ctx: &mut Context, f: impl FnOnce(&mut Ui)) {
    region(ctx, "page", rect(50.0, 50.0, 600.0, 500.0), f);
}

#[test]
fn button_clicks_only_when_released_inside() {
    let mut rig = Rig::new();
    let (mut n, mut r) = (0, Rect::default());
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            let b = ui.button("Press");
            r = b.rect;
            n += b.clicked() as u32;
        })
    };
    rig.idle(2, &mut ui);
    rig.click(vec2(60.0, 60.0), &mut ui);
    rig.drag(vec2(60.0, 60.0), vec2(500.0, 400.0), &mut ui);
    assert_eq!(n, 1);
    assert!(r.width() > 40.0);
}

#[test]
fn disabled_button_does_nothing() {
    let mut rig = Rig::new();
    let mut n = 0;
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            let r = ui.add_enabled(false, |ui| ui.button("Press"));
            n += r.clicked() as u32;
        })
    };
    rig.idle(2, &mut ui);
    rig.click(vec2(60.0, 60.0), &mut ui);
    assert_eq!(n, 0);
}

#[test]
fn slider_drags_to_both_ends_and_clamps() {
    let mut rig = Rig::new();
    let mut v = 0.5;
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ui.add(Slider::new(&mut v, 0.0..=1.0).width(300.0));
        })
    };
    rig.idle(2, &mut ui);
    rig.drag(vec2(200.0, 66.0), vec2(780.0, 66.0), &mut ui);
    rig.idle(40, &mut ui);
    let _ = &ui;
    assert_eq!(v, 1.0);
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ui.add(Slider::new(&mut v, 0.0..=1.0).width(300.0));
        })
    };
    rig.drag(vec2(331.0, 66.0), vec2(0.0, 66.0), &mut ui);
    assert_eq!(v, 0.0);
}

#[test]
fn stepped_slider_snaps() {
    let mut rig = Rig::new();
    let mut v = 0.0;
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ui.add(Slider::new(&mut v, 0.0..=10.0).step(2.5).width(300.0));
        })
    };
    rig.idle(2, &mut ui);
    rig.drag(vec2(68.0, 66.0), vec2(210.0, 66.0), &mut ui);
    assert_eq!(v, 5.0);
}

#[test]
fn toggle_by_tap_and_by_drag() {
    let mut rig = Rig::new();
    let mut on = false;
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                ui.horizontal(|ui| ui.add(Toggle::new(&mut on)));
            })
        };
        rig.idle(3, &mut ui);
        rig.click(vec2(65.0, 65.0), &mut ui);
        rig.idle(60, &mut ui);
    }
    assert!(on);
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                ui.horizontal(|ui| ui.add(Toggle::new(&mut on)));
            })
        };
        rig.drag(vec2(92.0, 65.0), vec2(40.0, 65.0), &mut ui);
    }
    assert!(!on);
}

#[test]
fn segmented_tap_drag_and_keys() {
    let mut rig = Rig::new();
    let mut sel = 0;
    let mut step = |rig: &mut Rig, act: &mut dyn FnMut(&mut Rig, Draw)| {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                ui.add(Segmented::new(&mut sel, &["A", "B", "C"]).width(300.0));
            })
        };
        act(rig, &mut ui);
        rig.idle(60, &mut ui);
        sel
    };
    step(&mut rig, &mut |r, ui| r.idle(3, ui));
    assert_eq!(step(&mut rig, &mut |r, ui| r.click(vec2(300.0, 66.0), ui)), 2);
    assert_eq!(step(&mut rig, &mut |r, ui| r.drag(vec2(300.0, 66.0), vec2(200.0, 66.0), ui)), 1);
    assert_eq!(
        step(&mut rig, &mut |r, ui| {
            r.frame(&[Event::Key(Key::Tab)], ui);
            r.frame(&[Event::Key(Key::Left)], ui);
        }),
        0
    );
}

#[test]
fn tab_bar_follows_a_drag() {
    let mut rig = Rig::new();
    let mut sel = 0;
    let mut ui = |ctx: &mut Context| {
        TabBar {
            selected: &mut sel,
            tabs: &[(Icon::Home, "A"), (Icon::Star, "B"), (Icon::Heart, "C"), (Icon::Search, "D")],
        }
        .show_at(ctx, Id::new("t"), vec2(400.0, 500.0));
    };
    rig.idle(3, &mut ui);
    rig.drag(vec2(285.0, 500.0), vec2(285.0 + 77.0 * 2.0, 500.0), &mut ui);
    assert_eq!(sel, 2);
}

#[test]
fn typing_selecting_and_replacing() {
    let mut rig = Rig::new();
    let mut s = String::new();
    let mut copied = Vec::new();
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                ui.add(TextEdit::new(&mut s).width(300.0));
            })
        };
        rig.idle(2, &mut ui);
        rig.click(vec2(120.0, 66.0), &mut ui);
        rig.frame(&[Event::Text("Hello glass".into())], &mut ui);
        let ctrl = Modifiers { ctrl: true, command: true, ..Default::default() };
        rig.frame(&[Event::Modifiers(ctrl), Event::Key(Key::Char('a')), Event::Key(Key::Char('c'))], &mut ui);
        copied.append(&mut rig.ctx.host);
        rig.frame(&[Event::Modifiers(Modifiers::default()), Event::Text("X".into())], &mut ui);
        rig.frame(&[Event::Text("YZ".into())], &mut ui);
        rig.frame(&[Event::Key(Key::Left), Event::Key(Key::Backspace)], &mut ui);
    }
    assert_eq!(s, "XZ");
    assert!(format!("{copied:?}").contains("Hello glass"));
}

#[test]
fn menu_opens_picks_and_closes() {
    let mut rig = Rig::new();
    let mut sel = 0;
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                ComboBox { selected: &mut sel, options: &["One", "Two", "Three"], width: 160.0 }.show(ui);
            })
        };
        rig.idle(2, &mut ui);
        rig.click(vec2(100.0, 66.0), &mut ui);
        rig.idle(60, &mut ui);
        // Second row of the open menu.
        rig.click(vec2(100.0, 50.0 + 8.0 + 38.0 + 18.0), &mut ui);
        rig.idle(60, &mut ui);
        // Closed again: this click must not pick anything.
        rig.click(vec2(100.0, 50.0 + 8.0 + 76.0 + 18.0), &mut ui);
        rig.idle(30, &mut ui);
    }
    assert_eq!(sel, 1);
}

#[test]
fn menu_closes_on_outside_press_and_escape() {
    let mut rig = Rig::new();
    let mut open = false;
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            let r = MenuButton::new("Menu").show(ui, |ui| {
                ui.add(MenuItem::new("Item"));
            });
            open = ui.ctx.popup_open(r.id);
        })
    };
    rig.idle(2, &mut ui);
    rig.click(vec2(70.0, 66.0), &mut ui);
    rig.idle(40, &mut ui);
    rig.click(vec2(600.0, 500.0), &mut ui);
    rig.idle(40, &mut ui);
    rig.click(vec2(70.0, 66.0), &mut ui);
    rig.idle(40, &mut ui);
    rig.frame(&[Event::Key(Key::Escape)], &mut ui);
    rig.idle(2, &mut ui);
    assert!(!open);
}

#[test]
fn scroll_moves_content_and_returns_from_the_end() {
    let mut rig = Rig::new();
    let mut y = 0.0;
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ScrollArea::new().height(200.0).show(ui, |ui| {
                for i in 0..30 {
                    let r = ui.label(&format!("Row {i}"));
                    if i == 0 {
                        y = r.rect.min.y;
                    }
                }
            });
        })
    };
    rig.idle(3, &mut ui);
    rig.frame(&[Event::PointerMoved(vec2(100.0, 120.0))], &mut ui);
    rig.frame(&[Event::Scroll(vec2(0.0, -100.0))], &mut ui);
    rig.idle(60, &mut ui);
    let _ = &ui;
    assert!((y - (50.0 - 100.0)).abs() < 1.0, "{y}");
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ScrollArea::new().height(200.0).show(ui, |ui| {
                for i in 0..30 {
                    let r = ui.label(&format!("Row {i}"));
                    if i == 0 {
                        y = r.rect.min.y;
                    }
                }
            });
        })
    };
    for _ in 0..10 {
        rig.frame(&[Event::Scroll(vec2(0.0, 500.0))], &mut ui);
    }
    rig.idle(120, &mut ui);
    assert!((y - 50.0).abs() < 0.6, "{y}");
}

#[test]
fn content_behind_a_modal_is_out_of_reach() {
    let mut rig = Rig::new();
    let (mut n, mut open) = (0, true);
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| n += ui.button("Behind").clicked() as u32);
        alert(ctx, Id::new("a"), &mut open, "Title", "Message", &["Cancel", "OK"]);
    };
    rig.idle(40, &mut ui);
    rig.click(vec2(60.0, 60.0), &mut ui);
    assert_eq!(n, 0);
}

#[test]
fn everything_comes_to_rest() {
    let mut rig = Rig::new();
    let (mut v, mut on, mut sel, mut tab) = (0.5, false, 0, 0);
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ui.button("Press");
            ui.slider(&mut v, 0.0..=1.0);
            ui.toggle(&mut on, "Toggle");
            ui.segmented(&mut sel, &["A", "B"]);
            ui.progress(0.4);
        });
        TabBar { selected: &mut tab, tabs: &[(Icon::Home, "A"), (Icon::Star, "B")] }.show_at(
            ctx,
            Id::new("t"),
            vec2(400.0, 560.0),
        );
    };
    rig.idle(2, &mut ui);
    rig.drag(vec2(120.0, 110.0), vec2(300.0, 110.0), &mut ui);
    rig.frame(&[Event::PointerMoved(vec2(790.0, 10.0))], &mut ui);
    rig.idle(240, &mut ui);
    assert!(!rig.ctx.wants_repaint(), "an idle window must not keep drawing");
}

#[test]
fn window_folds_moves_and_closes() {
    let mut rig = Rig::new();
    let mut open = true;
    let mut ui = |ctx: &mut Context| {
        Window::new("W").pos(vec2(100.0, 100.0)).width(240.0).open(&mut open).show(ctx, |ui| {
            ui.label("Body");
        });
    };
    rig.idle(60, &mut ui);
    rig.drag(vec2(220.0, 120.0), vec2(320.0, 220.0), &mut ui);
    rig.idle(60, &mut ui);
    // The close button moved along: 24 points in from the left edge.
    rig.click(vec2(224.0, 223.0), &mut ui);
    assert!(!open);
}
