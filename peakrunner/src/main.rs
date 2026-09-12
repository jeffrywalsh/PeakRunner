#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("PeakRunner"),
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
