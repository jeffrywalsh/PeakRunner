//! Launch the real client renderer and save its window, then close only this test.
//! QA_CAPTURE_PATH must be set. Optional PEAKRUNNER_JOIN exercises a live match.
//! Optional QA_CLICKS="x,y@seconds;..." clicks window points at those times (for
//! example a map, a difficulty and Start match), and QA_CAPTURE_AT delays the
//! capture from the default 8 s so an offline bot match can develop.
use std::time::Instant;
use eframe::egui;
struct Probe { app: peakrunner::PeakRunnerApp, started: Instant, requested: bool, saved: bool, paused: bool, offline_stage:u8, clicked:usize }
impl eframe::App for Probe {
    fn raw_input_hook(&mut self, ctx:&egui::Context, input:&mut egui::RawInput) {
        let clicks:Vec<(f32,f32,f32)>=std::env::var("QA_CLICKS").unwrap_or_default().split(';').filter_map(|c| {
            let (xy,t)=c.split_once('@')?;let (x,y)=xy.split_once(',')?;
            Some((x.trim().parse().ok()?,y.trim().parse().ok()?,t.trim().parse().ok()?))
        }).collect();
        if let Some(&(x,y,t))=clicks.get(self.clicked) {
            if self.started.elapsed().as_secs_f32()>=t {
                self.clicked+=1;
                let pos=egui::pos2(x,y);
                input.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true,false] {input.events.push(egui::Event::PointerButton {
                    pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()
                });}
            }
        }
        if std::env::var_os("QA_STONEHENGE_PLAY").is_some() || std::env::var_os("QA_COLLECTION_PLAY").is_some() {
            let seconds=self.started.elapsed().as_secs_f32();
            let target=if self.offline_stage==0 && seconds>=2. {
                let x=match std::env::var("QA_COLLECTION_PLAY").as_deref() {
                    Ok("snowblind-clone")=>300.,Ok("desert-of-death-clone")=>480.,_=>120.
                };
                Some(egui::pos2(x,ctx.content_rect().bottom()-216.))
            } else if self.offline_stage==1 && seconds>=4. {
                Some(egui::pos2(90.,ctx.content_rect().bottom()-142.))
            } else {None};
            if let Some(pos)=target {
                self.offline_stage+=1;
                input.events.push(egui::Event::PointerMoved(pos));
                for pressed in [true,false] {input.events.push(egui::Event::PointerButton {
                    pos,button:egui::PointerButton::Primary,pressed,modifiers:Default::default()
                });}
            }
        }
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
        let capture_at=std::env::var("QA_CAPTURE_AT").ok().and_then(|v|v.parse::<f32>().ok()).unwrap_or(8.);
        if self.started.elapsed().as_secs_f32()>=capture_at && !self.requested {
            self.requested=true;ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
        }
        assert!(self.saved || self.started.elapsed().as_secs_f32()<capture_at+82.,"client screenshot timed out");
        ctx.request_repaint();
    }
}
fn main()->eframe::Result {
    std::env::var("QA_CAPTURE_PATH").expect("set QA_CAPTURE_PATH to a PNG output path");
    env_logger::init();
    let _local = if std::env::var_os("QA_LOCAL").is_some() {
        let map = std::env::var("QA_MAP").unwrap_or_else(|_| "Raindance".into());
        let mut host = peakrunner_server::GameHost::bind("127.0.0.1:0", "HUD QA", 8, &map).unwrap();
        // QA_MODE=football: the local server runs Football on QA_MAP (a map
        // with a declared field, or a preview pack through PEAKRUNNER_MAP_PACK).
        if std::env::var("QA_MODE").is_ok_and(|m| m == "football") {
            host = host.with_rotation(&format!(r#"[{{"map":"{}","mode":"football"}}]"#, map.to_lowercase())).unwrap();
        }
        // QA_MODE=cnh_server: the local server runs Capture & Hold on QA_MAP.
        if std::env::var("QA_MODE").is_ok_and(|m| m == "cnh_server") {
            host = host.with_rotation(&format!(r#"[{{"map":"{}","mode":"capture_and_hold"}}]"#, map.to_lowercase())).unwrap();
        }
        // QA_MODE=team_deathmatch: the local server runs it on QA_MAP.
        if let Ok(m) = std::env::var("QA_MODE").map(|m| m.to_lowercase()) {
            if m == "team_deathmatch" {
                host = host.with_rotation(&format!(r#"[{{"map":"{}","mode":"{m}"}}]"#, map.to_lowercase())).unwrap();
            }
        }
        std::env::set_var("PEAKRUNNER_JOIN", host.local_addr().to_string());
        Some(host.spawn())
    } else { None };
    eframe::run_native("PeakRunner platform check",eframe::NativeOptions {
        viewport:egui::ViewportBuilder::default().with_inner_size([1280.,800.]),
        renderer:eframe::Renderer::Wgpu,..Default::default()
    },Box::new(|cc|Ok(Box::new(Probe {app:peakrunner::PeakRunnerApp::new(cc).map_err(std::io::Error::other)?,started:Instant::now(),requested:false,saved:false,paused:false,offline_stage:0,clicked:0}))))
}
