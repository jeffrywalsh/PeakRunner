mod render;
mod sim;
mod terrain;

use render::Renderer;
use sim::{MatchState, World};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

#[wasm_bindgen]
pub struct Game {
    world: World,
    renderer: Renderer,
}

#[wasm_bindgen]
impl Game {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement) -> Result<Game, JsValue> {
        let renderer = Renderer::new(&canvas).map_err(|e| JsValue::from_str(&e))?;
        Ok(Game {
            world: World::new(),
            renderer,
        })
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        self.renderer.resize(w, h);
    }

    pub fn start_match(&mut self, ember: bool) {
        self.world.start_match(ember);
    }

    pub fn set_paused(&mut self, paused: bool) {
        match (self.world.state, paused) {
            (MatchState::Playing, true) => self.world.state = MatchState::Paused,
            (MatchState::Paused, false) => self.world.state = MatchState::Playing,
            _ => {}
        }
    }

    pub fn add_look(&mut self, dx: f32, dy: f32) {
        self.world.add_look(dx, dy);
    }

    pub fn set_input(
        &mut self,
        move_x: f32,
        move_z: f32,
        jump: bool,
        fire: bool,
        weapon: u8,
        look_x: f32,
        look_y: f32,
    ) {
        if self.world.input.keys_override.is_none() {
            self.world.input.move_x = move_x;
            self.world.input.move_z = move_z;
            self.world.input.jump = jump;
            self.world.input.fire = fire;
        }
        self.world.input.weapon = weapon.min(1);
        self.world.input.look_stick_x = look_x;
        self.world.input.look_stick_y = look_y;
    }

    pub fn set_keys(&mut self, codes: js_sys::Array) {
        let mut list = Vec::new();
        for c in codes.iter() {
            if let Some(s) = c.as_string() {
                list.push(s);
            }
        }
        if list.is_empty() {
            self.world.input.keys_override = None;
            self.world.input.move_x = 0.0;
            self.world.input.move_z = 0.0;
            self.world.input.jump = false;
            self.world.input.fire = false;
        } else {
            self.world.input.keys_override = Some(list);
        }
    }

    pub fn frame(&mut self, dt: f32) -> String {
        self.world.apply_override();
        self.world.tick(dt);
        self.renderer.draw(&self.world);
        self.world.hud_json()
    }

    pub fn get_yaw(&self) -> f32 {
        self.world.player_yaw()
    }

    pub fn get_speed(&self) -> f32 {
        self.world.player_speed()
    }

    pub fn get_px(&self) -> f32 {
        self.world.player_pos().x
    }
    pub fn get_py(&self) -> f32 {
        self.world.player_pos().y
    }
    pub fn get_pz(&self) -> f32 {
        self.world.player_pos().z
    }
}
