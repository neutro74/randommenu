use gorilla_api::*;

// mod index constants matching the C# plugin button order
pub const MOD_SPEED:      u32 = 0;
pub const MOD_FLY:        u32 = 1;
pub const MOD_LONG_ARMS:  u32 = 2;
pub const MOD_FREEZE:     u32 = 3;
pub const MOD_GHOST:      u32 = 4;
pub const MOD_BOUNCE:     u32 = 5;

pub unsafe fn on_enable(index: u32) {
    match index {
        MOD_LONG_ARMS => { gorilla_player_set_scale(1.5); }
        MOD_FREEZE    => { gorilla_player_set_movement_disabled(1); }
        MOD_GHOST     => { gorilla_rig_set_color(0.5, 0.5, 0.5); }
        _ => {}
    }
}

pub unsafe fn on_disable(index: u32) {
    match index {
        MOD_SPEED     => { gorilla_player_set_velocity(0.0, 0.0, 0.0); }
        MOD_FLY       => { gorilla_player_set_velocity(0.0, 0.0, 0.0); }
        MOD_LONG_ARMS => { gorilla_player_set_scale(1.0); }
        MOD_FREEZE    => { gorilla_player_set_movement_disabled(0); }
        MOD_GHOST     => { gorilla_rig_set_color(0.0, 0.0, 0.0); }
        _ => {}
    }
}

pub unsafe fn tick(index: u32) {
    match index {
        MOD_SPEED => {
            let mut vx = 0f32; let mut vy = 0f32; let mut vz = 0f32;
            gorilla_player_get_velocity(&mut vx, &mut vy, &mut vz);
            gorilla_player_set_velocity(vx * 1.5, vy, vz * 1.5);
        }
        MOD_FLY => {
            gorilla_player_add_force(0.0, 9.81, 0.0, 1);
        }
        MOD_BOUNCE => {
            let mut vx = 0f32; let mut vy = 0f32; let mut vz = 0f32;
            gorilla_player_get_velocity(&mut vx, &mut vy, &mut vz);
            if vy < -1.0 {
                gorilla_player_add_force(0.0, vy.abs() * 2.0, 0.0, 2);
            }
        }
        _ => {}
    }
}
