use gorilla_api::{
    gorilla_player_get_position,
    gorilla_player_get_hand_position,
};
use crate::mods::ALL_MODS;
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

// called every frame from the LateUpdate hook
pub unsafe fn tick() {
    let mut state = match get_state().try_lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if !state.loaded {
        state.load_from_disk();
        let enabled: Vec<String> = state.enabled.iter().cloned().collect();
        for id in &enabled {
            if let Some(m) = ALL_MODS.iter().find(|m| m.id == id.as_str()) {
                (m.on_enable)();
            }
        }
    }

    let enabled: Vec<String> = state.enabled.iter().cloned().collect();
    drop(state);

    // tick active mods
    for id in &enabled {
        if let Some(m) = ALL_MODS.iter().find(|m| m.id == id.as_str()) {
            if let Some(tick_fn) = m.tick {
                tick_fn();
            }
        }
    }

    // gesture: hands very close = toggle menu
    let mut lx = 0f32; let mut ly = 0f32; let mut lz = 0f32;
    let mut rx = 0f32; let mut ry = 0f32; let mut rz = 0f32;
    gorilla_player_get_hand_position(1, &mut lx, &mut ly, &mut lz);
    gorilla_player_get_hand_position(0, &mut rx, &mut ry, &mut rz);
    let left_hand  = Vec3 { x: lx, y: ly, z: lz };
    let right_hand = Vec3 { x: rx, y: ry, z: rz };
    let hands_close = left_hand.dist(right_hand) < 0.1;

    let mut state = match get_state().try_lock() {
        Ok(s) => s,
        Err(_) => return,
    };

    if hands_close && !state.open {
        state.open = true;
    } else if !hands_close && state.open {
        // check button presses while menu is open
        let mut hx = 0f32; let mut hy = 0f32; let mut hz = 0f32;
        gorilla_player_get_position(&mut hx, &mut hy, &mut hz);
        let head = Vec3 { x: hx, y: hy, z: hz };
        let btns = button_positions(head);

        for (i, btn_pos) in btns.iter().enumerate() {
            if i >= ALL_MODS.len() { break; }
            let mod_id = ALL_MODS[i].id;
            if hand_in_button(left_hand, *btn_pos) || hand_in_button(right_hand, *btn_pos) {
                state.toggle(mod_id);
            }
        }
    }
}
