//! Drives widgets with scripted input. No GPU needed.

use std::cell::Cell;

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
        TabBar::new(
            &mut sel,
            &[(Icon::Home, "A"), (Icon::Star, "B"), (Icon::Heart, "C"), (Icon::Search, "D")],
        )
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
        copied.append(&mut rig.ctx.host_commands);
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
                ComboBox::new(&mut sel, &["One", "Two", "Three"]).width(160.0).show(ui);
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
        TabBar::new(&mut tab, &[(Icon::Home, "A"), (Icon::Star, "B")]).show_at(
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

#[test]
fn screen_reader_sees_and_operates_widgets() {
    use rust_glass::accessibility::{Action, Role};
    let mut rig = Rig::new();
    rig.ctx.set_accessibility(true);
    let (mut n, mut v, mut on, mut text) = (0, 0.5, false, String::new());
    let ids = std::cell::RefCell::new(Vec::new());
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            let b = ui.button("Save");
            n += b.clicked() as u32;
            let s = ui.add(Slider::new(&mut v, 0.0..=1.0).step(0.1));
            ui.ctx.access_label(s.id, "Volume");
            let t = ui.toggle(&mut on, "Wi-Fi");
            let f = ui.text_edit(&mut text, "Name");
            *ids.borrow_mut() = vec![b.id, s.id, t.id, f.id];
        })
    };
    rig.idle(2, &mut ui);
    let nodes = rig.ctx.access_nodes().to_vec();
    let find = |role: Role| nodes.iter().find(|n| n.role == role).unwrap().clone();
    assert_eq!(find(Role::Button).label, "Save");
    assert_eq!(find(Role::Slider).label, "Volume");
    assert_eq!(find(Role::Switch).toggled, Some(false));
    assert!(find(Role::TextInput).focusable);
    let ids = ids.borrow().clone();
    let (b, s, t, f) = (ids[0], ids[1], ids[2], ids[3]);
    rig.frame(&[Event::Access(b, Action::Click)], &mut ui);
    rig.frame(&[Event::Access(s, Action::Increment)], &mut ui);
    rig.frame(&[Event::Access(t, Action::Click)], &mut ui);
    rig.frame(&[Event::Access(f, Action::SetValue("Ada".into()))], &mut ui);
    rig.idle(2, &mut ui);
    let tree = rig.ctx.access_update("Test");
    let _ = &ui;
    assert_eq!(n, 1);
    assert!((v - 0.6).abs() < 1e-5, "{v}");
    assert!(on);
    assert_eq!(text, "Ada");
    assert!(tree.nodes.len() >= 5);
}

#[test]
fn no_tree_is_built_without_a_screen_reader() {
    let mut rig = Rig::new();
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ui.button("Save");
        })
    };
    rig.idle(2, &mut ui);
    assert!(rig.ctx.access_nodes().is_empty());
}

fn cards(ui: &mut Ui, n: usize, first: &Cell<f32>) {
    for i in 0..n {
        let r = ui.button(&format!("Card {i}"));
        if i == 0 {
            first.set(r.rect.min.x);
        }
    }
}

#[test]
fn horizontal_scroll_takes_sideways_input_and_leaves_vertical_to_the_page() {
    let mut rig = Rig::new();
    let (xc, yc) = (Cell::new(0.0), Cell::new(0.0));
    let (x, y) = (|| xc.get(), || yc.get());
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ScrollArea::new().id("page").height(300.0).show(ui, |ui| {
                yc.set(ui.label("Top").rect.min.y);
                ScrollArea::horizontal().id("row").height(60.0).show(ui, |ui| cards(ui, 40, &xc));
                for i in 0..30 {
                    ui.label(&format!("Row {i}"));
                }
            });
        })
    };
    rig.idle(3, &mut ui);
    let (x0, y0) = (x(), y());
    // Over the row: a sideways swipe moves it, a vertical wheel moves the page.
    rig.frame(&[Event::PointerMoved(vec2(200.0, 100.0))], &mut ui);
    rig.frame(&[Event::Scroll(vec2(-150.0, 0.0))], &mut ui);
    rig.idle(60, &mut ui);
    assert!((x() - (x0 - 150.0)).abs() < 1.0, "{x0} {}", x());
    assert!((y() - y0).abs() < 0.5);
    // Shift turns the wheel sideways.
    let shift = Modifiers { shift: true, ..Default::default() };
    rig.frame(&[Event::Modifiers(shift), Event::Scroll(vec2(0.0, -100.0))], &mut ui);
    rig.frame(&[Event::Modifiers(Modifiers::default())], &mut ui);
    rig.idle(60, &mut ui);
    assert!((x() - (x0 - 250.0)).abs() < 1.0, "{x0} {}", x());
    rig.frame(&[Event::Scroll(vec2(0.0, -80.0))], &mut ui);
    rig.idle(60, &mut ui);
    let _ = &ui;
    assert!((y() - (y0 - 80.0)).abs() < 1.0, "{y0} {}", y());
    assert!((x() - (x0 - 250.0)).abs() < 1.0);
}

