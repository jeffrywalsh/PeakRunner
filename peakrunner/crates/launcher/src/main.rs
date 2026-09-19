#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]
use eframe::egui::{self, Color32, RichText, Vec2};
use peakrunner_launcher::{self as updater, Release, Store};
use std::{
    process::Child,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

const BG: Color32 = Color32::from_rgb(11, 18, 23);
const PANEL: Color32 = Color32::from_rgb(20, 31, 39);
const TEXT: Color32 = Color32::from_rgb(230, 239, 242);
const MUTED: Color32 = Color32::from_rgb(153, 175, 185);
const ACCENT: Color32 = Color32::from_rgb(117, 213, 228);

#[derive(serde::Deserialize)]
struct Directory {
    servers: Vec<Host>,
}
#[derive(serde::Deserialize)]
struct Host {
    name: String,
    map: String,
    players: u32,
    max_players: u32,
}
enum Event {
    Progress(String),
    Checked(Option<Release>, Vec<Host>, String),
    Done(std::result::Result<(), String>),
    Started(std::result::Result<Child, String>),
}
pub(crate) struct Launcher {
    store: Option<Arc<Store>>,
    installed: Option<String>,
    available: Option<Release>,
    hosts: Vec<Host>,
    status: String,
    busy: bool,
    child: Option<Child>,
    tx: mpsc::Sender<Event>,
    rx: mpsc::Receiver<Event>,
    checked: Instant,
    rollback_confirm: bool,
}
impl Launcher {
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::ThemePreference::Dark);
        let mut style = (*cc.egui_ctx.style_of(egui::Theme::Dark)).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = BG;
        style.visuals.override_text_color = Some(TEXT);
        style.spacing.item_spacing = Vec2::new(12., 12.);
        cc.egui_ctx.set_style_of(egui::Theme::Dark, style);
        let (tx, rx) = mpsc::channel();
        let store = updater::data_dir()
            .and_then(|root| Store::open(root, updater::public_key()?, updater::platform().into()))
            .map(Arc::new);
        let (store, status) = match store {
            Ok(s) => (Some(s), "Checking for updates…".into()),
            Err(e) => (None, e.to_string()),
        };
        let mut app = Self {
            store,
            installed: None,
            available: None,
            hosts: Vec::new(),
            status,
            busy: false,
            child: None,
            tx,
            rx,
            checked: Instant::now(),
            rollback_confirm: false,
        };
        app.refresh_installed();
        if app.store.is_some() {
            app.check();
        }
        app
    }
    fn refresh_installed(&mut self) {
        if let Some(store) = &self.store {
            match store.current() {
                Ok(current) => self.installed = current.map(|c| c.release.manifest.version),
                Err(e) => {
                    self.installed = None;
                    self.status = format!("Installation metadata needs attention: {e}");
                }
            }
        }
    }
    fn check(&mut self) {
        if self.busy || self.child.is_some() {
            return;
        }
        let Some(store) = self.store.clone() else {
            return;
        };
        self.busy = true;
        self.checked = Instant::now();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let release = updater::latest().and_then(|r| {
                store.check_sequence(&r)?;
                Ok(r)
            });
            let hosts = updater::fetch("https://dir.peakrunner.net/servers", 256 * 1024)
                .and_then(|data| Ok(serde_json::from_slice::<Directory>(&data)?.servers))
                .map(|s| {
                    s.into_iter()
                        .filter(|s| {
                            s.name.len() <= 64
                                && s.map.len() <= 64
                                && s.players <= s.max_players
                                && s.max_players <= 128
                        })
                        .take(64)
                        .collect()
                })
                .unwrap_or_default();
            let (release,status)=match release {
                Ok(r)=>(Some(r),"Update signature verified. Ready.".into()),
                Err(e)=>(store.current().ok().flatten().map(|installed|installed.release),format!("Update check unavailable: {e}. A verified installed game can still be played.")),
            };
            let _ = tx.send(Event::Checked(release, hosts, status));
        });
    }
    fn install(&mut self) {
        let (Some(store), Some(release)) = (self.store.clone(), self.available.clone()) else {
            return;
        };
        self.busy = true;
        self.status = "Preparing verified update…".into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let result = store
                .install(
                    &release,
                    |asset| {
                        updater::fetch(
                            &format!("{}/blobs/{}", updater::ORIGIN, asset.sha256),
                            asset.size,
                        )
                    },
                    |s| {
                        let _ = tx.send(Event::Progress(s));
                    },
                )
                .map_err(|e| e.to_string());
            let _ = tx.send(Event::Done(result));
        });
    }
    fn play(&mut self) {
        let Some(store) = self.store.clone() else {
            return;
        };
        self.busy = true;
        self.status = "Verifying installed files…".into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Started(store.play().map_err(|e| e.to_string())));
        });
    }
    fn rollback(&mut self) {
        let Some(store) = self.store.clone() else {
            return;
        };
        self.busy = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Event::Done(store.rollback().map_err(|e| e.to_string())));
        });
    }
}
impl eframe::App for Launcher {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.painter().rect_filled(ui.max_rect(), 0., BG);
        while let Ok(event) = self.rx.try_recv() {
            match event {
                Event::Progress(s) => self.status = s,
                Event::Checked(release, hosts, status) => {
                    self.available = release;
                    self.hosts = hosts;
                    self.status = status;
                    self.busy = false;
                }
                Event::Done(result) => {
                    self.busy = false;
                    self.status = result.map_or_else(
                        |e| format!("Update failed: {e}"),
                        |_| "Ready. Installation verified.".into(),
                    );
                    self.refresh_installed();
                }
                Event::Started(result) => {
                    self.busy = false;
                    match result {
                        Ok(child) => {
                            self.child = Some(child);
                            self.status =
                                "Game running. Updates are paused until you quit the game.".into();
                        }
                        Err(e) => self.status = format!("Could not start: {e}"),
                    }
                }
            }
        }
        if let Some(child) = &mut self.child {
            match child.try_wait() {
                Ok(Some(exit)) => {
                    self.child = None;
                    self.status = if exit.success() {
                        "Game closed. Ready for your next match.".into()
                    } else {
                        "Game exited unexpectedly. Try Repair or Rollback; details are in game.log."
                            .into()
                    };
                }
                Err(e) => self.status = format!("Could not check game process: {e}"),
                _ => {}
            }
        }
        if !self.busy && self.child.is_none() && self.checked.elapsed() > Duration::from_secs(300) {
            self.check();
        }
        if self.busy && ui.ctx().input(|i| i.viewport().close_requested()) {
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        ui.ctx().request_repaint_after(Duration::from_millis(200));
        egui::Frame::new().fill(BG).inner_margin(28).show(ui,|ui| {
            ui.horizontal(|ui|{
                ui.label(RichText::new("PEAKRUNNER").size(34.).strong());
                ui.label(RichText::new("LAUNCHER / 01").size(12.).color(ACCENT));
            });
            ui.label(RichText::new("Own the ridgeline. Keep your game ready.").color(MUTED));
            ui.add_space(16.);
            egui::Frame::new().fill(PANEL).corner_radius(8).inner_margin(20).show(ui,|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new(self.installed.as_deref().unwrap_or("Ready for your first drop")).size(19.).strong());
                ui.label(RichText::new(format!("{} · verified, per-user installation",updater::platform())).color(MUTED));
                ui.add_space(8.);
                let ready=!self.busy && self.child.is_none() && self.store.is_some();
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(ready && self.installed.is_some(),egui::Button::new(RichText::new("PLAY").color(BG).strong()).fill(ACCENT).min_size(Vec2::new(128.,44.))).clicked(){self.play();}
                    let needs_update=self.available.as_ref().is_some_and(|r|Some(&r.manifest.version)!=self.installed.as_ref());
                    if ui.add_enabled(ready && self.available.is_some(),egui::Button::new(if self.installed.is_none(){"Install game"}else if needs_update{"Update game"}else{"Repair / verify"}).min_size(Vec2::new(128.,44.))).clicked(){self.install();}
                    if ui.add_enabled(ready,egui::Button::new("Check updates").min_size(Vec2::new(112.,44.))).clicked(){self.check();}
                    if ui.add_enabled(ready && self.installed.is_some(),egui::Button::new("Rollback").min_size(Vec2::new(90.,44.))).clicked(){self.rollback_confirm=true;}
                });
                if self.rollback_confirm {
                    ui.label("Restore the previous install? Older versions may not match the live server.");
                    ui.horizontal(|ui|{if ui.button("Restore previous").clicked(){self.rollback_confirm=false;self.rollback();}if ui.button("Cancel").clicked(){self.rollback_confirm=false;}});
                }
                ui.add_space(6.);
                ui.horizontal_wrapped(|ui|{if self.busy {ui.spinner();}ui.label(&self.status);});
            });
            ui.add_space(12.);
            egui::ScrollArea::vertical().show(ui,|ui| {
                ui.label(RichText::new("RELEASE NOTES").color(ACCENT).size(12.));
                if let Some(r)=&self.available {
                    ui.label(RichText::new(&r.manifest.version).strong());
                    egui::ScrollArea::vertical().id_salt("notes").max_height(180.).show(ui,|ui|{ui.label(&r.manifest.notes);});
                }
                else {ui.label(RichText::new("No verified release information is available yet.").color(MUTED));}
                ui.add_space(16.);ui.label(RichText::new("LIVE HOSTS").color(ACCENT).size(12.));
                if self.hosts.is_empty(){ui.label(RichText::new("No hosts available, or directory temporarily unreachable.").color(MUTED));}
                for host in &self.hosts {ui.horizontal_wrapped(|ui|{ui.label(RichText::new(&host.name).strong());ui.label(format!("{} · {} / {} players",host.map,host.players,host.max_players));});}
                ui.add_space(16.);ui.separator();
                ui.label(RichText::new("Only changed files are downloaded. Failed updates leave your working version intact. Launch the game to choose a server.").size(12.).color(MUTED));
                if let Some(store)=&self.store {ui.label(RichText::new(store.root.display().to_string()).size(11.).color(MUTED));}
                ui.hyperlink_to("Downloads & help","https://peakrunner.net/#downloads");
            });
        });
    }
}
fn main() -> eframe::Result {
    eframe::run_native(
        "PeakRunner Launcher",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([800., 720.])
                .with_min_inner_size([600., 520.]),
            renderer: updater::native_renderer(),
            ..Default::default()
        },
        Box::new(|cc| Ok(Box::new(Launcher::new(cc)))),
    )
}
