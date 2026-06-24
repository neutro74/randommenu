use gorilla_api::{
    gorilla_player_get_position,
    gorilla_player_get_hand_position,
};
use crate::input;
use crate::mods::ALL_MODS;
use crate::render;
use crate::state::STATE as get_state;

const PRESS_RADIUS: f32 = 0.08;
const BTN_H: f32 = 0.05;
const PANEL_DIST: f32 = 0.5;

#[derive(Clone, Copy)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn dist(&self, other: Vec3) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx*dx + dy*dy + dz*dz).sqrt()
    }
}

fn button_positions(head: Vec3) -> Vec<Vec3> {
    let count = ALL_MODS.len();
    let mut positions = Vec::with_capacity(count);
    for i in 0..count {
        let y_offset = (i as f32 - count as f32 / 2.0) * (BTN_H + 0.01);
        positions.push(Vec3 {
            x: head.x,
            y: head.y + y_offset,
            z: head.z + PANEL_DIST,
        });
    }
    positions
}

fn hand_in_button(hand: Vec3, btn: Vec3) -> bool {
    hand.dist(btn) < PRESS_RADIUS
}

pub unsafe fn tick() {
    let mut state = match get_state().try_lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    // restore saved mod state on first tick
    if !state.loaded {
        state.load_from_disk();
        let enabled: Vec<String> = state.enabled.iter().cloned().collect();
        for id in &enabled {
            if let Some(m) = ALL_MODS.iter().find(|m| m.id == id.as_str()) {
                (m.on_enable)();
            }
        }
    }

    // Y button toggles menu open/closed
    if input::y_button_down() {
        state.open = !state.open;
    }

    // tick active mods
    let enabled: Vec<String> = state.enabled.iter().cloned().collect();
    drop(state);

    for id in &enabled {
        if let Some(m) = ALL_MODS.iter().find(|m| m.id == id.as_str()) {
            if let Some(tick_fn) = m.tick {
                tick_fn();
            }
        }
    }

    // while menu is open, check if either hand is inside a button
    let mut state = match get_state().try_lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if state.open {
        let mut hx = 0f32; let mut hy = 0f32; let mut hz = 0f32;
        gorilla_player_get_position(&mut hx, &mut hy, &mut hz);
        let head = Vec3 { x: hx, y: hy, z: hz };
        let btns = button_positions(head);

        let mut lx = 0f32; let mut ly = 0f32; let mut lz = 0f32;
        let mut rx = 0f32; let mut ry = 0f32; let mut rz = 0f32;
        gorilla_player_get_hand_position(1, &mut lx, &mut ly, &mut lz);
        gorilla_player_get_hand_position(0, &mut rx, &mut ry, &mut rz);
        let left_hand  = Vec3 { x: lx, y: ly, z: lz };
        let right_hand = Vec3 { x: rx, y: ry, z: rz };

        for (i, btn_pos) in btns.iter().enumerate() {
            if i >= ALL_MODS.len() { break; }
            if hand_in_button(left_hand, *btn_pos) || hand_in_button(right_hand, *btn_pos) {
                state.toggle(ALL_MODS[i].id);
            }
        }
    }

    let mut hx = 0f32; let mut hy = 0f32; let mut hz = 0f32;
    gorilla_player_get_position(&mut hx, &mut hy, &mut hz);
    let enabled: Vec<String> = state.enabled.iter().cloned().collect();
    let selected = state.selected_index;
    let open = state.open;
    drop(state);

    render::update(open, hx, hy, hz, &enabled, selected);
}
