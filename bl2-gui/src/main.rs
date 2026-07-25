//! Entry points for the read-only save viewer: a native window, or a web/WASM
//! canvas (served by `docker compose up`). Both construct the same `bl2_gui::App`.

// ---- native ----
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([920.0, 700.0])
            .with_min_inner_size([560.0, 420.0])
            .with_title("BL2 Save Editor"),
        ..Default::default()
    };
    eframe::run_native(
        "BL2 Save Editor",
        native_options,
        Box::new(|cc| Ok(Box::new(bl2_gui::App::new(cc)))),
    )
}

// ---- web (compiled to WASM by trunk) ----
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    // Route `log` to the browser console.
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("missing the_canvas_id element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("the_canvas_id is not a <canvas>");

        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(bl2_gui::App::new(cc)))),
            )
            .await;

        // Remove the "Loading…" text once we're up (or report a crash).
        if let Some(loading) = document.get_element_by_id("loading_text") {
            match result {
                Ok(_) => loading.remove(),
                Err(e) => {
                    loading.set_inner_html("<p>The app crashed — see the browser console.</p>");
                    panic!("failed to start eframe: {e:?}");
                }
            }
        }
    });
}
