//! Headless verification of the programmatic "request focus + select all text"
//! pattern used by the cue-properties text fields.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Container {
    Plain,
    Grid,
    Scroll,
    GridScroll,
}

fn run_frame(ctx: &egui::Context, request_focus_before: bool, container: Container) -> bool {
    let mut focused = false;
    let id = egui::Id::new("test_edit");
    ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut draw = |ui: &mut egui::Ui| {
                if request_focus_before {
                    ui.memory_mut(|m| m.request_focus(id));
                }
                let mut text = String::from("Hello world");
                let resp = ui.add(egui::TextEdit::singleline(&mut text).id(id));
                focused = resp.gained_focus();
                if let Some(mut state) = egui::TextEdit::load_state(ctx, id) {
                    state.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(text.len()),
                    )));
                    egui::TextEdit::store_state(ctx, id, state);
                }
            };
            match container {
                Container::Plain => draw(ui),
                Container::Grid => {
                    egui::Grid::new("g").num_columns(1).show(ui, |ui| {
                        draw(ui);
                        ui.end_row();
                    });
                }
                Container::Scroll => {
                    egui::ScrollArea::vertical().show(ui, |ui| draw(ui));
                }
                Container::GridScroll => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        egui::Grid::new("gs").num_columns(1).show(ui, |ui| {
                            draw(ui);
                            ui.end_row();
                        });
                    });
                }
            }
        });
    });
    focused
}

fn get_selection(ctx: &egui::Context) -> Option<(usize, usize)> {
    let id = egui::Id::new("test_edit");
    let mut sel = None;
    ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let mut text = String::from("Hello world");
            let _resp = ui.add(egui::TextEdit::singleline(&mut text).id(id));
            sel = egui::TextEdit::load_state(ctx, id)
                .and_then(|s| s.cursor.char_range())
                .map(|r| (r.primary.index, r.secondary.index));
        });
    });
    sel
}

#[test]
fn focus_and_select_all_work_in_all_containers() {
    for container in [
        Container::Plain,
        Container::Grid,
        Container::Scroll,
        Container::GridScroll,
    ] {
        let ctx = egui::Context::default();
        let gained = run_frame(&ctx, true, container);
        let sel = get_selection(&ctx);
        eprintln!("{container:?}: gained_focus={gained} selection={sel:?}");
        assert!(gained, "{container:?}: widget should gain focus on the request frame");
        assert_eq!(sel, Some((11, 0)), "{container:?}: text should be fully selected");
    }
}
