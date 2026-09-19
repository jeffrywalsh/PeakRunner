//! Launch the real client renderer and save its window, then close only this test.
//! QA_CAPTURE_PATH must be set. Optional PEAKRUNNER_JOIN exercises a live match.
use std::time::Instant;
use eframe::egui;
struct Probe { app: peakrunner::PeakRunnerApp, started: Instant, requested: bool, saved: bool, paused: bool }
impl eframe::App for Probe {
    fn raw_input_hook(&mut self, ctx:&egui::Context, input:&mut egui::RawInput) {
        if std::env::var_os("QA_SYSTEM_THEME_LIGHT").is_some() {
            input.system_theme=Some(egui::Theme::Light);
        }
        if !self.paused && std::env::var_os("QA_BROWSER").is_some() && self.started.elapsed().as_secs()>=2 {
            self.paused=true;
            // Deterministic 1280x800 menu layout: activate the real Find match
            // button, rather than changing private app state for a screenshot.
            let pos=egui::pos2(90.,ctx.content_rect().bottom()-97.);
                input.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true,false] {input.events.push(egui::Event::PointerButton {
                    pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()
                });}
        }
    }
    fn logic(&mut self, ctx:&egui::Context, frame:&mut eframe::Frame) {
        if std::env::var_os("QA_SCOREBOARD").is_some() && self.started.elapsed().as_secs() >= 4 {
            ctx.input_mut(|i| { i.keys_down.insert(egui::Key::Tab); });
        }
        let chat = std::env::var("QA_CHAT").ok();
        if !self.paused && self.started.elapsed().as_secs() >= 4 && (std::env::var_os("QA_PAUSE").is_some() || chat.is_some()) {
            self.paused = true;
            let key = match chat.as_deref() { Some("team")=>egui::Key::Y, Some(_)=>egui::Key::T, None=>egui::Key::Escape };
            ctx.input_mut(|i| i.events.push(egui::Event::Key { key,
                physical_key:None, pressed:true, repeat:false, modifiers:Default::default() }));
        }
        self.app.logic(ctx,frame);
    }
    fn ui(&mut self, ui:&mut egui::Ui, frame:&mut eframe::Frame) {
        self.app.ui(ui,frame);
        let ctx=ui.ctx();
        let screenshot=ctx.input(|i|i.events.iter().find_map(|e|match e {
            egui::Event::Screenshot {image,..}=>Some(image.clone()), _=>None,
        }));
        if let Some(image)=screenshot {
            assert!(image.pixels.iter().any(|p|p.r()>100),"blank client render");
            let path=std::env::var("QA_CAPTURE_PATH").expect("capture path");
            let file=std::fs::File::create(path).expect("capture output");
            let mut encoder=png::Encoder::new(file,image.width() as u32,image.height() as u32);
            encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);
            let pixels:Vec<u8>=image.pixels.iter().flat_map(|p|p.to_array()).collect();
            encoder.write_header().unwrap().write_image_data(&pixels).unwrap();
            self.saved=true;println!("PASS: real client window rendered and captured");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.started.elapsed().as_secs()>=8 && !self.requested {
            self.requested=true;ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        assert!(self.saved || self.started.elapsed().as_secs()<90,"client screenshot timed out");
        ctx.request_repaint();
    }
}
fn main()->eframe::Result {
    std::env::var("QA_CAPTURE_PATH").expect("set QA_CAPTURE_PATH to a PNG output path");
    env_logger::init();
    let _local = if std::env::var_os("QA_LOCAL").is_some() {
        let host = peakrunner_server::GameHost::bind("127.0.0.1:0", "HUD QA", 8, "Raindance").unwrap();
        std::env::set_var("PEAKRUNNER_JOIN", host.local_addr().to_string());
        Some(host.spawn())
    } else { None };
    eframe::run_native("PeakRunner platform check",eframe::NativeOptions {
        viewport:egui::ViewportBuilder::default().with_inner_size([1280.,800.]),
        renderer:eframe::Renderer::Wgpu,..Default::default()
    },Box::new(|cc|Ok(Box::new(Probe {app:peakrunner::PeakRunnerApp::new(cc).map_err(std::io::Error::other)?,started:Instant::now(),requested:false,saved:false,paused:false}))))
}
