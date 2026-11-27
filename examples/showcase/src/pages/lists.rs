use rust_glass::accessibility::Role;
use rust_glass::prelude::*;

use super::{Marks, hint, section};

const TILES: [(&str, Color, Color); 8] = [
    ("Coast", Color::TEAL, Color::BLUE),
    ("Dune", Color::ORANGE, Color::PINK),
    ("Moss", Color::GREEN, Color::TEAL),
    ("Dusk", Color::INDIGO, Color::PINK),
    ("Ember", Color::RED, Color::ORANGE),
    ("Lagoon", Color::BLUE, Color::GREEN),
    ("Iris", Color::PURPLE, Color::INDIGO),
    ("Citrus", Color::YELLOW, Color::GREEN),
];

const TASK_HEIGHT: f32 = 40.0;

pub struct State {
    tasks: Vec<String>,
    /// Two lists, "To do" and "Done", that items move between.
    boards: [Vec<String>; 2],
}

impl Default for State {
    fn default() -> Self {
        let strings = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        Self {
            tasks: strings(&["Pick up the glass", "Sand the edges", "Polish", "Mount the frame", "Hang it"]),
            boards: [
                strings(&["Write the release notes", "Record the clips", "Test on Linux"]),
                strings(&["Accessibility tree"]),
            ],
        }
    }
}

pub fn show(state: &mut State, marks: &mut Marks, ui: &mut Ui) {
    section(ui, "Horizontal", |ui| {
        hint(ui, "Swipe sideways, or hold Shift while turning the wheel.");
        ScrollArea::horizontal().id("tiles").height(140.0).fade_sides(28.0, 28.0).show(ui, |ui| {
            for i in 0..24 {
                let (name, top, bottom) = TILES[i % TILES.len()];
                let rect = ui.allocate(vec2(170.0, 118.0));
                let style = TextStyle::new(15.0).weight(Weight::Semibold);
                let label = ui.layout_text(&format!("{name} {}", i + 1), &style);
                let mut painter = ui.painter();
                painter.shadow(rect.translate(vec2(0.0, 4.0)), 22.0, 14.0, Color::black(0.18));
                painter.gradient(rect, 22.0, top, bottom, vec2(0.6, 1.0));
                painter.text(
                    rect.min + vec2(16.0, rect.height() - 16.0 - label.size.y),
                    &label,
                    Color::WHITE,
                );
            }
        });
    });

    section(ui, "Both directions", |ui| {
        ScrollArea::both().id("sheet").height(200.0).content_width(1290.0).show(ui, |ui| {
            ui.spacing = 0.0;
            for row in 0..40 {
                ui.horizontal(|ui| {
                    ui.spacing = 0.0;
                    for column in 0..15 {
                        let rect = ui.allocate(vec2(86.0, 30.0));
                        let name = format!("{}{}", (b'A' + column as u8) as char, row + 1);
                        let label = ui.layout_text(&name, &TextStyle::new(13.0));
                        let (text, line) = (ui.surface.secondary_color(), ui.surface.separator_color());
                        let mut painter = ui.painter();
                        painter.fill(
                            Rect { min: vec2(rect.max.x - 1.0, rect.min.y), max: rect.max },
                            0.0,
                            line,
                        );
                        painter.fill(
                            Rect { min: vec2(rect.min.x, rect.max.y - 1.0), max: rect.max },
                            0.0,
                            line,
                        );
                        painter.text(rect.center() - label.size * 0.5, &label, text);
                    }
                });
            }
        });
    });

    section(ui, "100 000 rows", |ui| {
        hint(ui, "Only the rows in view are laid out, so a long list costs as much as a short one.");
        let (_, rect) = ui.scope(|ui| {
            ScrollArea::new().id("long").height(220.0).show_rows(ui, 32.0, 100_000, |ui, rows| {
                for i in rows {
                    let width = ui.available_width() - 12.0;
                    let rect = ui.allocate(vec2(width, 32.0));
                    let (fill, text, secondary) =
                        (ui.surface.fill_color(), ui.surface.text_color(), ui.surface.secondary_color());
                    let title = ui
                        .layout_text(&format!("Row {}", i + 1), &TextStyle::new(14.0).weight(Weight::Medium));
                    let code = ui.layout_text(&format!("#{:05X}", i * 7919 % 0xFFFFF), &TextStyle::new(13.0));
                    let mut painter = ui.painter();
                    if i % 2 == 0 {
                        painter.fill(rect, 9.0, fill);
                    }
                    painter.text(vec2(rect.min.x + 12.0, rect.center().y - title.size.y * 0.5), &title, text);
                    let right = vec2(rect.max.x - 12.0 - code.size.x, rect.center().y - code.size.y * 0.5);
                    painter.text(right, &code, secondary);
                }
            });
        });
        marks.set("long list", rect);
    });

    section(ui, "Reorder", |ui| {
        hint(ui, "Drag a task to a new place. Escape puts it back.");
        ui.spacing = 6.0;
        let (_, rect) = ui.scope(|ui| {
            ui.reorder("tasks", &mut state.tasks, |ui, task| {
                let width = ui.available_width();
                task_row(ui, task, width);
            });
        });
        marks.set("task list", rect);
    });

    section(ui, "Drag between lists", |ui| {
        let width = ((ui.available_width() - 16.0) * 0.5).max(120.0);
        let mut moved = None;
        ui.horizontal(|ui| {
            for (board, name) in ["To do", "Done"].iter().enumerate() {
                ui.vertical(width, |ui| {
                    ui.strong(name);
                    let items = &state.boards[board];
                    let ((_, dropped), rect) = ui.scope(|ui| {
                        ui.drop_zone::<(usize, usize), _>(("board", board), vec2(width, 150.0), |ui| {
                            ui.spacing = 6.0;
                            for (i, task) in items.iter().enumerate() {
                                // The same id in both lists, so a card lands
                                // where it is let go.
                                let id = Id::new(("card", task.as_str()));
                                ui.drag_source_id(id, (board, i), |ui| task_row(ui, task, width));
                            }
                        })
                    });
                    marks.set(if board == 0 { "todo list" } else { "done list" }, rect);
                    if let Some((from, i)) = dropped
                        && from != board
                    {
                        moved = Some((from, i, board));
                    }
                });
            }
        });
        if let Some((from, i, to)) = moved {
            let task = state.boards[from].remove(i);
            state.boards[to].push(task);
        }
    });
}

/// A task card with a grip at its end.
fn task_row(ui: &mut Ui, task: &str, width: f32) {
    let rect = ui.allocate(vec2(width, TASK_HEIGHT));
    let (fill, text, grip) = (ui.surface.fill_color(), ui.surface.text_color(), ui.surface.tertiary_color());
    let label = ui.layout_text(task, &TextStyle::new(15.0));
    let mut painter = ui.painter();
    painter.fill(rect, 12.0, fill);
    for k in 0..3 {
        let y = rect.center().y - 5.0 + 5.0 * k as f32;
        painter.line(vec2(rect.max.x - 30.0, y), vec2(rect.max.x - 16.0, y), 1.6, grip);
    }
    painter.text(vec2(rect.min.x + 14.0, rect.center().y - label.size.y * 0.5), &label, text);
    if let Some(node) = ui.access(ui.make_id(("task", task)), Role::Label, rect) {
        node.label(task);
    }
}
