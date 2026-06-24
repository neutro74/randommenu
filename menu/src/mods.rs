use gorilla_api::{
    gorilla_player_set_velocity,
    gorilla_player_add_force,
    gorilla_player_set_scale,
    gorilla_player_set_movement_disabled,
    gorilla_player_get_velocity,
    gorilla_rig_set_color,
};

// every mod has an id, display name, and whether it needs a per-frame tick
pub struct Mod {
    pub id: &'static str,
    pub name: &'static str,
    // called once when the mod is toggled on
    pub on_enable: fn(),
    // called once when toggled off
    pub on_disable: fn(),
    // called every frame while active, None if the mod is set-and-forget
    pub tick: Option<fn()>,
}

// --- mod implementations ---

fn speed_enable()  {}
fn speed_disable() { unsafe { gorilla_player_set_velocity(0.0, 0.0, 0.0); } }
fn speed_tick() {
    unsafe {
        let mut vx = 0f32; let mut vy = 0f32; let mut vz = 0f32;
        gorilla_player_get_velocity(&mut vx, &mut vy, &mut vz);
        // scale horizontal velocity up
        gorilla_player_set_velocity(vx * 1.5, vy, vz * 1.5);
    }
}

fn fly_enable()  {}
fn fly_disable() { unsafe { gorilla_player_set_velocity(0.0, 0.0, 0.0); } }
fn fly_tick() {
    unsafe {
        let mut vx = 0f32; let mut vy = 0f32; let mut vz = 0f32;
        gorilla_player_get_velocity(&mut vx, &mut vy, &mut vz);
        // counteract gravity to hover
        gorilla_player_add_force(0.0, 9.81, 0.0, 1); // ForceMode::Acceleration
    }
}

fn long_arms_enable()  { unsafe { gorilla_player_set_scale(1.5); } }
fn long_arms_disable() { unsafe { gorilla_player_set_scale(1.0); } }

fn freeze_enable()  { unsafe { gorilla_player_set_movement_disabled(1); } }
fn freeze_disable() { unsafe { gorilla_player_set_movement_disabled(0); } }

fn ghost_enable()  { unsafe { gorilla_rig_set_color(0.5, 0.5, 0.5); } }
fn ghost_disable() { unsafe { gorilla_rig_set_color(0.0, 0.0, 0.0); } }

fn bounce_enable()  {}
fn bounce_disable() {}
fn bounce_tick() {
    unsafe {
        let mut vx = 0f32; let mut vy = 0f32; let mut vz = 0f32;
        gorilla_player_get_velocity(&mut vx, &mut vy, &mut vz);
        // if moving downward and close to ground, launch up
        if vy < -1.0 {
            gorilla_player_add_force(0.0, vy.abs() * 2.0, 0.0, 2); // ForceMode::Impulse
        }
    }
}

pub const ALL_MODS: &[Mod] = &[
    Mod {
        id: "speed",
        name: "Speed Boost",
        on_enable: speed_enable,
        on_disable: speed_disable,
        tick: Some(speed_tick),
    },
    Mod {
        id: "fly",
        name: "Fly",
        on_enable: fly_enable,
        on_disable: fly_disable,
        tick: Some(fly_tick),
    },
    Mod {
        id: "long_arms",
        name: "Long Arms",
        on_enable: long_arms_enable,
        on_disable: long_arms_disable,
        tick: None,
    },
    Mod {
        id: "freeze",
        name: "Freeze Self",
        on_enable: freeze_enable,
        on_disable: freeze_disable,
        tick: None,
    },
    Mod {
        id: "ghost",
        name: "Ghost",
        on_enable: ghost_enable,
        on_disable: ghost_disable,
        tick: None,
    },
    Mod {
        id: "bounce",
        name: "Bounce",
        on_enable: bounce_enable,
        on_disable: bounce_disable,
        tick: Some(bounce_tick),
    },
];