#[test]
fn long_lists_lay_out_only_visible_rows() {
    let mut rig = Rig::new();
    let cell = std::cell::RefCell::new(0..0);
    let drawn = || cell.borrow().clone();
    let mut ui = |ctx: &mut Context| {
        page(ctx, |ui| {
            ScrollArea::new().height(300.0).show_rows(ui, 24.0, 100_000, |ui, rows| {
                *cell.borrow_mut() = rows.clone();
                for i in rows {
                    ui.label(&format!("Row {i}"));
                }
            });
        })
    };
    rig.idle(3, &mut ui);
    assert_eq!(drawn().start, 0);
    assert!(drawn().len() < 20, "{:?}", drawn());
    rig.frame(&[Event::PointerMoved(vec2(200.0, 200.0))], &mut ui);
    for _ in 0..50 {
        rig.frame(&[Event::Scroll(vec2(0.0, -2000.0))], &mut ui);
    }
    rig.idle(90, &mut ui);
    let _ = &ui;
    let stride = 24.0 + Theme::default().spacing;
    assert!(drawn().start as f32 >= 90_000.0 / stride, "{:?}", drawn());
    assert!(drawn().len() < 20);
}

#[test]
fn drag_and_drop_moves_a_payload() {
    let mut rig = Rig::new();
    let mut left = vec!["a".to_string(), "b".to_string()];
    let mut right: Vec<String> = Vec::new();
    let second = Cell::new(Vec2::ZERO);
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(200.0, |ui| {
                        for (i, s) in left.iter().enumerate() {
                            let r = ui.drag_source(("item", s.as_str()), i, |ui| {
                                ui.button(s);
                            });
                            if i == 1 {
                                second.set(r.rect.center());
                            }
                        }
                    });
                    let (_, got) = ui.drop_zone::<usize, _>("right", vec2(200.0, 200.0), |ui| {
                        for s in &right {
                            ui.label(s);
                        }
                    });
                    if let Some(i) = got {
                        right.push(left.remove(i));
                    }
                });
            })
        };
        rig.idle(3, &mut ui);
        // Second item of the left column onto the right zone.
        rig.drag(second.get(), vec2(400.0, 120.0), &mut ui);
        rig.idle(30, &mut ui);
    }
    assert_eq!(left, ["a"]);
    assert_eq!(right, ["b"]);
}

#[test]
fn reorder_moves_an_item_down() {
    let mut rig = Rig::new();
    let mut items = vec![1, 2, 3, 4];
    let rects = std::cell::RefCell::new(Vec::new());
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                rects.borrow_mut().clear();
                ui.reorder("list", &mut items, |ui, i| {
                    let r = ui.button(&format!("Item {i}")).rect;
                    rects.borrow_mut().push(r);
                });
            })
        };
        rig.idle(3, &mut ui);
        let (a, b) = (rects.borrow()[0].center(), rects.borrow()[2].max);
        rig.drag(a, vec2(a.x, b.y - 4.0), &mut ui);
        rig.idle(10, &mut ui);
    }
    assert_eq!(items, [2, 3, 1, 4]);
}

#[test]
fn undo_and_redo_in_a_text_field() {
    let mut rig = Rig::new();
    let mut s = String::new();
    let ctrl = Modifiers { ctrl: true, command: true, ..Default::default() };
    let ctrl_shift = Modifiers { shift: true, ..ctrl };
    let none = Modifiers::default();
    let mut states = Vec::new();
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                ui.add(TextEdit::new(&mut s).width(300.0));
            })
        };
        rig.idle(2, &mut ui);
        rig.click(vec2(120.0, 66.0), &mut ui);
        for c in ["o", "n", "e", " ", "t", "w", "o", "o"] {
            rig.frame(&[Event::Text(c.into())], &mut ui);
        }
        rig.frame(&[Event::Key(Key::Backspace)], &mut ui);
        let z = Key::Char('z');
        for m in [ctrl, ctrl, ctrl, ctrl, ctrl_shift, ctrl_shift] {
            rig.frame(&[Event::Modifiers(m), Event::Key(z), Event::Modifiers(none)], &mut ui);
            rig.ctx.set_accessibility(true);
            rig.frame(&[], &mut ui);
            states.push(rig.ctx.access_nodes().iter().find_map(|n| n.value.clone()).unwrap_or_default());
            rig.ctx.set_accessibility(false);
        }
    }
    // The backspace is a step of its own, typing undoes a word at a time.
    assert_eq!(states, ["one twoo", "one ", "", "", "one ", "one twoo"]);
}

#[test]
fn input_method_composes_before_it_commits() {
    let mut rig = Rig::new();
    let mut s = String::from("ab");
    let mut changed = 0;
    {
        let mut ui = |ctx: &mut Context| {
            page(ctx, |ui| {
                changed += ui.add(TextEdit::new(&mut s).width(300.0)).changed() as u32;
            })
        };
        rig.idle(2, &mut ui);
        rig.click(vec2(300.0, 66.0), &mut ui);
        rig.frame(&[Event::Preedit("にほ".into(), Some((6, 6)))], &mut ui);
        // Keys belong to the input method while it composes.
        rig.frame(&[Event::Key(Key::Backspace)], &mut ui);
        assert!(rig.ctx.ime_area.is_some(), "the caret is reported to the input method");
        rig.frame(&[Event::Preedit(String::new(), None), Event::Text("日本".into())], &mut ui);
    }
    assert_eq!(s, "ab日本");
    assert_eq!(changed, 1);
}
