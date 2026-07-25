//! Entry points for the read-only save viewer: a native window, or a web/WASM
//! canvas (served by `docker compose up`). Both construct the same `bl2_gui::App`.

// ---- native ----
/// Our original app icon: a gold hexagon (matches the in-app emblem) on
/// transparency. Generated in code — no bundled art. See ASSETS.md.
#[cfg(not(target_arch = "wasm32"))]
fn app_icon() -> eframe::egui::IconData {
    const SIZE: u32 = 64;
    let (c, r) = (SIZE as f32 / 2.0, SIZE as f32 * 0.44);
    let gold = [0xF5u8, 0xB1, 0x1E, 0xFF];
    let ink = [0x12u8, 0x11, 0x0E, 0xFF];
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let (ax, ay) = (dx.abs(), dy.abs());
            // pointy-top regular hexagon (circumradius r): |x| <= √3/2·r and
            // |y| + |x|/√3 <= r.
            let hex = ax <= 0.866_025 * r && ay + 0.577_35 * ax <= r;
            // inner dark diamond, echoing the in-app emblem.
            let diamond = ax + ay <= r * 0.42;
            let i = ((y * SIZE + x) * 4) as usize;
            if diamond {
                rgba[i..i + 4].copy_from_slice(&ink);
            } else if hex {
                rgba[i..i + 4].copy_from_slice(&gold);
            }
        }
    }
    eframe::egui::IconData { rgba, width: SIZE, height: SIZE }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([920.0, 700.0])
            .with_min_inner_size([560.0, 420.0])
            .with_title("BL2 Save Editor")
            .with_icon(std::sync::Arc::new(app_icon())),
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
