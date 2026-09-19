#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    // Hold a separate launcher-session lock even if the launcher itself exits.
    // Managed installs are immutable; no update may race a running managed game.
    let _launcher_lock = if let Some(path) = std::env::var_os("PEAKRUNNER_LAUNCHER_LOCK") {
        let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)
            .map_err(|e| eframe::Error::AppCreation(Box::new(e)))?;
        file.try_lock().map_err(|e| eframe::Error::AppCreation(Box::new(std::io::Error::other(e.to_string()))))?;
        Some(file)
    } else { None };
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title(if std::env::var_os("PEAKRUNNER_MAP_PACK").is_some() {"PeakRunner — Raindance playtest"} else {"PeakRunner"}),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "PeakRunner",
        native_options,
        Box::new(|cc| {
            peakrunner::PeakRunnerApp::new(cc)
                .map(|app| Box::new(app) as Box<dyn eframe::App>)
                .map_err(|err| -> Box<dyn std::error::Error + Send + Sync> {
                    std::io::Error::other(err).into()
                })
        }),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("document");
        let canvas = document
            .get_element_by_id("peakrunner_canvas")
            .expect("canvas #peakrunner_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("canvas element");
        let start = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    peakrunner::PeakRunnerApp::new(cc)
                        .map(|app| Box::new(app) as Box<dyn eframe::App>)
                        .map_err(|err| -> Box<dyn std::error::Error + Send + Sync> {
                            std::io::Error::other(err).into()
                        })
                }),
            )
            .await;
        if let Err(err) = start {
            log::error!("PeakRunner failed to start: {err:?}");
        }
    });
}
